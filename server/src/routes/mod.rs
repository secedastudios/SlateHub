use axum::{
    Json, Router,
    response::Html,
    routing::{get, get_service},
};
use serde::Serialize;
use tower_http::{
    compression::CompressionLayer,
    services::ServeDir,
    trace::{self, TraceLayer},
};
use tracing::{Level, debug, error, info};

use crate::{error::Error, templates};

pub fn app() -> Router {
    // Static file service
    let static_service = ServeDir::new("static")
        .append_index_html_on_directories(false)
        .precompressed_gzip()
        .precompressed_br();

    Router::new()
        // Page routes
        .route("/", get(index))
        .route("/projects", get(projects))
        .route("/people", get(people))
        .route("/about", get(about))
        // API routes
        .route("/api/health", get(health_check))
        .route("/api/stats", get(stats))
        // Static files
        .nest_service("/static", get_service(static_service))
        // Middleware
        .layer(CompressionLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_request(trace::DefaultOnRequest::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO))
                .on_failure(trace::DefaultOnFailure::new().level(Level::ERROR)),
        )
}

// Page handlers

async fn index() -> Result<Html<String>, Error> {
    debug!("Rendering index page");

    let mut context = templates::base_context();
    context.insert("active_page", "home");

    let html = templates::render_with_context("index.html", &context).map_err(|e| {
        error!("Failed to render index template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn projects() -> Result<Html<String>, Error> {
    debug!("Rendering projects page");

    let mut context = templates::base_context();
    context.insert("active_page", "projects");

    let html = templates::render_with_context("projects.html", &context).map_err(|e| {
        error!("Failed to render projects template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn people() -> Result<Html<String>, Error> {
    debug!("Rendering people page");

    let mut context = templates::base_context();
    context.insert("active_page", "people");

    let html = templates::render_with_context("people.html", &context).map_err(|e| {
        error!("Failed to render people template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn about() -> Result<Html<String>, Error> {
    debug!("Rendering about page");

    let mut context = templates::base_context();
    context.insert("active_page", "about");

    let html = templates::render_with_context("about.html", &context).map_err(|e| {
        error!("Failed to render about template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

// API handlers

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

// Error handling is now centralized in src/error.rs
