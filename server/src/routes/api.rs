use axum::{Json, Router, routing::get};
use serde::Serialize;
use tracing::{debug, info};

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(stats))
}

#[derive(Serialize)]
struct HealthStatus {
    status: String,
    database: String,
    version: String,
    timestamp: String,
}

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
