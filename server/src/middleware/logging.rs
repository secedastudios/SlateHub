use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Enhanced logging middleware that logs detailed request/response information
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
            info!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = status_code,
                duration_ms = duration_ms,
                user = %user_info,
                "✓ Request completed successfully"
            );
        }
        300..=399 => {
            info!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = status_code,
                duration_ms = duration_ms,
                user = %user_info,
                "→ Request redirected"
            );
        }
        400..=499 => {
            warn!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = status_code,
                duration_ms = duration_ms,
                user = %user_info,
                "⚠ Client error"
            );
        }
        500..=599 => {
            error!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = status_code,
                duration_ms = duration_ms,
                user = %user_info,
                "✗ Server error"
            );
        }
        _ => {
            debug!(
                target: "http_response",
                method = %method,
                uri = %uri,
                path = %path,
                status = status_code,
                duration_ms = duration_ms,
                user = %user_info,
                "Request completed"
            );
        }
    }

    // Log slow requests (over 1 second)
    if duration_ms > 1000 {
        warn!(
            target: "performance",
            method = %method,
            uri = %uri,
            path = %path,
            status = status_code,
            duration_ms = duration_ms,
            user = %user_info,
            "⏱ Slow request detected"
        );
    }

    Ok(response)
}

/// Middleware for logging only specific routes or with specific filters
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

/// Format bytes for human-readable output
#[allow(dead_code)]
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Log summary statistics periodically
#[derive(Debug, Default)]
pub struct RequestStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_duration_ms: u64,
}

impl RequestStats {
    pub fn log_summary(&self) {
        if self.total_requests > 0 {
            let avg_duration = self.total_duration_ms / self.total_requests;
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
