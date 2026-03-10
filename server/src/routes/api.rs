use axum::{
    Json, Router,
    extract::{Path, Query},
    response::{IntoResponse, Redirect},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info};

use crate::db::DB;
use crate::middleware::AuthenticatedUser;
use crate::models::involvement::InvolvementModel;
use crate::models::production::ProductionModel;
use crate::models::system::System;
use crate::record_id_ext::RecordIdExt;

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(stats))
        .route("/avatar", get(avatar))
        .route("/debug/user", get(debug_user))
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
    let person_record = if id.starts_with("person:") {
        id.to_string()
    } else {
        format!("person:{}", id)
    };

    // Query for the person's avatar URL
    let sql = format!("SELECT profile.avatar FROM {} LIMIT 1", person_record);

    if let Ok(mut response) = DB.query(&sql).await {
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
    }

    // Fall back to DiceBear for deterministic avatars based on user ID
    let avatar_url = format!(
        "https://api.dicebear.com/7.x/initials/svg?seed={}&backgroundColor=4f46e5",
        id
    );

    Redirect::permanent(&avatar_url)
}

#[axum::debug_handler]
async fn debug_user(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    use crate::models::person::Person;

    let username = params
        .get("username")
        .map(|s| s.as_str())
        .unwrap_or("chris");
    debug!("Debug: Looking up user: {}", username);

    let mut query_results = Vec::new();

    // Test 1: Find by username
    debug!("Test 1: Person::find_by_username({})", username);
    match Person::find_by_username(username).await {
        Ok(Some(person)) => {
            query_results.push(serde_json::json!({
                "method": "Person::find_by_username",
                "success": true,
                "data": serde_json::json!({
                    "id": person.id.to_raw_string(),
                    "username": person.username,
                    "email": person.email,
                    "has_profile": person.profile.is_some()
                })
            }));
        }
        Ok(None) => {
            query_results.push(serde_json::json!({
                "method": "Person::find_by_username",
                "success": true,
                "data": null,
                "message": "No user found"
            }));
        }
        Err(e) => {
            query_results.push(serde_json::json!({
                "method": "Person::find_by_username",
                "success": false,
                "error": format!("Query failed: {}", e)
            }));
        }
    }

    // Test 2: Find by identifier (can be username or email)
    debug!("Test 2: Person::find_by_identifier({})", username);
    match Person::find_by_identifier(username).await {
        Ok(Some(person)) => {
            query_results.push(serde_json::json!({
                "method": "Person::find_by_identifier",
                "success": true,
                "data": serde_json::json!({
                    "id": person.id.to_raw_string(),
                    "username": person.username,
                    "email": person.email,
                    "has_profile": person.profile.is_some()
                })
            }));
        }
        Ok(None) => {
            query_results.push(serde_json::json!({
                "method": "Person::find_by_identifier",
                "success": true,
                "data": null,
                "message": "No user found"
            }));
        }
        Err(e) => {
            query_results.push(serde_json::json!({
                "method": "Person::find_by_identifier",
                "success": false,
                "error": format!("Query failed: {}", e)
            }));
        }
    }

    // Test 3: Get all users (limited for debugging)
    debug!("Test 3: Person::get_paginated(limit=5)");
    match Person::get_paginated(5, 0).await {
        Ok(persons) => {
            query_results.push(serde_json::json!({
                "method": "Person::get_paginated",
                "success": true,
                "count": persons.len(),
                "data": persons.iter().map(|p| {
                    serde_json::json!({
                        "id": p.id.to_raw_string(),
                        "username": p.username,
                        "email": p.email
                    })
                }).collect::<Vec<_>>()
            }));
        }
        Err(e) => {
            query_results.push(serde_json::json!({
                "method": "Person::get_paginated",
                "success": false,
                "error": format!("Query failed: {}", e)
            }));
        }
    }

    Json(serde_json::json!({
        "username": username,
        "tests": query_results
    }))
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

/// Fix avatar URLs by removing colons from paths (S3 path compatibility)
async fn fix_avatar_urls() -> impl IntoResponse {
    debug!("Fixing avatar URLs to remove colons from paths");

    // Update all person records that have avatar URLs containing "person:" in the path
    let sql = r#"
        UPDATE person
        SET profile.avatar = string::replace(profile.avatar, '/profiles/person:', '/profiles/')
        WHERE profile.avatar CONTAINS '/profiles/person:'
        RETURN AFTER
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
