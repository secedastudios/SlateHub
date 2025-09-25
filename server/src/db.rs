use crate::log_db_error;
use std::sync::LazyLock;
use surrealdb::{Surreal, engine::remote::ws::Client};
use tracing::{debug, info, instrument};

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
