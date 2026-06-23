//! Page-view activity tracking middleware.
//!
//! [`activity_middleware`] is the innermost middleware in the stack built by
//! [`crate::routes::app`]: on the request path it runs after the auth
//! middleware, so the `Arc<CurrentUser>` extension is already populated. It
//! inserts nothing into the request itself; once the handler responds, it
//! records a `page_view` activity event through
//! `crate::services::activity::log_activity` for successful GET requests to
//! user-facing pages, skipping static assets, API routes, and crawler
//! endpoints.

use axum::{extract::Request, middleware::Next, response::Response};

use super::auth::CurrentUser;
use std::sync::Arc;

/// Log a `page_view` activity event for each successful GET request.
///
/// Reads the optional `Arc<CurrentUser>` extension inserted by
/// [`super::auth::auth_middleware`], so it must be layered inside the auth
/// middleware; anonymous views are recorded without a user ID. Only GET
/// requests that complete with a 2xx status and target a trackable path
/// (not static assets, `/api/`, or crawler endpoints) are recorded.
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

/// Decide whether a path represents a user-facing page worth tracking.
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
