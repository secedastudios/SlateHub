use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
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

    #[error("internal server error")]
    Internal,

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
        let (status, error_message) = match &self {
            Error::Database(msg) => {
                error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error occurred")
            }
            Error::Template(msg) => {
                error!("Template rendering error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Template rendering failed",
                )
            }
            Error::NotFound => (StatusCode::NOT_FOUND, "Resource not found"),
            Error::Internal => {
                error!("Internal server error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            Error::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
            Error::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            Error::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),
            Error::Conflict(msg) => (StatusCode::CONFLICT, msg.as_str()),
            Error::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.as_str()),
            Error::ExternalService(msg) => {
                error!("External service error: {}", msg);
                (StatusCode::BAD_GATEWAY, "External service error")
            }
        };

        // Create a JSON response with error details
        let body = serde_json::json!({
            "error": error_message,
            "status": status.as_u16(),
        });

        (status, Json(body)).into_response()
    }
}

// Conversion from surrealdb errors
impl From<surrealdb::Error> for Error {
    fn from(err: surrealdb::Error) -> Self {
        error!("Database error occurred: {:?}", err);
        Self::Database(err.to_string())
    }
}

// Conversion from template errors (assuming you're using a template engine like tera)
impl From<tera::Error> for Error {
    fn from(err: tera::Error) -> Self {
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
        Self::Internal
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
}
