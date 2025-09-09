use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use tracing::{Instrument, info_span};
use uuid::Uuid;

/// Extension type for the request ID
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    /// Create a new request ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Get the ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Middleware that adds a unique request ID to each request
pub async fn request_id_middleware(mut request: Request<Body>, next: Next) -> Response {
    // Generate a new request ID
    let request_id = RequestId::new();
    let id_str = request_id.as_str().to_string();

    // Add the request ID to request extensions so it can be accessed by handlers
    request.extensions_mut().insert(request_id.clone());

    // Create a tracing span with the request ID
    let span = info_span!(
        "request",
        request_id = %id_str,
        method = %request.method(),
        uri = %request.uri(),
        version = ?request.version(),
    );

    // Log the start of the request with the ID
    let _enter = span.enter();
    tracing::info!(
        "Starting request {} - {} {}",
        id_str,
        request.method(),
        request.uri()
    );
    drop(_enter);

    // Process the request within the span
    let mut response = next.run(request).instrument(span.clone()).await;

    // Add the request ID to the response headers for debugging
    if let Ok(header_value) = HeaderValue::from_str(&id_str) {
        response.headers_mut().insert("X-Request-Id", header_value);
    }

    // Log the completion of the request
    let _enter = span.enter();
    tracing::info!(
        "Completed request {} - Status: {}",
        id_str,
        response.status()
    );

    response
}

/// Extension trait to easily get the request ID from a request
pub trait RequestIdExt {
    fn request_id(&self) -> Option<&RequestId>;
}

impl<T> RequestIdExt for Request<T> {
    fn request_id(&self) -> Option<&RequestId> {
        self.extensions().get::<RequestId>()
    }
}
