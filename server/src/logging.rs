use std::env;
use std::fmt::Display;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

// ANSI color codes for terminal output
const COLOR_RESET: &str = "\x1b[0m";
const COLOR_GREEN: &str = "\x1b[32m";
const COLOR_YELLOW: &str = "\x1b[33m";
const COLOR_ORANGE: &str = "\x1b[38;5;214m"; // Light orange
const COLOR_RED: &str = "\x1b[31m";
const COLOR_LIGHT_ORANGE: &str = "\x1b[38;5;215m"; // Light orange for database errors

/// Format an HTTP status code with appropriate color
pub fn format_http_status(status: u16) -> String {
    let color = match status {
        200..=299 => COLOR_GREEN,  // 2xx - Success (Green)
        300..=399 => COLOR_YELLOW, // 3xx - Redirect (Yellow)
        400..=499 => COLOR_ORANGE, // 4xx - Client Error (Orange)
        500..=599 => COLOR_RED,    // 5xx - Server Error (Red)
        _ => COLOR_RESET,          // Other status codes
    };

    format!("{}{}{}", color, status, COLOR_RESET)
}

/// Format a database error message with light orange color
pub fn format_database_error<T: Display>(message: T) -> String {
    format!(
        "{}Database error: {}{}",
        COLOR_LIGHT_ORANGE, message, COLOR_RESET
    )
}

/// Format any error message with appropriate color based on type
pub fn format_colored_error<T: Display>(error_type: &str, message: T) -> String {
    match error_type.to_lowercase().as_str() {
        "database" | "db" => format!(
            "{}Database error: {}{}",
            COLOR_LIGHT_ORANGE, message, COLOR_RESET
        ),
        "http" | "network" => format!("{}Network error: {}{}", COLOR_ORANGE, message, COLOR_RESET),
        "internal" | "server" => format!("{}Internal error: {}{}", COLOR_RED, message, COLOR_RESET),
        _ => format!("{}Error: {}{}", COLOR_ORANGE, message, COLOR_RESET),
    }
}

/// Initialize the tracing subscriber for logging
pub fn init() {
    // Get log format from environment, default to "dev" for better debugging
    // Options: "json", "compact", "dev", "pretty"
    let log_format = env::var("LOG_FORMAT").unwrap_or_else(|_| "dev".to_string());

    // Create env filter from RUST_LOG or use default
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default log level configuration with enhanced HTTP request/response logging
        // - info: general application logs
        // - slatehub=debug: detailed app-specific logs
        // - tower_http=debug: HTTP layer logging
        // - http_request=info: custom request logging
        // - http_response=info: custom response logging with status codes
        EnvFilter::new("info,slatehub=debug,tower_http=debug,http_request=info,http_response=info")
    });

    match log_format.as_str() {
        "json" => {
            // JSON formatted logs - useful for production and log aggregation
            // Includes full location information for debugging
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_file(true)
                        .with_line_number(true)
                        .with_target(true)
                        .with_span_events(fmt::format::FmtSpan::FULL),
                )
                .init();
        }
        "compact" => {
            // Compact format - includes location info but more condensed
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .compact()
                        .with_target(true)
                        .with_file(true)
                        .with_line_number(true)
                        .with_thread_names(false),
                )
                .init();
        }
        "dev" => {
            // Developer format - clean location info for easy debugging
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_file(true)
                        .with_line_number(true)
                        .with_thread_names(false)
                        .with_thread_ids(false)
                        .with_level(true)
                        .with_ansi(true)
                        .compact()
                        .with_span_events(fmt::format::FmtSpan::NONE),
                )
                .init();
        }
        _ => {
            // Pretty format (default) - good for development with full debugging info
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .pretty()
                        .with_target(true)
                        .with_file(true)
                        .with_line_number(true)
                        .with_thread_names(false)
                        .with_thread_ids(false)
                        .with_span_events(fmt::format::FmtSpan::CLOSE),
                )
                .init();
        }
    }

    tracing::info!(
        "Logging initialized with format: {} (includes file:line info for debugging)",
        log_format
    );
}

/// Log an HTTP response with colored status code
#[macro_export]
macro_rules! log_http_response {
    ($status:expr, $method:expr, $path:expr) => {{
        let status_code = $status;
        let formatted_status = $crate::logging::format_http_status(status_code);
        tracing::info!("← {} {} {}", $method, $path, formatted_status);
    }};
    ($status:expr, $method:expr, $path:expr, $latency_ms:expr) => {{
        let status_code = $status;
        let formatted_status = $crate::logging::format_http_status(status_code);
        tracing::info!(
            "← {} {} {} ({}ms)",
            $method,
            $path,
            formatted_status,
            $latency_ms
        );
    }};
}

/// Log a database error with color formatting
#[macro_export]
macro_rules! log_db_error {
    ($err:expr) => {{
        let formatted_error = $crate::logging::format_database_error(&$err);
        tracing::error!("{}", formatted_error);
    }};
    ($err:expr, $operation:expr) => {{
        let formatted_error =
            $crate::logging::format_database_error(format!("{} - {}", $operation, $err));
        tracing::error!("{}", formatted_error);
    }};
}

/// Log a colored error based on error type
#[macro_export]
macro_rules! log_colored_error {
    ($error_type:expr, $message:expr) => {{
        let formatted_error = $crate::logging::format_colored_error($error_type, $message);
        tracing::error!("{}", formatted_error);
    }};
}

/// Create a span for HTTP requests
#[macro_export]
macro_rules! http_span {
    ($method:expr, $uri:expr) => {
        tracing::info_span!(
            "http_request",
            method = %$method,
            uri = %$uri,
            status = tracing::field::Empty,
            latency = tracing::field::Empty,
        )
    };
}

/// Log an error with context
#[macro_export]
macro_rules! log_error {
    ($err:expr) => {
        tracing::error!(error = ?$err, "Error occurred");
    };
    ($err:expr, $msg:expr) => {
        tracing::error!(error = ?$err, $msg);
    };
}

/// Log database operations
#[macro_export]
macro_rules! db_span {
    ($operation:expr) => {
        tracing::debug_span!("db_operation", operation = $operation)
    };
    ($operation:expr, $details:expr) => {
        tracing::debug_span!("db_operation", operation = $operation, details = %$details)
    };
}

/// Helper macro for debug logging with automatic location info
/// Usage: debug_log!("Message") or debug_log!("Message with {}", variable)
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        tracing::debug!(
            target: module_path!(),
            $($arg)*
        )
    };
}

/// Helper macro for error logging with automatic location info
/// Usage: error_log!("Message") or error_log!("Message with {}", variable)
#[macro_export]
macro_rules! error_log {
    ($($arg:tt)*) => {
        tracing::error!(
            target: module_path!(),
            $($arg)*
        )
    };
}

/// Helper macro for info logging with automatic location info
/// Usage: info_log!("Message") or info_log!("Message with {}", variable)
#[macro_export]
macro_rules! info_log {
    ($($arg:tt)*) => {
        tracing::info!(
            target: module_path!(),
            $($arg)*
        )
    };
}

/// Helper macro for warning logging with automatic location info
/// Usage: warn_log!("Message") or warn_log!("Message with {}", variable)
#[macro_export]
macro_rules! warn_log {
    ($($arg:tt)*) => {
        tracing::warn!(
            target: module_path!(),
            $($arg)*
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_does_not_panic() {
        // This test just ensures init() doesn't panic
        // In a real test environment, you might want to use a test subscriber
        init();
    }

    #[test]
    fn test_format_http_status_2xx() {
        // Test success status codes (should be green)
        let formatted = format_http_status(200);
        assert!(formatted.contains("200"));
        assert!(formatted.contains("\x1b[32m")); // Green color
        assert!(formatted.contains("\x1b[0m")); // Reset color

        let formatted = format_http_status(201);
        assert!(formatted.contains("201"));
        assert!(formatted.contains("\x1b[32m")); // Green color

        let formatted = format_http_status(204);
        assert!(formatted.contains("204"));
        assert!(formatted.contains("\x1b[32m")); // Green color
    }

    #[test]
    fn test_format_http_status_3xx() {
        // Test redirect status codes (should be yellow)
        let formatted = format_http_status(301);
        assert!(formatted.contains("301"));
        assert!(formatted.contains("\x1b[33m")); // Yellow color
        assert!(formatted.contains("\x1b[0m")); // Reset color

        let formatted = format_http_status(302);
        assert!(formatted.contains("302"));
        assert!(formatted.contains("\x1b[33m")); // Yellow color

        let formatted = format_http_status(304);
        assert!(formatted.contains("304"));
        assert!(formatted.contains("\x1b[33m")); // Yellow color
    }

    #[test]
    fn test_format_http_status_4xx() {
        // Test client error status codes (should be orange)
        let formatted = format_http_status(400);
        assert!(formatted.contains("400"));
        assert!(formatted.contains("\x1b[38;5;214m")); // Orange color
        assert!(formatted.contains("\x1b[0m")); // Reset color

        let formatted = format_http_status(404);
        assert!(formatted.contains("404"));
        assert!(formatted.contains("\x1b[38;5;214m")); // Orange color

        let formatted = format_http_status(422);
        assert!(formatted.contains("422"));
        assert!(formatted.contains("\x1b[38;5;214m")); // Orange color
    }

    #[test]
    fn test_format_http_status_5xx() {
        // Test server error status codes (should be red)
        let formatted = format_http_status(500);
        assert!(formatted.contains("500"));
        assert!(formatted.contains("\x1b[31m")); // Red color
        assert!(formatted.contains("\x1b[0m")); // Reset color

        let formatted = format_http_status(502);
        assert!(formatted.contains("502"));
        assert!(formatted.contains("\x1b[31m")); // Red color

        let formatted = format_http_status(503);
        assert!(formatted.contains("503"));
        assert!(formatted.contains("\x1b[31m")); // Red color
    }

    #[test]
    fn test_format_database_error() {
        // Test database error formatting (should be light orange)
        let error_message = "Connection failed";
        let formatted = format_database_error(error_message);

        assert!(formatted.contains("Database error:"));
        assert!(formatted.contains(error_message));
        assert!(formatted.contains("\x1b[38;5;215m")); // Light orange color
        assert!(formatted.contains("\x1b[0m")); // Reset color
    }

    #[test]
    fn test_format_colored_error_database() {
        // Test database error type
        let error_message = "Table not found";
        let formatted = format_colored_error("database", error_message);

        assert!(formatted.contains("Database error:"));
        assert!(formatted.contains(error_message));
        assert!(formatted.contains("\x1b[38;5;215m")); // Light orange color
        assert!(formatted.contains("\x1b[0m")); // Reset color
    }

    #[test]
    fn test_format_colored_error_network() {
        // Test network/http error type
        let error_message = "Connection timeout";
        let formatted = format_colored_error("http", error_message);

        assert!(formatted.contains("Network error:"));
        assert!(formatted.contains(error_message));
        assert!(formatted.contains("\x1b[38;5;214m")); // Orange color
        assert!(formatted.contains("\x1b[0m")); // Reset color
    }

    #[test]
    fn test_format_colored_error_internal() {
        // Test internal/server error type
        let error_message = "Internal server error";
        let formatted = format_colored_error("internal", error_message);

        assert!(formatted.contains("Internal error:"));
        assert!(formatted.contains(error_message));
        assert!(formatted.contains("\x1b[31m")); // Red color
        assert!(formatted.contains("\x1b[0m")); // Reset color
    }

    #[test]
    fn test_format_colored_error_unknown() {
        // Test unknown error type (default case)
        let error_message = "Unknown error occurred";
        let formatted = format_colored_error("unknown", error_message);

        assert!(formatted.contains("Error:"));
        assert!(formatted.contains(error_message));
        assert!(formatted.contains("\x1b[38;5;214m")); // Orange color (default)
        assert!(formatted.contains("\x1b[0m")); // Reset color
    }

    #[test]
    fn test_edge_cases() {
        // Test edge case status codes
        let formatted = format_http_status(100); // Informational
        assert!(formatted.contains("100"));
        assert!(formatted.contains("\x1b[0m")); // Should use reset color for unknown ranges

        let formatted = format_http_status(600); // Out of standard range
        assert!(formatted.contains("600"));
        assert!(formatted.contains("\x1b[0m")); // Should use reset color for unknown ranges
    }
}
