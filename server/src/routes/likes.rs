use askama::Template;
use axum::{
    Json, Router,
    extract::{Path, Query as AxumQuery, Request},
    http::header,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;
use tracing::{debug, error};

use crate::{
    error::Error,
    middleware::{AuthenticatedUser, UserExtractor},
    models::likes::LikesModel,
    templates::{BaseContext, LikesTemplate, User},
};

pub fn router() -> Router {
    Router::new()
        .route("/likes", get(likes_page))
        .route("/api/likes/toggle", post(toggle_like))
        .route("/api/likes/toggle-sse/{target_id}", post(toggle_like_sse))
        .route("/api/likes/check", post(check_likes))
}

#[derive(Deserialize)]
struct ToggleRequest {
    target_id: String,
}

#[derive(Serialize)]
struct ToggleResponse {
    liked: bool,
}

#[derive(Deserialize)]
struct CheckRequest {
    ids: Vec<String>,
}

#[derive(Serialize)]
struct CheckResponse {
    liked_ids: Vec<String>,
}

/// Parse a "table:key" string into a RecordId
fn parse_target_id(s: &str) -> Result<RecordId, Error> {
    RecordId::parse_simple(s)
        .map_err(|e| Error::BadRequest(format!("Invalid target ID '{}': {}", s, e)))
}

/// Validate that a target_id string is safe for use in CSS selectors and HTML attributes.
/// Must be in format `table:key` with only safe characters.
fn validate_target_id_str(s: &str) -> Result<(), Error> {
    if s.is_empty()
        || !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '-')
    {
        return Err(Error::BadRequest(format!(
            "Invalid target ID format: {}",
            s
        )));
    }
    Ok(())
}

/// Toggle a like (requires auth)
async fn toggle_like(
    AuthenticatedUser(user): AuthenticatedUser,
    Json(body): Json<ToggleRequest>,
) -> Result<Json<ToggleResponse>, Error> {
    debug!("Toggle like: user={} target={}", user.id, body.target_id);

    let person_id = if user.id.starts_with("person:") {
        RecordId::parse_simple(&user.id).map_err(|e| Error::BadRequest(e.to_string()))?
    } else {
        RecordId::new("person", user.id.as_str())
    };

    let target_id = parse_target_id(&body.target_id)?;
    let liked = LikesModel::toggle(&person_id, &target_id).await?;

    Ok(Json(ToggleResponse { liked }))
}

/// Check which of the given IDs are liked (requires auth)
async fn check_likes(
    AuthenticatedUser(user): AuthenticatedUser,
    Json(body): Json<CheckRequest>,
) -> Result<Json<CheckResponse>, Error> {
    let person_id = if user.id.starts_with("person:") {
        RecordId::parse_simple(&user.id).map_err(|e| Error::BadRequest(e.to_string()))?
    } else {
        RecordId::new("person", user.id.as_str())
    };

    let target_ids: Vec<RecordId> = body
        .ids
        .iter()
        .filter_map(|s| RecordId::parse_simple(s).ok())
        .collect();

    let liked_ids = LikesModel::get_liked_ids(&person_id, &target_ids).await?;

    Ok(Json(CheckResponse { liked_ids }))
}

// -- SSE helpers for Datastar --

fn sse_patch_elements(selector: &str, mode: &str, elements: &str) -> String {
    let mut s = format!(
        "event: datastar-patch-elements\ndata: selector {}\ndata: mode {}\n",
        selector, mode
    );
    if !elements.is_empty() {
        s += &format!("data: elements {}\n", elements.replace('\n', " "));
    }
    s += "\n";
    s
}

fn sse_response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

const HEART_PATH: &str = "M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z";

fn like_button_html(target_id: &str, liked: bool, variant: &str) -> String {
    let fill = if liked { "#e53e3e" } else { "none" };
    let stroke = if liked { "#e53e3e" } else { "currentColor" };
    let label = if liked { "Unlike" } else { "Like" };

    match variant {
        "people" => format!(
            r#"<button type="button" data-role="card-like" data-like-target="{tid}" data-on:click="@post('/api/likes/toggle-sse/{tid}?v=people')" data-liked="{liked}" aria-label="{label}"><svg width="18" height="18" viewBox="0 0 24 24" fill="{fill}" stroke="{stroke}" stroke-width="1.5"><path d="{hp}"/></svg></button>"#,
            tid = target_id,
            liked = liked,
            label = label,
            fill = fill,
            stroke = stroke,
            hp = HEART_PATH
        ),
        "locations" => format!(
            r#"<button type="button" class="loc-card-like" data-like-target="{tid}" data-on:click="@post('/api/likes/toggle-sse/{tid}?v=locations')" data-liked="{liked}" aria-label="{label}"><svg width="18" height="18" viewBox="0 0 24 24" fill="{fill}" stroke="{stroke}" stroke-width="1.5"><path d="{hp}"/></svg></button>"#,
            tid = target_id,
            liked = liked,
            label = label,
            fill = fill,
            stroke = stroke,
            hp = HEART_PATH
        ),
        "profile" => {
            let type_val = if liked { "liked" } else { "outline" };
            let text = if liked { "Liked" } else { "Like" };
            format!(
                r#"<button type="button" data-like-target="{tid}" data-on:click="@post('/api/likes/toggle-sse/{tid}?v=profile')" data-liked="{liked}" data-type="{type_val}" aria-label="{label}"><svg width="18" height="18" viewBox="0 0 24 24" fill="{fill}" stroke="{stroke}" stroke-width="1.5" style="vertical-align:middle;margin-right:4px"><path d="{hp}"/></svg>{text}</button>"#,
                tid = target_id,
                liked = liked,
                type_val = type_val,
                label = label,
                fill = fill,
                stroke = stroke,
                hp = HEART_PATH,
                text = text
            )
        }
        "likes" => format!(
            r#"<button type="button" data-role="card-like" data-like-target="{tid}" data-on:click="@post('/api/likes/toggle-sse/{tid}?v=likes')" data-liked="{liked}" aria-label="{label}"><svg width="18" height="18" viewBox="0 0 24 24" fill="{fill}" stroke="{stroke}" stroke-width="1.5"><path d="{hp}"/></svg></button>"#,
            tid = target_id,
            liked = liked,
            label = label,
            fill = fill,
            stroke = stroke,
            hp = HEART_PATH
        ),
        _ => format!(
            r#"<button type="button" data-like-target="{tid}" data-on:click="@post('/api/likes/toggle-sse/{tid}')" data-liked="{liked}" aria-label="{label}"><svg width="20" height="20" viewBox="0 0 24 24" fill="{fill}" stroke="{stroke}" stroke-width="1.5"><path d="{hp}"/></svg></button>"#,
            tid = target_id,
            liked = liked,
            label = label,
            fill = fill,
            stroke = stroke,
            hp = HEART_PATH
        ),
    }
}

#[derive(Deserialize)]
struct ToggleSseQuery {
    v: Option<String>,
}

/// Toggle a like via SSE (Datastar)
async fn toggle_like_sse(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(target_id_raw): Path<String>,
    AxumQuery(query): AxumQuery<ToggleSseQuery>,
) -> Result<Response, Error> {
    debug!("Toggle like SSE: user={} target={}", user.id, target_id_raw);

    // Validate target_id is safe for use in CSS selectors and HTML attributes
    validate_target_id_str(&target_id_raw)?;

    let person_id = if user.id.starts_with("person:") {
        RecordId::parse_simple(&user.id).map_err(|e| Error::BadRequest(e.to_string()))?
    } else {
        RecordId::new("person", user.id.as_str())
    };

    let target_id = parse_target_id(&target_id_raw)?;
    let liked = LikesModel::toggle(&person_id, &target_id).await?;

    let variant = query.v.as_deref().unwrap_or("default");
    let btn_html = like_button_html(&target_id_raw, liked, variant);
    let selector = format!(r#"[data-like-target="{}"]"#, target_id_raw);

    let mut sse = sse_patch_elements(&selector, "outer", &btn_html);

    // On likes page, remove the card and update the tab count when unliked
    if !liked && variant == "likes" {
        let card_selector = format!(r#"[data-like-card="{}"]"#, target_id_raw);
        sse += &sse_patch_elements(&card_selector, "remove", "");

        // Update the tab count
        let count_tab = if target_id_raw.starts_with("location:") {
            "locations"
        } else {
            "people"
        };
        let count = if count_tab == "people" {
            LikesModel::count_liked_people(&person_id)
                .await
                .unwrap_or(0)
        } else {
            LikesModel::count_liked_locations(&person_id)
                .await
                .unwrap_or(0)
        };
        let count_selector = format!("#likes-count-{}", count_tab);
        sse += &sse_patch_elements(&count_selector, "inner", &count.to_string());
    }

    Ok(sse_response(sse))
}

/// Likes page (requires auth)
async fn likes_page(request: Request) -> Result<Html<String>, Error> {
    let current_user = match request.get_user() {
        Some(u) => u,
        None => return Err(Error::Unauthorized),
    };

    let mut base = BaseContext::new().with_page("likes");
    base = base.with_user(User::from_session_user(&current_user).await);

    let person_id = if current_user.id.starts_with("person:") {
        RecordId::parse_simple(&current_user.id).map_err(|e| Error::BadRequest(e.to_string()))?
    } else {
        RecordId::new("person", current_user.id.as_str())
    };

    let liked_people = LikesModel::get_liked_people(&person_id)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to get liked people: {}", e);
            vec![]
        });

    let liked_locations = LikesModel::get_liked_locations(&person_id)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to get liked locations: {}", e);
            vec![]
        });

    let template = LikesTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        liked_people,
        liked_locations,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render likes template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}
