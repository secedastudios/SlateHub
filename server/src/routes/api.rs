use axum::{
    Json, Router,
    extract::Query,
    response::{IntoResponse, Redirect},
    routing::get,
};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{debug, info};

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(stats))
        .route("/avatar", get(avatar))
        .route("/debug/user", get(debug_user))
}

#[derive(Serialize)]
struct HealthStatus {
    status: String,
    database: String,
    version: String,
    timestamp: String,
}

#[axum::debug_handler]
async fn health_check() -> Json<HealthStatus> {
    debug!("Health check requested");

    // Check database connectivity
    let db_status = match crate::db::DB.health().await {
        Ok(_) => {
            info!("Database health check: OK");
            "connected"
        }
        Err(e) => {
            tracing::warn!("Database health check failed: {:?}", e);
            "disconnected"
        }
    };

    let health = HealthStatus {
        status: "healthy".to_string(),
        database: db_status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    info!(
        "Health check complete: status={}, db={}",
        health.status, health.database
    );

    Json(health)
}

#[derive(Serialize)]
struct PlatformStats {
    projects: u32,
    users: u32,
    connections: u32,
}

#[axum::debug_handler]
async fn stats() -> Json<PlatformStats> {
    debug!("Stats endpoint called");

    // In production, these would be fetched from the database
    // For now, return mock data
    let stats = PlatformStats {
        projects: 1247,
        users: 5892,
        connections: 18453,
    };

    Json(stats)
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
