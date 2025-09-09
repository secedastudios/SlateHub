//! Common test utilities and helpers for SlateHub integration tests

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use once_cell::sync::Lazy;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test database URL - uses a separate test database
pub static TEST_DB_URL: Lazy<String> = Lazy::new(|| {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "ws://localhost:8000/test".to_string())
});

/// Mutex to ensure database tests run sequentially
pub static DB_MUTEX: Lazy<Arc<Mutex<()>>> = Lazy::new(|| Arc::new(Mutex::new(())));

/// Initialize test environment
pub async fn setup() {
    // Load test environment variables
    dotenv::dotenv().ok();

    // Set up test logging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("slatehub=debug,tower_http=debug")
        .try_init();

    // Initialize templates
    if let Err(e) = slatehub::templates::init() {
        eprintln!("Warning: Failed to initialize templates for tests: {}", e);
    }
}

/// Create a test application instance
pub async fn test_app() -> Router {
    setup().await;
    slatehub::routes::app()
}

/// Create a test JWT token for authentication
pub fn create_test_token(user_id: &str, username: &str) -> String {
    use chrono::{Duration, Utc};
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Claims {
        user_id: String,
        username: String,
        exp: i64,
    }

    let claims = Claims {
        user_id: user_id.to_string(),
        username: username.to_string(),
        exp: (Utc::now() + Duration::hours(24)).timestamp(),
    };

    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "test_secret".to_string());

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .unwrap_or_else(|_| "invalid_token".to_string())
}

/// Builder for creating test requests
pub struct TestRequest {
    method: Method,
    uri: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

impl TestRequest {
    /// Create a new GET request
    pub fn get(uri: impl Into<String>) -> Self {
        Self {
            method: Method::GET,
            uri: uri.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    /// Create a new POST request
    pub fn post(uri: impl Into<String>) -> Self {
        Self {
            method: Method::POST,
            uri: uri.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    /// Add authentication to the request
    pub fn with_auth(mut self, user_id: &str, username: &str) -> Self {
        let token = create_test_token(user_id, username);
        self.headers
            .push(("Cookie".to_string(), format!("auth_token={}", token)));
        self
    }

    /// Add a header to the request
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    /// Set the request body as form data
    pub fn with_form(mut self, data: &[(&str, &str)]) -> Self {
        let form_data: Vec<String> = data
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect();

        self.body = Some(form_data.join("&"));
        self.headers.push((
            header::CONTENT_TYPE.to_string(),
            "application/x-www-form-urlencoded".to_string(),
        ));
        self
    }

    /// Set the request body as JSON
    pub fn with_json(mut self, json: Value) -> Self {
        self.body = Some(json.to_string());
        self.headers.push((
            header::CONTENT_TYPE.to_string(),
            "application/json".to_string(),
        ));
        self
    }

    /// Build the request
    pub fn build(self) -> Request<Body> {
        let mut builder = Request::builder().method(self.method).uri(self.uri);

        for (key, value) in self.headers {
            builder = builder.header(key, value);
        }

        let body = self.body.map(|b| Body::from(b)).unwrap_or_else(Body::empty);

        builder.body(body).unwrap()
    }
}

/// Assert that a response redirects to a specific location
pub fn assert_redirect(response: &axum::response::Response, expected_location: &str) {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok());

    assert_eq!(location, Some(expected_location));
}

/// Assert that a response contains specific text in the body
pub async fn assert_body_contains(body: Body, text: &str) {
    use axum::body::to_bytes;

    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    let body_str = String::from_utf8_lossy(&bytes);

    assert!(
        body_str.contains(text),
        "Expected body to contain '{}', but it didn't. Body: {}",
        text,
        body_str
    );
}

/// Create test profile data
pub fn test_profile_data() -> serde_json::Value {
    serde_json::json!({
        "name": "Test User",
        "headline": "Test Professional",
        "bio": "This is a test bio",
        "location": "Test City",
        "website": "https://example.com",
        "is_public": true,
        "skills": ["Skill1", "Skill2"],
        "languages": ["English", "Spanish"],
        "unions": ["SAG-AFTRA"],
        "availability": "available"
    })
}

/// Create test user data
pub fn test_user_data() -> serde_json::Value {
    serde_json::json!({
        "username": "testuser",
        "email": "test@example.com",
        "password": "Test123!@#",
        "confirm_password": "Test123!@#"
    })
}

/// Clean up test data after tests
pub async fn cleanup() {
    // This would connect to test database and clean up test data
    // For now, it's a placeholder
    // In a real implementation, you'd want to:
    // 1. Connect to test database
    // 2. Delete test users/data created during tests
    // 3. Reset sequences/counters
}

/// Macro to create a test that requires database access
#[macro_export]
macro_rules! db_test {
    ($name:ident, $body:expr) => {
        #[tokio::test]
        async fn $name() {
            let _lock = $crate::common::DB_MUTEX.lock().await;
            $crate::common::setup().await;

            $body

            $crate::common::cleanup().await;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_builder() {
        let request = TestRequest::get("/test")
            .with_header("X-Test", "value")
            .build();

        assert_eq!(request.method(), Method::GET);
        assert_eq!(request.uri(), "/test");
        assert_eq!(
            request
                .headers()
                .get("X-Test")
                .and_then(|v| v.to_str().ok()),
            Some("value")
        );
    }

    #[test]
    fn test_form_request_builder() {
        let request = TestRequest::post("/test")
            .with_form(&[("key", "value"), ("foo", "bar")])
            .build();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/x-www-form-urlencoded")
        );
    }

    #[test]
    fn test_json_request_builder() {
        let request = TestRequest::post("/test")
            .with_json(serde_json::json!({"key": "value"}))
            .build();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
    }

    #[test]
    fn test_auth_request_builder() {
        let request = TestRequest::get("/test")
            .with_auth("user123", "testuser")
            .build();

        let cookie = request
            .headers()
            .get("Cookie")
            .and_then(|v| v.to_str().ok());

        assert!(cookie.is_some());
        assert!(cookie.unwrap().starts_with("auth_token="));
    }
}
