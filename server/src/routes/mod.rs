use axum::http::{Request, Response};
use axum::{Router, middleware, routing::get_service};
use std::time::Duration;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};
use tracing::{error, info};

use crate::middleware::{auth_middleware, logging_middleware};

mod api;
mod auth;
mod pages;

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
        // Mount API routes under /api
        .nest("/api", api::router())
        // Static files
        .nest_service("/static", get_service(static_service))
        // Apply auth middleware to extract user from JWT cookies
        // TODO: Fix middleware signatures and re-enable
        // .layer(middleware::from_fn(auth_middleware))
        // Apply custom logging middleware for detailed request/response logging
        // .layer(middleware::from_fn(logging_middleware))
        // Middleware
        .layer(CompressionLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        version = ?request.version(),
                    )
                })
                .on_request(|request: &Request<_>, _span: &tracing::Span| {
                    info!(
                        "Started processing request: {} {}",
                        request.method(),
                        request.uri()
                    );
                })
                .on_response(
                    |response: &Response<_>, latency: Duration, _span: &tracing::Span| {
                        info!(
                            "Finished processing request: status={}, latency={:?}",
                            response.status(),
                            latency
                        );
                    },
                )
                .on_failure(
                    |error: tower_http::classify::ServerErrorsFailureClass,
                     latency: Duration,
                     _span: &tracing::Span| {
                        error!("Request failed: error={:?}, latency={:?}", error, latency);
                    },
                ),
        )
}
