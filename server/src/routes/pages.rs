use axum::{Router, extract::Request, response::Html, routing::get};

use tracing::{debug, error};

use crate::{error::Error, middleware::UserExtractor, templates};

pub fn router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/projects", get(projects))
        .route("/people", get(people))
        .route("/about", get(about))
}

async fn index(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering index page");

    let mut context = templates::base_context();
    context.insert("active_page", "home");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        context.insert(
            "user",
            &serde_json::json!({
                "id": user.id,
                "name": user.username,
                "email": user.email,
                "avatar": format!("/api/avatar?id={}", user.id)
            }),
        );
    }

    // Add static stats data (in production, fetch from database)
    context.insert("project_count", &1247);
    context.insert("user_count", &5892);
    context.insert("connection_count", &18453);

    // Add sample activities (in production, fetch from database)
    let activities = vec![
        serde_json::json!({
            "user": "Sarah Johnson",
            "action": "created a new project",
            "time": "2 minutes ago"
        }),
        serde_json::json!({
            "user": "Mike Chen",
            "action": "joined the platform",
            "time": "15 minutes ago"
        }),
        serde_json::json!({
            "user": "Emily Rodriguez",
            "action": "posted a job opening",
            "time": "1 hour ago"
        }),
        serde_json::json!({
            "user": "David Kim",
            "action": "completed a collaboration",
            "time": "3 hours ago"
        }),
        serde_json::json!({
            "user": "Lisa Thompson",
            "action": "updated their portfolio",
            "time": "5 hours ago"
        }),
    ];
    context.insert("activities", &activities);

    let html = templates::render_with_context("index.html", &context).map_err(|e| {
        error!("Failed to render index template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn projects(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering projects page");

    let mut context = templates::base_context();
    context.insert("active_page", "projects");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        context.insert(
            "user",
            &serde_json::json!({
                "id": user.id,
                "name": user.username,
                "email": user.email,
                "avatar": format!("/api/avatar?id={}", user.id)
            }),
        );
    }

    let html = templates::render_with_context("projects.html", &context).map_err(|e| {
        error!("Failed to render projects template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn people(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering people page");

    let mut context = templates::base_context();
    context.insert("active_page", "people");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        context.insert(
            "user",
            &serde_json::json!({
                "id": user.id,
                "name": user.username,
                "email": user.email,
                "avatar": format!("/api/avatar?id={}", user.id)
            }),
        );
    }

    let html = templates::render_with_context("people.html", &context).map_err(|e| {
        error!("Failed to render people template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn about(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering about page");

    let mut context = templates::base_context();
    context.insert("active_page", "about");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        context.insert(
            "user",
            &serde_json::json!({
                "id": user.id,
                "name": user.username,
                "email": user.email,
                "avatar": format!("/api/avatar?id={}", user.id)
            }),
        );
    }

    let html = templates::render_with_context("about.html", &context).map_err(|e| {
        error!("Failed to render about template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}
