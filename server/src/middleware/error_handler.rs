use axum::{
    Json,
    extract::Request,
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use serde_json::json;
use tracing::{error, warn};

use crate::{error::Error, middleware::RequestIdExt};
use crate::{log_colored_error, log_db_error};

/// Check if the client accepts HTML responses
fn accepts_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/html"))
        .unwrap_or(false)
}

/// Create an error response based on the client's Accept header
pub fn create_error_response(
    error: &Error,
    headers: &HeaderMap,
    request_path: Option<String>,
    request_id: Option<String>,
) -> Response {
    let (status, error_message, custom_message) = match error {
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

    if accepts_html(headers) {
        render_html_error(
            status,
            error_message,
            custom_message,
            request_path,
            request_id,
        )
    } else {
        render_json_error(status, error_message, request_id)
    }
}

/// Render an HTML error page
fn render_html_error(
    status: StatusCode,
    error_message: &str,
    custom_message: Option<String>,
    request_path: Option<String>,
    request_id: Option<String>,
) -> Response {
    let status_code = status.as_u16();
    let status_text = status.canonical_reason().unwrap_or("Error");

    let (title, heading, description) = match status {
        StatusCode::NOT_FOUND => (
            "Page Not Found",
            "404".to_string(),
            custom_message.unwrap_or_else(|| "The page you're looking for doesn't exist or may have been moved.".to_string()),
        ),
        StatusCode::UNAUTHORIZED => (
            "Unauthorized",
            "401".to_string(),
            custom_message.unwrap_or_else(|| "You need to be signed in to access this page.".to_string()),
        ),
        StatusCode::FORBIDDEN => (
            "Access Forbidden",
            "403".to_string(),
            custom_message.unwrap_or_else(|| "You don't have permission to access this resource.".to_string()),
        ),
        StatusCode::INTERNAL_SERVER_ERROR | StatusCode::BAD_GATEWAY => (
            "Server Error",
            "500".to_string(),
            custom_message.unwrap_or_else(|| "Something went wrong on our end. We've been notified and are working to fix the issue.".to_string()),
        ),
        StatusCode::UNPROCESSABLE_ENTITY => (
            "Invalid Input",
            "422".to_string(),
            custom_message.unwrap_or_else(|| "The information you provided couldn't be processed. Please check your input and try again.".to_string()),
        ),
        StatusCode::BAD_REQUEST => (
            "Bad Request",
            "400".to_string(),
            custom_message.unwrap_or_else(|| "Your request couldn't be understood. Please check your input and try again.".to_string()),
        ),
        _ => (
            status_text,
            status_code.to_string(),
            custom_message.unwrap_or_else(|| error_message.to_string()),
        ),
    };

    let _ = (request_path, request_id);

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="color-scheme" content="dark">
    <title>{title} - SlateHub</title>
    <link rel="stylesheet" href="/static/css/main.css">
    <link rel="stylesheet" href="/static/css/pages/errors.css">
</head>
<body data-page="error-{status_code}" style="background:#171717;color:#d6d8ca">
    <main id="main-content">
        <article data-component="error-page" data-error-code="{status_code}">
            <a href="/" data-role="error-brand" aria-label="SlateHub Home">
                <img src="/static/images/logo.svg" alt="SlateHub" height="14" />
            </a>
            <header data-role="error-header">
                <span data-role="error-code">{heading}</span>
                <h1>{status_text}</h1>
                <p data-role="error-description">{description}</p>
            </header>
            <nav data-role="error-actions">
                <a href="/" role="button" data-type="primary">Homepage</a>
                <a href="/login" role="button" data-type="secondary">Sign In</a>
            </nav>
            <footer data-role="error-footer">
                <p>If this persists, please <a href="/contact">contact support</a>.</p>
            </footer>
        </article>
    </main>
</body>
</html>"#,
        title = title,
        status_code = status_code,
        heading = heading,
        status_text = status_text,
        description = description,
    );

    (status, Html(html)).into_response()
}

/// Render a JSON error response
fn render_json_error(
    status: StatusCode,
    error_message: &str,
    request_id: Option<String>,
) -> Response {
    let body = json!({
        "error": error_message,
        "status": status.as_u16(),
        "request_id": request_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    (status, Json(body)).into_response()
}

/// Middleware to handle errors and render appropriate responses
pub async fn error_response_middleware(req: Request, next: Next) -> Response {
    let headers = req.headers().clone();
    let path = req.uri().path().to_string();
    let method = req.method().to_string();
    let query = req.uri().query().map(|q| q.to_string());
    let request_id = req.request_id().map(|id| id.to_string());

    let response = next.run(req).await;

    // Check if response has an error status code
    if response.status().is_client_error() || response.status().is_server_error() {
        let status = response.status();

        // Extract error message from header if available (for detailed logging)
        let error_detail = response
            .headers()
            .get("X-Error-Message")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("Unknown error");

        // Log the error with appropriate detail level
        match status {
            StatusCode::NOT_FOUND => {
                warn!(
                    path = %path,
                    method = %method,
                    request_id = ?request_id,
                    "404 Not Found"
                );
            }
            StatusCode::UNAUTHORIZED => {
                warn!(
                    path = %path,
                    method = %method,
                    request_id = ?request_id,
                    "401 Unauthorized"
                );
            }
            StatusCode::FORBIDDEN => {
                warn!(
                    path = %path,
                    method = %method,
                    request_id = ?request_id,
                    "403 Forbidden"
                );
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                warn!(
                    path = %path,
                    method = %method,
                    error = %error_detail,
                    request_id = ?request_id,
                    "422 Unprocessable Entity"
                );
            }
            StatusCode::BAD_REQUEST => {
                warn!(
                    path = %path,
                    method = %method,
                    error = %error_detail,
                    request_id = ?request_id,
                    "400 Bad Request"
                );
            }
            _ if status.is_server_error() => {
                // Enhanced logging for 500 errors - include all available context
                error!(
                    status = %status.as_u16(),
                    path = %path,
                    method = %method,
                    query = ?query,
                    error = %error_detail,
                    request_id = ?request_id,
                    "SERVER ERROR - This requires investigation"
                );
            }
            _ => {
                warn!(
                    status = %status.as_u16(),
                    path = %path,
                    method = %method,
                    request_id = ?request_id,
                    "Client Error"
                );
            }
        }

        // Check if this is our error response (has X-Error-Message header) and client accepts HTML
        if accepts_html(&headers) {
            // Check for our special error headers
            let has_error_header = response.headers().contains_key("X-Error-Message");

            if has_error_header {
                // Extract custom message if available
                let custom_message = response
                    .headers()
                    .get("X-Error-Custom-Message")
                    .and_then(|v| v.to_str().ok())
                    .map(String::from);

                // Create the appropriate error based on status code
                let error = match status {
                    StatusCode::NOT_FOUND => Error::NotFound,
                    StatusCode::UNAUTHORIZED => Error::Unauthorized,
                    StatusCode::FORBIDDEN => Error::Forbidden,
                    StatusCode::BAD_REQUEST => {
                        if let Some(msg) = custom_message.clone() {
                            Error::BadRequest(msg)
                        } else {
                            Error::BadRequest("Bad request".to_string())
                        }
                    }
                    StatusCode::CONFLICT => {
                        if let Some(msg) = custom_message.clone() {
                            Error::Conflict(msg)
                        } else {
                            Error::Conflict("Conflict".to_string())
                        }
                    }
                    StatusCode::UNPROCESSABLE_ENTITY => {
                        if let Some(msg) = custom_message.clone() {
                            Error::Validation(msg)
                        } else {
                            Error::Validation("Validation error".to_string())
                        }
                    }
                    StatusCode::BAD_GATEWAY => {
                        Error::ExternalService("External service error".to_string())
                    }
                    StatusCode::INTERNAL_SERVER_ERROR => {
                        Error::Internal("Internal server error".to_string())
                    }
                    _ => Error::Internal(format!("HTTP {}", status.as_u16())),
                };

                // Replace the response with an HTML error page
                return create_error_response(&error, &headers, Some(path), request_id);
            }
        }
    }

    response
}

/// Helper trait for converting errors to responses with context
pub trait ErrorWithContext {
    fn with_context(
        self,
        headers: &HeaderMap,
        path: Option<String>,
        request_id: Option<String>,
    ) -> Response;
}

impl ErrorWithContext for Error {
    fn with_context(
        self,
        headers: &HeaderMap,
        path: Option<String>,
        request_id: Option<String>,
    ) -> Response {
        create_error_response(&self, headers, path, request_id)
    }
}

/// Extension trait for Result types to convert errors with context
pub trait ResultExt<T> {
    #[allow(clippy::result_large_err)]
    fn with_error_context(self, req: &Request) -> Result<T, Response>;
}

impl<T> ResultExt<T> for Result<T, Error> {
    fn with_error_context(self, req: &Request) -> Result<T, Response> {
        self.map_err(|e| {
            let headers = req.headers().clone();
            let path = Some(req.uri().path().to_string());
            let request_id = req.request_id().map(|id| id.to_string());

            e.with_context(&headers, path, request_id)
        })
    }
}
