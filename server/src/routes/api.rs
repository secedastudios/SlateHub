use axum::{
    Extension, Json, Router,
    extract::{Path, Query},
    response::{IntoResponse, Redirect},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::db::DB;
use crate::middleware::{AuthenticatedUser, CurrentUser};
use crate::models::involvement::InvolvementModel;
use crate::models::production::ProductionModel;
use crate::models::system::System;
use crate::record_id_ext::RecordIdExt;

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(stats))
        .route("/avatar", get(avatar))
        .route("/fix-avatar-urls", post(fix_avatar_urls))
        .route("/tmdb/search", get(tmdb_search))
        .route("/tmdb/credits/{person_id}", get(tmdb_credits))
        .route("/tmdb/import", post(tmdb_import))
        .route("/productions/search", get(productions_search))
        .route("/productions/{slug}/claim", post(production_claim))
        .route("/involvements", post(create_involvement))
        .route("/involvements/with-production", post(create_involvement_with_production))
        .route("/involvements/{id}", delete(delete_involvement))
        .route("/involvements/{id}/verify", post(verify_involvement))
        .route("/involvements/{id}/reject", post(reject_involvement))
        .route("/feedback", post(submit_feedback))
        .route("/check-username", get(check_username))
        .route("/people/search", get(people_search))
        .route("/og/profile/{username}", get(og_profile_image))
        .route("/qr/profile/{username}", get(qr_profile_image))
}

#[axum::debug_handler]
async fn health_check() -> impl IntoResponse {
    debug!("Health check requested");

    match System::health_check().await {
        Ok(health) => {
            info!(
                "Health check complete: status={}, db={}",
                health.status, health.database
            );
            Json(health).into_response()
        }
        Err(e) => {
            tracing::error!("Health check failed: {:?}", e);
            let error_response = serde_json::json!({
                "status": "error",
                "database": "unknown",
                "version": env!("CARGO_PKG_VERSION"),
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "error": e.to_string()
            });
            Json(error_response).into_response()
        }
    }
}

#[derive(Serialize)]
struct PlatformStats {
    productions: usize,
    users: usize,
    connections: usize,
}

#[axum::debug_handler]
async fn stats() -> impl IntoResponse {
    debug!("Stats endpoint called");

    // Use the System model to get actual counts
    let productions = System::count_records("production").await.unwrap_or(0);
    let users = System::count_records("person").await.unwrap_or(0);
    let connections = System::count_records("involvement").await.unwrap_or(0);

    let stats = PlatformStats {
        productions,
        users,
        connections,
    };

    Json(stats).into_response()
}

#[axum::debug_handler]
async fn avatar(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let id = params.get("id").map(|s| s.as_str()).unwrap_or("unknown");
    debug!("Avatar requested for user: {}", id);

    // First, try to get the actual avatar URL from the person's profile
    let person_rid = if id.starts_with("person:") {
        surrealdb::types::RecordId::parse_simple(id)
    } else {
        Ok(surrealdb::types::RecordId::new("person", id))
    };

    // Query for the person's avatar URL
    if let Ok(rid) = person_rid {
    if let Ok(mut response) = DB.query("SELECT profile.avatar FROM ONLY $pid LIMIT 1")
        .bind(("pid", rid)).await {
        if let Ok(result) = response.take::<Option<serde_json::Value>>(0) {
            if let Some(data) = result {
                if let Some(avatar_url) = data
                    .get("profile")
                    .and_then(|p| p.get("avatar"))
                    .and_then(|a| a.as_str())
                {
                    // User has a custom avatar, redirect to it
                    return Redirect::permanent(avatar_url);
                }
            }
        }
    }}

    // Fall back to DiceBear for deterministic avatars based on user ID
    let avatar_url = format!(
        "https://api.dicebear.com/7.x/initials/svg?seed={}&backgroundColor=4f46e5",
        id
    );

    Redirect::permanent(&avatar_url)
}

/// Search TMDB for people by name
async fn tmdb_search(
    AuthenticatedUser(_user): AuthenticatedUser,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let query = match params.get("q") {
        Some(q) if !q.is_empty() => q,
        _ => {
            return Json(serde_json::json!({ "error": "Missing 'q' query parameter" }))
                .into_response();
        }
    };

    let service = match crate::services::tmdb::get_service() {
        Ok(s) => s,
        Err(_) => {
            return Json(serde_json::json!({ "error": "TMDB API key not configured" }))
                .into_response();
        }
    };

    match service.search_person(query).await {
        Ok(results) => Json(serde_json::json!({ "results": results })).into_response(),
        Err(e) => {
            error!("TMDB search failed: {}", e);
            Json(serde_json::json!({ "error": format!("TMDB search failed: {}", e) }))
                .into_response()
        }
    }
}

/// Fetch combined credits for a TMDB person
async fn tmdb_credits(
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(person_id): Path<i64>,
) -> impl IntoResponse {
    let service = match crate::services::tmdb::get_service() {
        Ok(s) => s,
        Err(_) => {
            return Json(serde_json::json!({ "error": "TMDB API key not configured" }))
                .into_response();
        }
    };

    match service.get_person_credits(person_id).await {
        Ok(credits) => Json(serde_json::json!({ "credits": credits })).into_response(),
        Err(e) => {
            error!("TMDB credits fetch failed: {}", e);
            Json(serde_json::json!({ "error": format!("TMDB credits fetch failed: {}", e) }))
                .into_response()
        }
    }
}

// --- TMDB Import ---

#[derive(Debug, Deserialize)]
struct TmdbImportCredit {
    tmdb_id: i64,
    title: String,
    role: String,
    media_type: String,
    poster_url: Option<String>,
    tmdb_url: String,
    release_date: Option<String>,
    overview: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmdbImportRequest {
    credits: Vec<TmdbImportCredit>,
}

/// Import selected TMDB credits: find/create productions, then create involvement edges
#[axum::debug_handler]
async fn tmdb_import(
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<TmdbImportRequest>,
) -> impl IntoResponse {
    info!("TMDB import: user={}, credits_count={}", user.username, payload.credits.len());
    let person_id = &user.id;
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();
    let mut imported_credits: Vec<serde_json::Value> = Vec::new();

    for credit in &payload.credits {
        // Determine relation_type and credit_type from role/department
        let (relation_type, credit_type) = if credit.department.is_some() {
            ("crew", Some("crew"))
        } else {
            ("cast", Some("cast"))
        };

        // Find or create the production from TMDB data
        let production = match ProductionModel::find_or_create_from_tmdb(
            credit.tmdb_id,
            credit.title.clone(),
            credit.media_type.clone(),
            credit.poster_url.clone(),
            credit.tmdb_url.clone(),
            credit.release_date.clone(),
            credit.overview.clone(),
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to find/create production for tmdb_id {}: {}", credit.tmdb_id, e);
                errors.push(format!("{}: {}", credit.title, e));
                continue;
            }
        };

        // Check for dedup
        match InvolvementModel::exists(person_id, &production.id, Some(&credit.role)).await {
            Ok(true) => {
                skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                error!("Failed dedup check: {}", e);
                errors.push(format!("{}: {}", credit.title, e));
                continue;
            }
        }

        // Create involvement edge
        match InvolvementModel::create(
            person_id,
            &production.id,
            relation_type,
            Some(&credit.role),
            credit.department.as_deref(),
            credit_type,
            "tmdb_import",
        )
        .await
        {
            Ok(involvement_id) => {
                imported += 1;
                imported_credits.push(serde_json::json!({
                    "involvement_id": involvement_id,
                    "role": credit.role,
                    "relation_type": relation_type,
                    "production_title": credit.title,
                    "production_slug": production.slug,
                    "production_type": production.production_type,
                    "poster_url": credit.poster_url,
                    "tmdb_url": credit.tmdb_url,
                    "release_date": credit.release_date,
                    "verification_status": "externally_sourced",
                }));
            }
            Err(e) => {
                error!("Failed to create involvement: {}", e);
                errors.push(format!("{}: {}", credit.title, e));
            }
        }
    }

    info!("TMDB import complete: imported={}, skipped={}, errors={}", imported, skipped, errors.len());
    Json(serde_json::json!({
        "imported": imported,
        "skipped": skipped,
        "errors": errors,
        "credits": imported_credits,
    }))
}

// --- Production Search ---

/// Search productions by title for autocomplete / dedup
async fn productions_search(
    AuthenticatedUser(_user): AuthenticatedUser,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let query = match params.get("q") {
        Some(q) if !q.is_empty() => q,
        _ => {
            return Json(serde_json::json!({ "results": [] })).into_response();
        }
    };

    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(10);

    match ProductionModel::search_by_title(query, limit).await {
        Ok(productions) => {
            let results: Vec<serde_json::Value> = productions
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "id": p.id.to_raw_string(),
                        "title": p.title,
                        "slug": p.slug,
                        "type": p.production_type,
                        "poster_url": p.poster_url,
                        "tmdb_id": p.tmdb_id,
                        "release_date": p.release_date,
                        "media_type": p.media_type,
                    })
                })
                .collect();
            Json(serde_json::json!({ "results": results })).into_response()
        }
        Err(e) => {
            error!("Production search failed: {}", e);
            Json(serde_json::json!({ "error": format!("Search failed: {}", e) })).into_response()
        }
    }
}

// --- Production Claim ---

/// Claim an unclaimed production (creates owner member_of edge)
async fn production_claim(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let production = match ProductionModel::get_by_slug(&slug).await {
        Ok(p) => p,
        Err(_) => {
            return Json(serde_json::json!({ "error": "Production not found" })).into_response();
        }
    };

    // Check if already claimed
    match ProductionModel::is_claimed(&production.id).await {
        Ok(true) => {
            return Json(serde_json::json!({ "error": "Production is already claimed" }))
                .into_response();
        }
        Ok(false) => {}
        Err(e) => {
            return Json(serde_json::json!({ "error": format!("Failed to check claim: {}", e) }))
                .into_response();
        }
    }

    match ProductionModel::claim(&production.id, &user.id).await {
        Ok(()) => {
            info!("User {} claimed production {}", user.username, slug);
            Json(serde_json::json!({ "success": true })).into_response()
        }
        Err(e) => {
            error!("Failed to claim production: {}", e);
            Json(serde_json::json!({ "error": format!("Failed to claim: {}", e) })).into_response()
        }
    }
}

// --- Involvement CRUD ---

#[derive(Debug, Deserialize)]
struct CreateInvolvementRequest {
    production_id: String,
    relation_type: String,
    role: Option<String>,
    department: Option<String>,
    credit_type: Option<String>,
}

/// Create an involvement edge to an existing production
async fn create_involvement(
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateInvolvementRequest>,
) -> impl IntoResponse {
    let production_id = match surrealdb::types::RecordId::parse_simple(&payload.production_id) {
        Ok(id) => id,
        Err(e) => {
            return Json(serde_json::json!({ "error": format!("Invalid production_id: {}", e) }))
                .into_response();
        }
    };

    // Dedup check
    match InvolvementModel::exists(&user.id, &production_id, payload.role.as_deref()).await {
        Ok(true) => {
            return Json(serde_json::json!({ "error": "This credit already exists" }))
                .into_response();
        }
        Ok(false) => {}
        Err(e) => {
            return Json(serde_json::json!({ "error": format!("Dedup check failed: {}", e) }))
                .into_response();
        }
    }

    match InvolvementModel::create(
        &user.id,
        &production_id,
        &payload.relation_type,
        payload.role.as_deref(),
        payload.department.as_deref(),
        payload.credit_type.as_deref(),
        "manual",
    )
    .await
    {
        Ok(involvement_id) => Json(serde_json::json!({
            "success": true,
            "involvement_id": involvement_id,
        }))
        .into_response(),
        Err(e) => {
            error!("Failed to create involvement: {}", e);
            Json(serde_json::json!({ "error": format!("Failed to create: {}", e) }))
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateInvolvementWithProductionRequest {
    title: String,
    production_type: String,
    relation_type: String,
    role: Option<String>,
    department: Option<String>,
    credit_type: Option<String>,
}

/// Create a new production and involvement edge atomically
async fn create_involvement_with_production(
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateInvolvementWithProductionRequest>,
) -> impl IntoResponse {
    use crate::models::production::CreateProductionData;

    // Create production (this also creates owner member_of edge)
    let production = match ProductionModel::create(
        CreateProductionData {
            title: payload.title,
            production_type: payload.production_type,
            status: "In Development".to_string(),
            start_date: None,
            end_date: None,
            description: None,
            location: None,
        },
        &user.id,
        "person",
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to create production: {}", e);
            return Json(serde_json::json!({ "error": format!("Failed to create production: {}", e) }))
                .into_response();
        }
    };

    // Create involvement edge
    match InvolvementModel::create(
        &user.id,
        &production.id,
        &payload.relation_type,
        payload.role.as_deref(),
        payload.department.as_deref(),
        payload.credit_type.as_deref(),
        "manual",
    )
    .await
    {
        Ok(involvement_id) => Json(serde_json::json!({
            "success": true,
            "involvement_id": involvement_id,
            "production_id": production.id.to_raw_string(),
            "production_slug": production.slug,
            "production_type": production.production_type,
        }))
        .into_response(),
        Err(e) => {
            error!("Failed to create involvement: {}", e);
            Json(serde_json::json!({ "error": format!("Failed to create involvement: {}", e) }))
                .into_response()
        }
    }
}

/// Delete an involvement edge (own credit or production owner can delete)
async fn delete_involvement(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    use surrealdb::types::RecordId;

    // Build full involvement record ID
    let involvement_id = if id.starts_with("involvement:") {
        id.clone()
    } else {
        format!("involvement:{}", id)
    };

    // Parse into RecordId for proper SurrealDB binding
    let inv_rid = if involvement_id.contains(':') {
        let parts: Vec<&str> = involvement_id.splitn(2, ':').collect();
        RecordId::new(parts[0], parts[1])
    } else {
        RecordId::new("involvement", involvement_id.as_str())
    };

    // Auth check: user must be the person on the involvement or owner of the production
    let query = r#"
        SELECT VALUE string::concat(meta::tb(in), ':', meta::id(in))
        FROM ONLY $rid
    "#;

    let mut result = match DB.query(query).bind(("rid", inv_rid)).await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to check involvement: {}", e);
            return Json(serde_json::json!({ "error": format!("Failed to check involvement: {}", e) }))
                .into_response();
        }
    };

    let person_id_str: Option<String> = match result.take(0) {
        Ok(r) => r,
        Err(e) => {
            error!("Involvement not found (deser): {}", e);
            return Json(serde_json::json!({ "error": format!("Involvement not found: {}", e) }))
                .into_response();
        }
    };

    let person_id_str = match person_id_str {
        Some(r) => r,
        None => {
            return Json(serde_json::json!({ "error": "Involvement not found" })).into_response();
        }
    };

    let user_full_id = if user.id.contains(':') {
        user.id.clone()
    } else {
        format!("person:{}", user.id)
    };
    let is_own = person_id_str == user.id || person_id_str == user_full_id;

    if !is_own {
        // Check if user is owner of the production
        if let Some(prod_id) = InvolvementModel::get_production_id(&involvement_id).await.ok().flatten() {
            match ProductionModel::can_edit(&prod_id, &user.id).await {
                Ok(true) => {} // allowed
                _ => {
                    return Json(serde_json::json!({ "error": "Unauthorized" })).into_response();
                }
            }
        } else {
            return Json(serde_json::json!({ "error": "Unauthorized" })).into_response();
        }
    }

    // Only delete the involvement edge, not the production
    match InvolvementModel::delete(&involvement_id).await {
        Ok(()) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => {
            error!("Failed to delete involvement: {}", e);
            Json(serde_json::json!({ "error": format!("Failed to delete: {}", e) }))
                .into_response()
        }
    }
}

// --- Credit Verification ---

/// Verify a credit (production owner only)
async fn verify_involvement(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let involvement_id = if id.starts_with("involvement:") {
        id.clone()
    } else {
        format!("involvement:{}", id)
    };

    // Auth: must be owner of the production this involvement points to
    let prod_id = match InvolvementModel::get_production_id(&involvement_id).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Json(serde_json::json!({ "error": "Involvement not found" })).into_response();
        }
        Err(e) => {
            return Json(serde_json::json!({ "error": format!("Lookup failed: {}", e) }))
                .into_response();
        }
    };

    match ProductionModel::can_edit(&prod_id, &user.id).await {
        Ok(true) => {}
        _ => {
            return Json(serde_json::json!({ "error": "Only production owners can verify credits" }))
                .into_response();
        }
    }

    match InvolvementModel::verify(&involvement_id, &user.id).await {
        Ok(()) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => {
            error!("Failed to verify involvement: {}", e);
            Json(serde_json::json!({ "error": format!("Failed to verify: {}", e) }))
                .into_response()
        }
    }
}

/// Reject a credit (production owner only)
async fn reject_involvement(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let involvement_id = if id.starts_with("involvement:") {
        id.clone()
    } else {
        format!("involvement:{}", id)
    };

    // Auth: must be owner of the production
    let prod_id = match InvolvementModel::get_production_id(&involvement_id).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Json(serde_json::json!({ "error": "Involvement not found" })).into_response();
        }
        Err(e) => {
            return Json(serde_json::json!({ "error": format!("Lookup failed: {}", e) }))
                .into_response();
        }
    };

    match ProductionModel::can_edit(&prod_id, &user.id).await {
        Ok(true) => {}
        _ => {
            return Json(serde_json::json!({ "error": "Only production owners can reject credits" }))
                .into_response();
        }
    }

    match InvolvementModel::reject(&involvement_id, &user.id).await {
        Ok(()) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => {
            error!("Failed to reject involvement: {}", e);
            Json(serde_json::json!({ "error": format!("Failed to reject: {}", e) }))
                .into_response()
        }
    }
}

// --- Feedback ---

#[derive(Debug, Deserialize)]
struct FeedbackRequest {
    page_url: String,
    message: String,
}

#[axum::debug_handler]
async fn submit_feedback(
    user: Option<Extension<Arc<CurrentUser>>>,
    Json(body): Json<FeedbackRequest>,
) -> impl IntoResponse {
    let message = body.message.trim().to_string();
    if message.is_empty() {
        return Json(serde_json::json!({ "error": "Message is required" }));
    }
    if message.len() > 2000 {
        return Json(serde_json::json!({ "error": "Message must be 2000 characters or less" }));
    }

    let username = user
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "anonymous".to_string());

    let page_url = body.page_url;
    debug!("Feedback from {} on {}", username, page_url);

    let sql = "INSERT INTO feedback (username, page_url, message) VALUES ($username, $page_url, $message)";
    if let Err(e) = DB
        .query(sql)
        .bind(("username", username.clone()))
        .bind(("page_url", page_url.clone()))
        .bind(("message", message.clone()))
        .await
    {
        error!("Failed to save feedback: {}", e);
        return Json(serde_json::json!({ "error": "Failed to save feedback" }));
    }

    // Fire-and-forget email notification
    let username_owned = username.clone();
    let page_url_owned = page_url.clone();
    let message_owned = message.clone();
    tokio::spawn(async move {
        match crate::services::email::EmailService::from_env() {
            Ok(email_service) => {
                if let Err(e) = email_service
                    .send_feedback_email(&username_owned, &page_url_owned, &message_owned)
                    .await
                {
                    error!("Failed to send feedback email: {}", e);
                }
            }
            Err(e) => {
                debug!("Email service not configured, skipping feedback email: {}", e);
            }
        }
    });

    info!("Feedback saved from {} on {}", username, page_url);
    Json(serde_json::json!({ "success": true }))
}

/// Fix avatar URLs by removing colons from paths (S3 path compatibility)
async fn fix_avatar_urls() -> impl IntoResponse {
    debug!("Fixing avatar URLs to remove colons from paths");

    // Update all person records that have avatar URLs containing "person:" in the path
    let sql = r#"
        UPDATE person
        SET profile.avatar = string::replace(profile.avatar, '/profiles/person:', '/profiles/')
        WHERE profile.avatar CONTAINS '/profiles/person:'
        RETURN <string> id AS id, profile.avatar AS avatar
    "#;

    match DB.query(sql).await {
        Ok(mut response) => {
            let updated: Vec<serde_json::Value> = response.take(0).unwrap_or_default();
            let count = updated.len();

            info!("Fixed {} avatar URLs", count);

            Json(serde_json::json!({
                "success": true,
                "message": format!("Fixed {} avatar URLs", count),
                "updated": count
            }))
        }
        Err(e) => {
            error!("Failed to fix avatar URLs: {}", e);
            Json(serde_json::json!({
                "error": format!("Failed to fix avatar URLs: {}", e)
            }))
        }
    }
}

// -----------------------------------------------------------------------------
// People Search (for invite autocomplete)
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
struct PeopleSearchQuery {
    q: Option<String>,
}

/// Lightweight people search for invite autocomplete.
/// Returns up to 8 matches by name, username, or email.
#[axum::debug_handler]
async fn people_search(
    _user: AuthenticatedUser,
    Query(params): Query<PeopleSearchQuery>,
) -> impl IntoResponse {
    use surrealdb::types::SurrealValue;

    let query = match params.q.filter(|q| q.len() >= 2) {
        Some(q) => q,
        None => return Json(serde_json::json!({ "results": [] })),
    };

    let query_lower = query.to_lowercase();

    #[derive(Debug, Deserialize, SurrealValue)]
    struct PersonHit {
        id: String,
        name: Option<String>,
        username: String,
        avatar_url: Option<String>,
    }

    let sql = "SELECT
            <string> id AS id,
            name,
            username,
            profile.avatar AS avatar_url
        FROM person
        WHERE
            string::lowercase(name ?? '') CONTAINS $q
            OR string::lowercase(username ?? '') CONTAINS $q
        LIMIT 8";

    let results: Vec<PersonHit> = match DB.query(sql).bind(("q", query_lower)).await {
        Ok(mut resp) => resp.take(0).unwrap_or_default(),
        Err(e) => {
            error!("People search failed: {}", e);
            return Json(serde_json::json!({ "results": [] }));
        }
    };

    let items: Vec<serde_json::Value> = results
        .into_iter()
        .map(|p| {
            let display_name = p.name.unwrap_or_else(|| p.username.clone());
            let initials = display_name
                .split_whitespace()
                .filter_map(|w| w.chars().next())
                .take(2)
                .collect::<String>()
                .to_uppercase();
            serde_json::json!({
                "id": p.id,
                "username": p.username,
                "name": display_name,
                "initials": initials,
                "avatar_url": p.avatar_url,
            })
        })
        .collect();

    Json(serde_json::json!({ "results": items }))
}

// -----------------------------------------------------------------------------
// Username Availability Check
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
struct CheckUsernameQuery {
    username: Option<String>,
}

#[axum::debug_handler]
async fn check_username(
    Query(params): Query<CheckUsernameQuery>,
) -> impl IntoResponse {
    use crate::models::person::{Person, validate_username};

    let username = match params.username {
        Some(u) => u,
        None => return Json(serde_json::json!({ "available": false, "error": "Username is required" })),
    };

    // Validate format
    let username = match validate_username(&username) {
        Ok(u) => u,
        Err(e) => return Json(serde_json::json!({ "available": false, "error": e.to_string() })),
    };

    // Check availability in DB
    match Person::find_by_username(&username).await {
        Ok(Some(_)) => Json(serde_json::json!({ "available": false, "error": "Username is already taken" })),
        Ok(None) => Json(serde_json::json!({ "available": true, "error": null })),
        Err(_) => Json(serde_json::json!({ "available": false, "error": "Unable to check username" })),
    }
}

// -----------------------------------------------------------------------------
// Dynamic OG Profile Image (1200x630)
// -----------------------------------------------------------------------------

/// Generates a branded 1200x630 PNG for social media link previews.
/// Embeds the person's avatar, name, headline, and a CTA.
#[axum::debug_handler]
async fn og_profile_image(
    Path(username): Path<String>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    use crate::models::person::Person;

    let person = Person::find_by_username(&username)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "Not found".to_string()))?;

    // Fetch avatar bytes — resolve to absolute URL and fetch via HTTP
    let avatar_url = person.get_absolute_avatar_url();
    debug!("OG image: avatar_url = {:?}", avatar_url);
    let avatar_bytes: Option<Vec<u8>> = if let Some(ref url) = avatar_url {
        debug!("OG image: fetching {}", url);
        match reqwest::get(url).await {
            Ok(resp) => {
                debug!("OG image: response status {}", resp.status());
                match resp.bytes().await {
                    Ok(b) => {
                        debug!("OG image: fetched {} bytes", b.len());
                        if b.is_empty() { None } else { Some(b.to_vec()) }
                    }
                    Err(e) => {
                        error!("OG image: failed to read response bytes: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                error!("OG image: failed to fetch avatar: {}", e);
                None
            }
        }
    } else {
        debug!("OG image: no avatar URL set");
        None
    };
    debug!("OG image: avatar_bytes len = {:?}", avatar_bytes.as_ref().map(|b| b.len()));

    // Render profile image + logo to PNG
    let png_data = render_og_png(avatar_bytes.as_deref())
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/jpeg"),
            (axum::http::header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        png_data,
    ))
}

fn render_og_png(avatar_bytes: Option<&[u8]>) -> Result<Vec<u8>, String> {
    const W: u32 = 1200;
    const H: u32 = 630;

    // Decode avatar into an RGBA buffer
    let avatar_rgba = avatar_bytes.and_then(|bytes| {
        match image::load_from_memory(bytes) {
            Ok(img) => {
                tracing::debug!("OG image: decoded avatar {}x{}", img.width(), img.height());
                Some(img)
            }
            Err(e) => {
                tracing::error!("OG image: failed to decode avatar: {}", e);
                None
            }
        }
    });

    // Create the output image
    let mut canvas = image::RgbaImage::new(W, H);

    // Fill canvas with page background color rgb(24, 24, 24)
    for pixel in canvas.pixels_mut() {
        *pixel = image::Rgba([24, 24, 24, 255]);
    }

    // Profile image: fit to height, left-aligned, preserve aspect ratio
    let avatar_right_edge = if let Some(avatar) = avatar_rgba {
        let src = avatar.to_rgba8();
        let scale = H as f32 / src.height() as f32;
        let scaled_w = (src.width() as f32 * scale).round() as u32;
        let resized = image::imageops::resize(&src, scaled_w, H, image::imageops::FilterType::Lanczos3);
        image::imageops::overlay(&mut canvas, &resized, 0, 0);
        scaled_w
    } else {
        0
    };

    // SlateHub logo: vertically centered in the space to the right of the profile image
    {
        let logo_svg = include_str!("../../static/images/logo.svg").to_string();
        let scale = 2.4_f32;
        let logo_w = (103.0 * scale) as u32;
        let logo_h = (16.0 * scale) as u32;
        let opts = resvg::usvg::Options::default();
        if let Ok(tree) = resvg::usvg::Tree::from_str(&logo_svg, &opts) {
            if let Some(mut logo_pixmap) = tiny_skia::Pixmap::new(logo_w, logo_h) {
                let transform = tiny_skia::Transform::from_scale(scale, scale);
                resvg::render(&tree, transform, &mut logo_pixmap.as_mut());
                if let Some(logo_img) = image::RgbaImage::from_raw(logo_w, logo_h, logo_pixmap.data().to_vec()) {
                    // Center logo in the remaining space to the right
                    let right_space_start = avatar_right_edge;
                    let right_space_w = W - right_space_start;
                    let x = (right_space_start + (right_space_w - logo_w) / 2) as i64;
                    let y = ((H - logo_h) / 2) as i64;
                    image::imageops::overlay(&mut canvas, &logo_img, x, y);
                }
            }
        }
    }

    // Encode to JPEG at 80% quality (keeps it well under 600KB)
    let rgb = image::DynamicImage::ImageRgba8(canvas).into_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80);
    rgb.write_with_encoder(encoder)
        .map_err(|e| format!("JPEG encode error: {}", e))?;

    Ok(buf.into_inner())
}

/// Generates a QR code PNG for a user's profile URL.
async fn qr_profile_image(
    Path(username): Path<String>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    use crate::models::person::Person;
    use qrcode::QrCode;

    // Verify user exists
    Person::find_by_username(&username)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "Not found".to_string()))?;

    let profile_url = format!("{}/{}", crate::config::app_url(), username);
    debug!("QR code: generating for {}", profile_url);

    let code = QrCode::new(profile_url.as_bytes())
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("QR encode error: {}", e)))?;

    // Render QR matrix to image manually (qrcode image feature needs image 0.25)
    let matrix = code.to_colors();
    let module_count = code.width() as u32;
    let quiet_zone = 4_u32;
    let total_modules = module_count + quiet_zone * 2;
    let scale = (400 / total_modules).max(1);
    let img_size = total_modules * scale;

    let mut qr_image = image::GrayImage::from_pixel(img_size, img_size, image::Luma([255u8]));
    for (i, color) in matrix.iter().enumerate() {
        let x = (i as u32 % module_count) + quiet_zone;
        let y = (i as u32 / module_count) + quiet_zone;
        if *color == qrcode::Color::Dark {
            for dy in 0..scale {
                for dx in 0..scale {
                    qr_image.put_pixel(x * scale + dx, y * scale + dy, image::Luma([0u8]));
                }
            }
        }
    }

    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(qr_image)
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("PNG encode error: {}", e)))?;

    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/png"),
            (axum::http::header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        buf.into_inner(),
    ))
}
