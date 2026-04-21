use crate::{db::DB, error::Error};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct ProductionScript {
    pub id: RecordId,
    pub production: RecordId,
    pub title: String,
    pub version: i64,
    pub file_url: String,
    pub file_key: String,
    pub file_size: i64,
    pub mime_type: String,
    pub visibility: String,
    pub uploaded_by: RecordId,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct ScriptModel;

impl ScriptModel {
    /// Create a new script version, auto-incrementing the version number
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        production_id: &RecordId,
        title: &str,
        file_url: &str,
        file_key: &str,
        file_size: i64,
        mime_type: &str,
        visibility: &str,
        uploaded_by: &str,
        notes: Option<&str>,
    ) -> Result<ProductionScript, Error> {
        debug!(
            "Creating script '{}' for production {:?}",
            title, production_id
        );

        // Get next version number
        let mut ver_result = DB
            .query("SELECT VALUE math::max(version) FROM production_script WHERE production = $prod AND title = $title")
            .bind(("prod", production_id.clone()))
            .bind(("title", title.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to get max version: {}", e)))?;

        let max_ver: Option<i64> = ver_result.take(0)?;
        let next_version = max_ver.unwrap_or(0) + 1;

        let result: Option<ProductionScript> = DB
            .query(
                "CREATE production_script CONTENT {
                    production: $production,
                    title: $title,
                    version: $version,
                    file_url: $file_url,
                    file_key: $file_key,
                    file_size: $file_size,
                    mime_type: $mime_type,
                    visibility: $visibility,
                    uploaded_by: $uploaded_by,
                    notes: $notes
                }",
            )
            .bind(("production", production_id.clone()))
            .bind(("title", title.to_string()))
            .bind(("version", next_version))
            .bind(("file_url", file_url.to_string()))
            .bind(("file_key", file_key.to_string()))
            .bind(("file_size", file_size))
            .bind(("mime_type", mime_type.to_string()))
            .bind(("visibility", visibility.to_string()))
            .bind(("uploaded_by", uploaded_by.to_string()))
            .bind(("notes", notes.map(|s| s.to_string())))
            .await?
            .take(0)?;

        result.ok_or_else(|| Error::Internal("Failed to create script".to_string()))
    }

    /// Get latest version of each script for a production
    pub async fn get_latest_for_production(
        production_id: &RecordId,
    ) -> Result<Vec<ProductionScript>, Error> {
        debug!("Getting latest scripts for production {:?}", production_id);

        // Get all scripts, then deduplicate by title keeping highest version
        let scripts: Vec<ProductionScript> = DB
            .query(
                "SELECT * FROM production_script WHERE production = $prod ORDER BY title ASC, version DESC",
            )
            .bind(("prod", production_id.clone()))
            .await?
            .take(0)?;

        // Keep only the first (highest version) for each title
        let mut seen_titles = std::collections::HashSet::new();
        let latest: Vec<ProductionScript> = scripts
            .into_iter()
            .filter(|s| seen_titles.insert(s.title.clone()))
            .collect();

        Ok(latest)
    }

    /// Get all versions of a specific script by title
    pub async fn get_versions(
        production_id: &RecordId,
        title: &str,
    ) -> Result<Vec<ProductionScript>, Error> {
        let scripts: Vec<ProductionScript> = DB
            .query(
                "SELECT * FROM production_script WHERE production = $prod AND title = $title ORDER BY version DESC",
            )
            .bind(("prod", production_id.clone()))
            .bind(("title", title.to_string()))
            .await?
            .take(0)?;

        Ok(scripts)
    }

    /// Get a single script by ID
    pub async fn get(script_id: &RecordId) -> Result<Option<ProductionScript>, Error> {
        let script: Option<ProductionScript> = DB
            .query("SELECT * FROM $id")
            .bind(("id", script_id.clone()))
            .await?
            .take(0)?;

        Ok(script)
    }

    /// Update script visibility
    pub async fn update_visibility(script_id: &RecordId, visibility: &str) -> Result<(), Error> {
        DB.query("UPDATE $id SET visibility = $visibility")
            .bind(("id", script_id.clone()))
            .bind(("visibility", visibility.to_string()))
            .await?;

        Ok(())
    }

    /// Delete a script version
    pub async fn delete(script_id: &RecordId) -> Result<Option<String>, Error> {
        // Get file_key before deleting so caller can clean up S3
        let script = Self::get(script_id).await?;
        let file_key = script.map(|s| s.file_key);

        DB.query("DELETE $id")
            .bind(("id", script_id.clone()))
            .await?;

        Ok(file_key)
    }
}
