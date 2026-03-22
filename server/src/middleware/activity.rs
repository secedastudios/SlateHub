use axum::{extract::Request, middleware::Next, response::Response};

use super::auth::CurrentUser;
use std::sync::Arc;

/// Middleware that logs page view activity events for successful GET requests.
/// Must run after auth middleware so user identity is available.
pub async fn activity_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // Extract user ID if authenticated
    let user_id: Option<String> = request
        .extensions()
        .get::<Arc<CurrentUser>>()
        .map(|u| u.id.clone());

    let response = next.run(request).await;

    // Only log successful GET requests to non-static, non-API paths
    if method == axum::http::Method::GET && response.status().is_success() && should_track(&path) {
        crate::services::activity::log_activity(user_id.as_deref(), "page_view", &path);
    }

    response
}

fn should_track(path: &str) -> bool {
    !path.starts_with("/static/")
        && !path.starts_with("/api/")
        && !path.starts_with("/favicon")
        && !path.starts_with("/robots")
        && !path.starts_with("/sitemap")
        && !path.starts_with("/llms")
        && !path.starts_with("/healthcheck")
        && !path.starts_with("/mcp")
}
