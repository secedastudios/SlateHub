use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use tracing::{Instrument, info_span};
use ulid::Ulid;

/// Extension type for the request ID
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    /// Create a new request ID
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    /// Create a request ID from an existing string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Middleware that adds a unique request ID to each request
///
/// This middleware will:
/// 1. Check for existing request IDs from upstream proxies/load balancers
/// 2. Generate a new ULID if no existing ID is found
/// 3. Add the ID to request extensions for use by handlers
/// 4. Include the ID in response headers
/// 5. Add the ID to tracing spans for correlated logging
pub async fn request_id_middleware(mut request: Request<Body>, next: Next) -> Response {
    // Check for existing request ID from various common headers
    // Priority order: X-Request-Id, X-Correlation-Id, X-Trace-Id, Request-Id
    let request_id = extract_existing_request_id(&request).unwrap_or_else(|| RequestId::new());

    let id_str = request_id.as_str().to_string();

    // Add the request ID to request extensions so it can be accessed by handlers
    request.extensions_mut().insert(request_id.clone());

    // Create a tracing span with the request ID
    let span = info_span!(
        "request",
        request_id = %id_str,
        method = %request.method(),
        uri = %request.uri(),
        path = %request.uri().path(),
        version = ?request.version(),
    );

    // Log the start of the request with the ID
    let _enter = span.enter();
    tracing::info!(
        request_id = %id_str,
        method = %request.method(),
        uri = %request.uri(),
        "→ Request started"
    );
    drop(_enter);

    // Process the request within the span
    let mut response = next.run(request).instrument(span.clone()).await;

    // Add the request ID to the response headers for debugging
    // This helps with tracing requests through multiple services
    if let Ok(header_value) = HeaderValue::from_str(&id_str) {
        response.headers_mut().insert("X-Request-Id", header_value);
    }

    // Log the completion of the request
    let _enter = span.enter();
    tracing::info!(
        request_id = %id_str,
        status = %response.status(),
        status_code = response.status().as_u16(),
        "← Request completed"
    );

    response
}

/// Extract an existing request ID from common headers
///
/// Checks the following headers in order:
/// - X-Request-Id (most common)
/// - X-Correlation-Id (Azure/Microsoft services)
/// - X-Trace-Id (some tracing systems)
/// - Request-Id (some legacy systems)
fn extract_existing_request_id(request: &Request<Body>) -> Option<RequestId> {
    // List of headers to check for existing request IDs
    const REQUEST_ID_HEADERS: &[&str] = &[
        "x-request-id",
        "x-correlation-id",
        "x-trace-id",
        "request-id",
    ];

    for header_name in REQUEST_ID_HEADERS {
        if let Some(header_value) = request.headers().get(*header_name) {
            if let Ok(id_str) = header_value.to_str() {
                // Validate the ID format (should be a valid ULID, UUID, or alphanumeric string)
                if is_valid_request_id(id_str) {
                    tracing::debug!(
                        header = header_name,
                        request_id = %id_str,
                        "Using existing request ID from header"
                    );
                    return Some(RequestId::from_string(id_str.to_string()));
                } else {
                    tracing::warn!(
                        header = header_name,
                        value = %id_str,
                        "Invalid request ID format in header, generating new ID"
                    );
                }
            }
        }
    }

    None
}

/// Validate that a request ID has a reasonable format
///
/// Accepts:
/// - ULIDs (26 character base32 strings)
/// - UUIDs (with or without hyphens)
/// - Alphanumeric strings with hyphens, underscores, and dots
/// - Length between 8 and 128 characters
fn is_valid_request_id(id: &str) -> bool {
    // Check length
    if id.len() < 8 || id.len() > 128 {
        return false;
    }

    // Check if it's a valid ULID (26 character base32 string)
    if id.len() == 26 {
        // ULID uses Crockford's Base32 (0-9, A-Z excluding I, L, O)
        if id.chars().all(|c| {
            c.is_ascii_digit() || (c.is_ascii_uppercase() && c != 'I' && c != 'L' && c != 'O')
        }) {
            return true;
        }
    }

    // Check if it's a valid UUID (with or without hyphens)
    if id.len() == 32 || id.len() == 36 {
        let id_no_hyphens = id.replace('-', "");
        if id_no_hyphens.len() == 32 && id_no_hyphens.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }
    }

    // Otherwise, accept alphanumeric strings with some common separators
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Extension trait to easily get the request ID from a request
pub trait RequestIdExt {
    /// Get the request ID if it exists
    fn request_id(&self) -> Option<&RequestId>;

    /// Get the request ID as a string, or "unknown" if not present
    fn request_id_str(&self) -> &str;
}

impl<T> RequestIdExt for Request<T> {
    fn request_id(&self) -> Option<&RequestId> {
        self.extensions().get::<RequestId>()
    }

    fn request_id_str(&self) -> &str {
        self.request_id().map(|id| id.as_str()).unwrap_or("unknown")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_request_ids() {
        // Valid ULIDs
        assert!(is_valid_request_id("01AN4Z07BY79KA1307SR9X4MV3"));
        assert!(is_valid_request_id("01ARYZ6S41TSV4RRFFQ69G5FAV"));

        // Valid UUIDs
        assert!(is_valid_request_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_valid_request_id("550e8400e29b41d4a716446655440000"));

        // Valid alphanumeric IDs
        assert!(is_valid_request_id("abc123def456"));
        assert!(is_valid_request_id("trace_123_456"));
        assert!(is_valid_request_id("correlation.id.789"));
        assert!(is_valid_request_id("req-2024-01-15-001"));
    }

    #[test]
    fn test_invalid_request_ids() {
        // Too short
        assert!(!is_valid_request_id("abc"));

        // Too long
        let long_id = "a".repeat(129);
        assert!(!is_valid_request_id(&long_id));

        // Invalid characters
        assert!(!is_valid_request_id("abc@123"));
        assert!(!is_valid_request_id("abc#def"));
        assert!(!is_valid_request_id("id with spaces"));
        assert!(!is_valid_request_id("id/with/slashes"));
    }

    #[test]
    fn test_request_id_new() {
        let id1 = RequestId::new();
        let id2 = RequestId::new();

        // Each new ID should be unique
        assert_ne!(id1.as_str(), id2.as_str());

        // Should be valid ULIDs
        assert!(is_valid_request_id(id1.as_str()));
        assert!(is_valid_request_id(id2.as_str()));

        // Should be 26 characters (ULID length)
        assert_eq!(id1.as_str().len(), 26);
        assert_eq!(id2.as_str().len(), 26);
    }

    #[test]
    fn test_request_id_from_string() {
        let original = "test-request-id-123";
        let id = RequestId::from_string(original.to_string());
        assert_eq!(id.as_str(), original);
    }

    #[test]
    fn test_request_id_display() {
        let id = RequestId::from_string("display-test".to_string());
        assert_eq!(format!("{}", id), "display-test");
    }
}
