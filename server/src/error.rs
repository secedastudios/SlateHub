//! The crate-wide error type and its HTTP mapping.
//!
//! Every fallible path in models, services, and routes returns
//! [`enum@Error`] via the crate's [`Result`] alias. Because `Error`
//! implements `IntoResponse`, handlers can simply `?` their way through and
//! axum renders the failure: the variant picks the status code, the body is
//! JSON, and `X-Error-Message`/`X-Error-Custom-Message` headers carry the
//! human text so [`crate::middleware`]'s error-response layer can re-render
//! it as an HTML error page when the client prefers HTML.
//!
//! Server-side variants (`Database`, `Template`, `Internal`,
//! `ExternalService`) log on conversion/response and deliberately return a
//! generic message — internals never leak to clients. Client-side variants
//! (`BadRequest`, `Conflict`, `Validation`) surface their message verbatim.

use crate::log_colored_error;
use crate::log_db_error;
use axum::Json;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

/// Application-wide error; maps 1:1 onto an HTTP status (see module docs).
#[derive(Error, Debug)]
pub enum Error {
    /// SurrealDB failure → 500. Message is logged, never sent to clients.
    #[error("database error: {0}")]
    Database(String),

    /// Askama rendering failure → 500. Logged, not sent to clients.
    #[error("template error: {0}")]
    Template(String),

    /// Missing resource → 404. Also used to *hide* gated features
    /// (management mode returns NotFound, not Forbidden, to non-members).
    #[error("not found")]
    NotFound,

    /// Unclassified server bug → 500. Logged, not sent to clients.
    #[error("internal server error: {0}")]
    Internal(String),

    /// Malformed client input → 400. Message is shown to the client.
    #[error("bad request: {0}")]
    BadRequest(String),

    /// No (valid) session → 401.
    #[error("unauthorized")]
    Unauthorized,

    /// Authenticated but not allowed → 403.
    #[error("forbidden")]
    Forbidden,

    /// State conflict (duplicate slug, double-submit …) → 409. Shown.
    #[error("conflict: {0}")]
    Conflict(String),

    /// Semantic form/input failure → 422. Shown to the client.
    #[error("validation error: {0}")]
    Validation(String),

    /// Upstream (S3, Stripe, Listmonk, LLM …) failure → 502. Logged.
    #[error("external service error: {0}")]
    ExternalService(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message, custom_message) = match &self {
            Error::Database(msg) => {
                log_db_error!(msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error occurred",
                    None,
                )
            }
            Error::Template(msg) => {
                log_colored_error!("internal", format!("Template rendering error: {}", msg));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Template rendering failed",
                    None,
                )
            }
            Error::NotFound => (StatusCode::NOT_FOUND, "Resource not found", None),
            Error::Internal(msg) => {
                log_colored_error!("internal", format!("Internal server error: {}", msg));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error",
                    None,
                )
            }
            Error::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.as_str(), Some(msg.clone())),
            Error::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized", None),
            Error::Forbidden => (StatusCode::FORBIDDEN, "Forbidden", None),
            Error::Conflict(msg) => (StatusCode::CONFLICT, msg.as_str(), Some(msg.clone())),
            Error::Validation(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                msg.as_str(),
                Some(msg.clone()),
            ),
            Error::ExternalService(msg) => {
                log_colored_error!("network", format!("External service error: {}", msg));
                (StatusCode::BAD_GATEWAY, "External service error", None)
            }
        };

        // Create a JSON response with error details
        let body = json!({
            "error": error_message,
            "status": status.as_u16(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        // Add a special header to indicate this is an error that could be converted to HTML
        // The middleware will check for this header and the Accept header to determine
        // whether to convert to HTML
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(
            "X-Error-Message",
            HeaderValue::from_str(error_message)
                .unwrap_or_else(|_| HeaderValue::from_static("error")),
        );
        if let Some(custom_msg) = custom_message {
            response.headers_mut().insert(
                "X-Error-Custom-Message",
                HeaderValue::from_str(&custom_msg).unwrap_or_else(|_| HeaderValue::from_static("")),
            );
        }
        response
    }
}

// Conversion from surrealdb errors
impl From<surrealdb::Error> for Error {
    fn from(err: surrealdb::Error) -> Self {
        log_db_error!(format!("{:?}", err), "SurrealDB operation failed");
        Self::Database(err.to_string())
    }
}

// Conversion from template errors (Askama)
impl From<askama::Error> for Error {
    fn from(err: askama::Error) -> Self {
        log_colored_error!("internal", format!("Template error occurred: {:?}", err));
        Self::Template(err.to_string())
    }
}

// Conversion from serde_json errors
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        log_colored_error!("http", format!("JSON serialization error: {:?}", err));
        Self::BadRequest(format!("Invalid JSON: {}", err))
    }
}

// Conversion from std::io::Error
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        log_colored_error!("internal", format!("IO error occurred: {:?}", err));
        Self::Internal(err.to_string())
    }
}

/// Crate-wide result alias; the `E` is always [`enum@Error`].
pub type Result<T> = std::result::Result<T, Error>;

// Convenience constructors so call sites read `Error::bad_request("…")`
// instead of `Error::BadRequest("…".to_string())`.
impl Error {
    pub fn database<S: Into<String>>(msg: S) -> Self {
        Self::Database(msg.into())
    }

    pub fn template<S: Into<String>>(msg: S) -> Self {
        Self::Template(msg.into())
    }

    pub fn bad_request<S: Into<String>>(msg: S) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn conflict<S: Into<String>>(msg: S) -> Self {
        Self::Conflict(msg.into())
    }

    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::Validation(msg.into())
    }

    pub fn external_service<S: Into<String>>(msg: S) -> Self {
        Self::ExternalService(msg.into())
    }

    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }

    /// Parse form validation errors and return a user-friendly message
    pub fn parse_form_validation_error<S: AsRef<str>>(error_msg: S) -> Self {
        let msg = error_msg.as_ref();

        // Common form validation error patterns and their user-friendly messages
        let friendly_message = if msg.contains("cannot parse integer from empty string") {
            if let Some(field) = extract_field_name(msg) {
                format!(
                    "Please enter a valid number for {}",
                    format_field_name(&field)
                )
            } else {
                "Please enter a valid number in all numeric fields".to_string()
            }
        } else if msg.contains("cannot parse") && msg.contains("from empty string") {
            "Please fill in all required fields".to_string()
        } else if msg.contains("invalid digit found") {
            "Please enter only numbers in numeric fields".to_string()
        } else if msg.contains("number too large") {
            "The number entered is too large".to_string()
        } else if msg.contains("number too small") || msg.contains("negative") {
            "Please enter a positive number".to_string()
        } else if msg.contains("Failed to deserialize form") {
            // Try to extract the specific field from the error
            if let Some(field) = extract_field_name(msg) {
                format!("Invalid value for field: {}", format_field_name(&field))
            } else {
                "Please check your form input and try again".to_string()
            }
        } else {
            // Default to the original message, but try to make it cleaner
            msg.replace("Failed to deserialize form body: ", "")
                .replace("Failed to deserialize query string: ", "")
        };

        Self::Validation(friendly_message)
    }
}

/// Extract field name from error messages like "field_name: error details"
fn extract_field_name(msg: &str) -> Option<String> {
    // Look for pattern like "field_name: " in the error message
    if let Some(colon_pos) = msg.find(':') {
        let potential_field = &msg[..colon_pos];
        // Check if it looks like a field name (contains underscores or is a single word)
        if potential_field.split_whitespace().count() == 1
            || potential_field.contains('_')
            || potential_field.contains("Failed to deserialize form body")
        {
            // Extract just the field name part
            let field = potential_field
                .replace("Failed to deserialize form body", "")
                .replace("Failed to deserialize query string", "")
                .trim()
                .to_string();
            if !field.is_empty() {
                return Some(field);
            }
        }
    }
    None
}

/// Format field names to be more user-friendly
fn format_field_name(field: &str) -> String {
    field
        .replace('_', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

// Implement From for axum's rejection types to provide better error messages
impl From<axum::extract::rejection::FormRejection> for Error {
    fn from(rejection: axum::extract::rejection::FormRejection) -> Self {
        let message = rejection.body_text();
        Error::parse_form_validation_error(message)
    }
}

impl From<axum::extract::rejection::QueryRejection> for Error {
    fn from(rejection: axum::extract::rejection::QueryRejection) -> Self {
        let message = rejection.body_text();
        Error::parse_form_validation_error(message)
    }
}
