use axum::extract::DefaultBodyLimit;
use axum::http::{Method, Request, Response, header, HeaderValue};
use axum::{Router, middleware, routing::get_service};
use std::time::Duration;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing::{Span, error, info};

use crate::middleware::{
    RequestIdExt, auth_middleware, error_response_middleware, request_id_middleware,
};

mod account;
mod admin;
mod analytics;
mod api;
mod auth;
mod equipment;
mod jobs;
mod likes;
mod locations;
mod media;
mod messages;
mod notifications;
mod organizations;
mod pages;
mod productions;
mod profile;
mod public_profiles;
mod search;
mod verification;

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
        // Mount search routes
        .merge(search::router())
        // Mount organizations routes
        .merge(organizations::router())
        // Mount productions routes
        .merge(productions::router())
        // Mount jobs routes
        .merge(jobs::router())
        // Mount likes routes
        .merge(likes::router())
        // Mount locations routes
        .merge(locations::router())
        // Mount notifications routes
        .merge(notifications::router())
        // Mount messages routes
        .merge(messages::router())
        // Mount equipment routes
        .merge(equipment::router())
        // Mount analytics routes (before profile to avoid /{username} conflict)
        .merge(analytics::router())
        // Mount profile routes
        .merge(profile::router())
        // Mount verification routes
        .merge(verification::router())
        // Mount account settings routes
        .merge(account::router())
        // Mount admin routes
        .merge(admin::router())
        // Mount API routes under /api
        .nest("/api", api::router())
        // Mount media routes under /api/media
        .nest("/api/media", media::router())
        // Mount MCP server for AI tool access
        .nest_service("/mcp", crate::mcp::create_mcp_service())
        // Raise body limit to 50MB to support script uploads (individual handlers enforce their own limits)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        // Static files — long cache with immutable (URLs include ?v= cache buster)
        .nest_service(
            "/static",
            get_service(static_service).layer(
                SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_static("public, max-age=31536000, immutable"),
                ),
            ),
        )
        // Mount public profiles last to handle /<username> routes
        // This must be last to avoid conflicts with other routes
        .merge(public_profiles::router())
        // Track page view activity (runs after auth so user identity is available)
        .layer(middleware::from_fn(crate::middleware::activity::activity_middleware))
        // Apply auth middleware to extract user from JWT cookies
        .layer(middleware::from_fn(auth_middleware))
        // Error response middleware - converts errors to HTML/JSON based on Accept header
        .layer(middleware::from_fn(error_response_middleware))
        // Security headers
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ))
        // CORS — allow Chrome extension origins to call /api/* endpoints
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::predicate(|origin, _| {
                    origin.as_bytes().starts_with(b"chrome-extension://")
                }))
                .allow_methods([Method::GET, Method::POST, Method::DELETE])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
                .max_age(Duration::from_secs(3600)),
        )
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
        // Apply request ID middleware at the bottom of the stack so it runs first
        // This ensures the request ID is available to all other middleware
        .layer(middleware::from_fn(request_id_middleware))
}
