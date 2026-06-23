//! The process-wide SurrealDB connection.
//!
//! One WebSocket client, shared everywhere: `main` connects/authenticates
//! [`DB`] at boot (and the test harness points it at the test container),
//! after which models and services issue queries through `DB.query(...)`
//! directly. The SDK multiplexes concurrent queries over the single
//! connection, so no pool is needed.

use crate::log_db_error;
use std::sync::LazyLock;
use surrealdb::{Surreal, engine::remote::ws::Client};
use tracing::{debug, info, instrument};

/// Global SurrealDB handle. Unconnected until `main` (or a test's
/// `setup_test_db`) calls `DB.connect(...)` + `signin` + `use_ns/use_db`;
/// queries issued before that return a connection error rather than panic.
pub static DB: LazyLock<Surreal<Client>> = LazyLock::new(|| {
    debug!("Initializing database client");
    Surreal::init()
});

/// Ensures the database client is initialized and ready
pub async fn ensure_db_initialized() -> Result<(), surrealdb::Error> {
    // Force initialization of the LazyLock if not already done
    let _ = &*DB;

    // Verify we can perform a basic operation
    debug!("Verifying database connection is ready");
    match DB.query("INFO FOR NS").await {
        Ok(_) => {
            info!("Database connection verified and ready");
            Ok(())
        }
        Err(e) => {
            log_db_error!(
                format!("{:?}", e),
                "Database connection verification failed"
            );
            Err(e)
        }
    }
}

/// Helper function to log database operations
#[instrument(skip_all)]
pub async fn log_db_operation<T, F>(operation: &str, f: F) -> Result<T, surrealdb::Error>
where
    F: std::future::Future<Output = Result<T, surrealdb::Error>>,
{
    debug!("Starting database operation: {}", operation);
    match f.await {
        Ok(result) => {
            debug!("Database operation completed successfully: {}", operation);
            Ok(result)
        }
        Err(e) => {
            log_db_error!(format!("{:?}", e), operation);
            Err(e)
        }
    }
}
