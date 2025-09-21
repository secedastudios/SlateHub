use axum::Json;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("database error: {0}")]
    Database(String),

    #[error("template error: {0}")]
    Template(String),

    #[error("not found")]
    NotFound,

    #[error("internal server error: {0}")]
    Internal(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("external service error: {0}")]
    ExternalService(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message, custom_message) = match &self {
            Error::Database(msg) => {
                error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error occurred",
                    None,
                )
            }
            Error::Template(msg) => {
                error!("Template rendering error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Template rendering failed",
                    None,
                )
            }
            Error::NotFound => (StatusCode::NOT_FOUND, "Resource not found", None),
            Error::Internal(msg) => {
                error!("Internal server error: {}", msg);
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
                error!("External service error: {}", msg);
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
        error!("Database error occurred: {:?}", err);
        Self::Database(err.to_string())
    }
}

// Conversion from template errors (Askama)
impl From<askama::Error> for Error {
    fn from(err: askama::Error) -> Self {
        error!("Template error occurred: {:?}", err);
        Self::Template(err.to_string())
    }
}

// Conversion from serde_json errors
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        error!("JSON serialization error: {:?}", err);
        Self::BadRequest(format!("Invalid JSON: {}", err))
    }
}

// Conversion from std::io::Error
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        error!("IO error occurred: {:?}", err);
        Self::Internal(err.to_string())
    }
}

// Helper type for Results
pub type Result<T> = std::result::Result<T, Error>;

// Convenience constructors
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
}
