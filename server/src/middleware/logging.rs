//! Standalone request/response logging middleware.
//!
//! These handlers log one line when a request starts and one leveled line
//! when it completes (info for 2xx/3xx, warn for 4xx, error for 5xx, plus a
//! `performance` warning for requests over one second). They read the
//! `Arc<CurrentUser>` extension left by the auth middleware to tag log lines
//! with a username, and insert nothing into the request extensions
//! themselves.
//!
//! Note that this module is re-exported from [`crate::middleware`] but is
//! not installed in the default stack built by [`crate::routes::app`]; there,
//! request logging comes from tower-http's `TraceLayer` and the request-ID
//! middleware. To use these handlers, layer [`logging_middleware`] (or
//! [`filtered_logging_middleware`], which skips health checks and static
//! assets) inside the auth middleware so the user extension is populated.

use crate::logging::format_http_status;
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Log detailed request and response information for a single request.
///
/// Emits an `http_request` event when the request starts and an
/// `http_response` event when it completes, leveled by status class, with
/// method, URI, duration, and the current user's username (or `anonymous`).
/// Requests slower than one second additionally emit a `performance`
/// warning.
///
/// # Errors
///
/// This function always returns `Ok`; the `Result` exists only to satisfy
/// the middleware signature.
pub async fn logging_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();

    // Extract user info if available (set by auth middleware)
    let user_info = request
        .extensions()
        .get::<std::sync::Arc<crate::middleware::auth::CurrentUser>>()
        .map(|user| format!("user:{}", user.username))
        .unwrap_or_else(|| "anonymous".to_string());

    // Log request start
    info!(
        target: "http_request",
        method = %method,
        uri = %uri,
        path = %path,
        user = %user_info,
        "→ Request started"
    );

    // Process the request
    let response = next.run(request).await;

    // Calculate request duration
    let duration = start_time.elapsed();
    let duration_ms = duration.as_millis();

    // Extract status code
    let status = response.status();
    let status_code = status.as_u16();

    // Determine log level based on status code
    match status_code {
        200..=299 => {
            let formatted_status = format_http_status(status_code);
            info!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = %formatted_status,
                duration_ms = duration_ms,
                user = %user_info,
                "✓ Request completed successfully"
            );
        }
        300..=399 => {
            let formatted_status = format_http_status(status_code);
            info!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = %formatted_status,
                duration_ms = duration_ms,
                user = %user_info,
                "→ Request redirected"
            );
        }
        400..=499 => {
            let formatted_status = format_http_status(status_code);
            warn!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = %formatted_status,
                duration_ms = duration_ms,
                user = %user_info,
                "⚠ Client error"
            );
        }
        500..=599 => {
            let formatted_status = format_http_status(status_code);
            error!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = %formatted_status,
                duration_ms = duration_ms,
                user = %user_info,
                "✗ Server error"
            );
        }
        _ => {
            let formatted_status = format_http_status(status_code);
            debug!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = %formatted_status,
                duration_ms = duration_ms,
                user = %user_info,
                "Request completed"
            );
        }
    }

    // Log slow requests (over 1 second)
    if duration_ms > 1000 {
        let formatted_status = format_http_status(status_code);
        warn!(
            target: "performance",
            method = %method,
            uri = %uri,
            path = %path,
            status = %formatted_status,
            duration_ms = duration_ms,
            user = %user_info,
            "⏱ Slow request detected"
        );
    }

    Ok(response)
}

/// Variant of [`logging_middleware`] that skips noisy paths.
///
/// Health checks (`/api/health`), `/favicon.ico`, `/robots.txt`, and
/// anything under `/static/` pass through without logging; every other
/// request is delegated to [`logging_middleware`].
///
/// # Errors
///
/// This function always returns `Ok`; the `Result` exists only to satisfy
/// the middleware signature.
pub async fn filtered_logging_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let uri = request.uri().clone();
    let path = uri.path();

    // Skip logging for certain paths
    if should_skip_logging(path) {
        return Ok(next.run(request).await);
    }

    // Use the main logging middleware for everything else
    logging_middleware(request, next).await
}

/// Determine if a path should skip logging
fn should_skip_logging(path: &str) -> bool {
    // Skip logging for health checks and static assets
    matches!(path, "/api/health" | "/favicon.ico" | "/robots.txt") || path.starts_with("/static/")
}

/// Aggregate request counters for periodic summary logging.
#[derive(Debug, Default)]
pub struct RequestStats {
    /// Total number of requests observed.
    pub total_requests: u64,
    /// Requests that completed with a success status.
    pub successful_requests: u64,
    /// Requests that completed with an error status.
    pub failed_requests: u64,
    /// Sum of all request durations in milliseconds, used for averaging.
    pub total_duration_ms: u64,
}

impl RequestStats {
    /// Log a single `http_stats` summary event with the request totals,
    /// average duration, and success rate. Does nothing when no requests
    /// have been recorded.
    pub fn log_summary(&self) {
        if let Some(avg_duration) = self.total_duration_ms.checked_div(self.total_requests) {
            let success_rate =
                (self.successful_requests as f64 / self.total_requests as f64) * 100.0;

            info!(
                target: "http_stats",
                total_requests = self.total_requests,
                successful = self.successful_requests,
                failed = self.failed_requests,
                avg_duration_ms = avg_duration,
                success_rate = format!("{:.1}%", success_rate),
                "Request statistics summary"
            );
        }
    }
}
