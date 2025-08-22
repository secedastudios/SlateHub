use axum::{Router, routing::get_service};
use tower_http::{
    compression::CompressionLayer,
    services::ServeDir,
    trace::{self, TraceLayer},
};
use tracing::Level;

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
