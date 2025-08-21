use std::sync::LazyLock;
use surrealdb::{Surreal, engine::remote::ws::Client};
use tracing::{debug, instrument};

pub static DB: LazyLock<Surreal<Client>> = LazyLock::new(|| {
    debug!("Initializing database client");
    Surreal::init()
});

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
            tracing::error!("Database operation failed: {} - {:?}", operation, e);
            Err(e)
        }
    }
}
