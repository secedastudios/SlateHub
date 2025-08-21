use std::env;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize the tracing subscriber for logging
pub fn init() {
    // Get log format from environment, default to "pretty"
    let log_format = env::var("LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string());

    // Create env filter from RUST_LOG or use default
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default log level configuration
        EnvFilter::new("info,slatehub=debug,tower_http=debug")
    });

    match log_format.as_str() {
        "json" => {
            // JSON formatted logs - useful for production and log aggregation
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().json())
                .init();
        }
        "compact" => {
            // Compact format - less verbose than pretty
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().compact())
                .init();
        }
        _ => {
            // Pretty format (default) - good for development
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().pretty())
                .init();
        }
    }

    tracing::info!("Logging initialized with format: {}", log_format);
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
