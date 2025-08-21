use axum::{Router, routing::get};

pub fn app() -> Router {
    Router::new()
        .route("/", get(root))
}

async fn root() -> &'static str {
    "Hello, world!"
}
