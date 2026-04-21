use askama::Template;
use axum::{
    Router,
    extract::Request,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use surrealdb::types::RecordId;
use tracing::error;

use crate::{
    db::DB,
    error::Error,
    middleware::{AuthenticatedUser, UserExtractor},
    templates::{BaseContext, GetVerifiedTemplate, User},
};

pub fn router() -> Router {
    Router::new()
        .route("/get-verified", get(get_verified_page))
        .route("/get-verified/request", post(request_verification))
}

fn parse_person_rid(person_id: &str) -> Option<RecordId> {
    if person_id.starts_with("person:") {
        RecordId::parse_simple(person_id).ok()
    } else {
        Some(RecordId::new("person", person_id))
    }
}

async fn has_pending_verification(person_id: &str) -> bool {
    let Some(rid) = parse_person_rid(person_id) else {
        return false;
    };
    if let Ok(mut result) = DB
        .query("SELECT count() AS c FROM verification_request WHERE person = $pid AND status = 'pending' GROUP ALL")
        .bind(("pid", rid))
        .await
        && let Ok(Some(row)) = result.take::<Option<serde_json::Value>>(0)
            && let Some(c) = row.get("c").and_then(|v| v.as_i64()) {
                return c > 0;
            }
    false
}

async fn get_verified_page(request: Request) -> Result<Response, Error> {
    let mut base = BaseContext::new().with_page("get-verified");
    let mut pending = false;

    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
        pending = has_pending_verification(&user.id).await;
    }

    let template = GetVerifiedTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        has_pending_request: pending,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render get-verified template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

async fn request_verification(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Response, Error> {
    let person_id = &user.id;

    // Check if already has a request
    if has_pending_verification(person_id).await {
        return Ok(Redirect::to("/get-verified").into_response());
    }

    let rid = parse_person_rid(person_id)
        .ok_or_else(|| Error::BadRequest("Invalid person ID".to_string()))?;

    if let Err(e) = DB
        .query("CREATE verification_request SET person = $pid, status = 'pending', created_at = time::now()")
        .bind(("pid", rid))
        .await
    {
        error!("Failed to create verification request: {}", e);
    }

    Ok(Redirect::to("/get-verified").into_response())
}
