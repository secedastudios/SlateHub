use axum::{
    Json, Router,
    extract::{Path, Query, Request},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{debug, error, info};

use crate::db::DB;
use crate::error::Error;
use crate::middleware::{ErrorWithContext, RequestIdExt};
use crate::models::system::System;

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(stats))
        .route("/avatar", get(avatar))
        .route("/debug/user", get(debug_user))
        .route("/fix-avatar-urls", post(fix_avatar_urls))
        .route("/test-error/{code}", get(test_error))
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
    projects: usize,
    users: usize,
    connections: usize,
}

#[axum::debug_handler]
async fn stats() -> impl IntoResponse {
    debug!("Stats endpoint called");

    // Use the System model to get actual counts
    let projects = System::count_records("production").await.unwrap_or(0);
    let users = System::count_records("person").await.unwrap_or(0);
    let connections = System::count_records("involvement").await.unwrap_or(0);

    let stats = PlatformStats {
        projects,
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
                    "id": person.id.to_string(),
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
                    "id": person.id.to_string(),
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
                        "id": p.id.to_string(),
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

/// Fix avatar URLs by removing colons from paths (MinIO compatibility)
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

/// Test route to demonstrate error page rendering
/// Access /api/test-error/404, /api/test-error/401, /api/test-error/500, etc.
async fn test_error(Path(code): Path<u16>, headers: HeaderMap, req: Request) -> Response {
    let request_id = req.request_id().map(|id| id.to_string());
    let path = req.uri().path().to_string();

    let error = match code {
        404 => Error::NotFound,
        401 => Error::Unauthorized,
        403 => Error::Forbidden,
        500 => Error::Internal("Test internal server error".to_string()),
        400 => Error::BadRequest("Test bad request error".to_string()),
        409 => Error::Conflict("Test conflict error".to_string()),
        422 => Error::Validation("Test validation error".to_string()),
        502 => Error::ExternalService("Test external service error".to_string()),
        _ => Error::Internal(format!("Test error with code {}", code)),
    };

    error.with_context(&headers, Some(path), request_id)
}
