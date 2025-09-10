use axum::http::{Request, Response};
use axum::{Router, middleware, routing::get_service};
use std::time::Duration;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};
use tracing::{Span, error, info};

use crate::middleware::{RequestIdExt, auth_middleware, request_id_middleware};

mod api;
mod auth;
mod pages;
mod profile;

pub fn app() -> Router {
    // Static file service
    let static_service = ServeDir::new("static")
        .append_index_html_on_directories(false)
        .precompressed_gzip()
        .precompressed_br();

    Router::new()
        // Mount the page routes at the root
        .merge(pages::router())
        // Mount auth routes
        .merge(auth::router())
        // Mount profile routes
        .merge(profile::router())
        // Mount API routes under /api
        .nest("/api", api::router())
        // Static files
        .nest_service("/static", get_service(static_service))
        // Apply auth middleware to extract user from JWT cookies
        .layer(middleware::from_fn(auth_middleware))
        // Apply request ID middleware early in the stack
        .layer(middleware::from_fn(request_id_middleware))
        // Middleware
        .layer(CompressionLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    // Try to get the request ID if it exists
                    let request_id = request
                        .request_id()
                        .map(|id| id.as_str())
                        .unwrap_or("unknown");

                    tracing::info_span!(
                        "http",
                        request_id = %request_id,
                        method = %request.method(),
                        uri = %request.uri(),
                        version = ?request.version(),
                    )
                })
                .on_request(|request: &Request<_>, span: &Span| {
                    let request_id = request
                        .request_id()
                        .map(|id| id.as_str())
                        .unwrap_or("unknown");

                    span.record("request_id", &request_id);

                    info!(
                        request_id = %request_id,
                        method = %request.method(),
                        uri = %request.uri(),
                        "→ Request started"
                    );
                })
                .on_response(|response: &Response<_>, latency: Duration, _span: &Span| {
                    info!(
                        status = %response.status(),
                        latency = ?latency,
                        "← Request completed"
                    );
                })
                .on_failure(
                    |error: tower_http::classify::ServerErrorsFailureClass,
                     latency: Duration,
                     _span: &Span| {
                        error!(
                            error = ?error,
                            latency = ?latency,
                            "✗ Request failed"
                        );
                    },
                ),
        )
}
