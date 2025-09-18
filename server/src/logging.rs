use std::env;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

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
}
