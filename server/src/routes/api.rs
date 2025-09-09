use axum::{
    Json, Router,
    extract::Query,
    response::{IntoResponse, Redirect},
    routing::get,
};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{debug, info};

use crate::models::system::System;

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(stats))
        .route("/avatar", get(avatar))
        .route("/debug/user", get(debug_user))
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

    // For now, redirect to a default avatar service
    // In production, you might:
    // 1. Check if user has uploaded a custom avatar
    // 2. Generate based on email hash (Gravatar style)
    // 3. Return a stored image from MinIO

    // Using DiceBear for deterministic avatars based on user ID
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
