//! System model for system-level operations
//!
//! This module provides functionality for system health checks,
//! database status, and other system-level operations.

use crate::db::DB;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

/// System information and status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub database_status: DatabaseStatus,
    pub namespace: String,
    pub database: String,
}

/// Database connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatus {
    pub connected: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub database: String,
    pub version: String,
    pub timestamp: String,
}

/// System model for system-level operations
pub struct System;

impl System {
    /// Check database connectivity and health
    ///
    /// # Returns
    /// A `Result` containing the `DatabaseStatus`
    pub async fn check_database_health() -> Result<DatabaseStatus> {
        debug!("Checking database health");

        match DB.health().await {
            Ok(_) => {
                info!("Database health check: OK");
                Ok(DatabaseStatus {
                    connected: true,
                    message: "Database is healthy".to_string(),
                    error: None,
                })
            }
            Err(e) => {
                error!("Database health check failed: {:?}", e);
                Ok(DatabaseStatus {
                    connected: false,
                    message: "Database connection failed".to_string(),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Get overall system health status
    ///
    /// # Returns
    /// A `Result` containing the `HealthStatus`
    pub async fn health_check() -> Result<HealthStatus> {
        debug!("System health check requested");

        let db_status = Self::check_database_health().await?;

        let status = if db_status.connected {
            "healthy"
        } else {
            "degraded"
        };

        let database = if db_status.connected {
            "connected"
        } else {
            "disconnected"
        };

        let version = env!("CARGO_PKG_VERSION").to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        Ok(HealthStatus {
            status: status.to_string(),
            database: database.to_string(),
            version,
            timestamp,
        })
    }

    /// Get system information
    ///
    /// # Returns
    /// A `Result` containing the `SystemInfo`
    pub async fn get_system_info() -> Result<SystemInfo> {
        use std::env;

        debug!("Getting system information");

        let database_status = Self::check_database_health().await?;
        let namespace = env::var("DB_NAMESPACE").unwrap_or_else(|_| "unknown".to_string());
        let database = env::var("DB_NAME").unwrap_or_else(|_| "unknown".to_string());
        let version = env!("CARGO_PKG_VERSION").to_string();

        Ok(SystemInfo {
            version,
            database_status,
            namespace,
            database,
        })
    }

    /// Get database statistics
    ///
    /// # Returns
    /// A `Result` containing database statistics as a JSON value
    pub async fn get_database_stats() -> Result<serde_json::Value> {
        debug!("Getting database statistics");

        // Query database info
        let sql = "INFO FOR DB";
        let mut response = DB.query(sql).await?;

        let stats: Option<serde_json::Value> = response.take(0)?;

        Ok(stats.unwrap_or_else(|| {
            serde_json::json!({
                "error": "Unable to retrieve database statistics"
            })
        }))
    }

    /// Verify database connection is properly configured
    ///
    /// # Returns
    /// A `Result` containing true if properly configured
    pub async fn verify_database_connection() -> Result<bool> {
        debug!("Verifying database connection configuration");

        // Try a simple query to verify we're connected and authenticated
        let sql = "RETURN 1";
        match DB.query(sql).await {
            Ok(mut response) => {
                let result: Option<i32> = response.take(0)?;
                Ok(result == Some(1))
            }
            Err(e) => {
                error!("Database connection verification failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Get current database session information
    ///
    /// # Returns
    /// A `Result` containing session info as a JSON value
    pub async fn get_session_info() -> Result<serde_json::Value> {
        debug!("Getting database session information");

        let sql = "RETURN { ns: $session.ns, db: $session.db, ac: $session.ac, au: $session.au }";
        let mut response = DB.query(sql).await?;

        let info: Option<serde_json::Value> = response.take(0)?;

        Ok(info.unwrap_or_else(|| {
            serde_json::json!({
                "error": "Unable to retrieve session information"
            })
        }))
    }

    /// Count total records in a table
    ///
    /// # Arguments
    /// * `table` - The table name to count records from
    ///
    /// # Returns
    /// A `Result` containing the count
    pub async fn count_records(table: &str) -> Result<usize> {
        debug!("Counting records in table: {}", table);

        // Validate table name to prevent injection
        if !table.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(crate::error::Error::BadRequest(
                "Invalid table name".to_string(),
            ));
        }

        let sql = format!("SELECT count() FROM {} GROUP ALL", table);
        let mut response = DB.query(&sql).await?;

        #[derive(Deserialize)]
        struct CountResult {
            count: usize,
        }

        let results: Vec<CountResult> = response.take(0)?;
        Ok(results.first().map(|r| r.count).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        // This test would require a database connection
        // For now, just verify the method exists and compiles
        let _ = System::health_check().await;
    }

    #[tokio::test]
    async fn test_system_info() {
        // This test would require a database connection
        // For now, just verify the method exists and compiles
        let _ = System::get_system_info().await;
    }
}
