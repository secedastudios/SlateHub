//! Versioned script uploads attached to a production.
//!
//! Owns the `production_script` table. Versions auto-increment per
//! `(production, title)`; files themselves live in S3 (this table stores
//! `file_key`/`file_url`). `uploaded_by` must be bound as a real `RecordId`
//! — SurrealDB 3.1 rejects string-encoded ids on `record<person>` fields.
//! Called from `routes::productions` (upload/visibility/delete) and the
//! management Script tab in `routes::productions_manage`.

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

/// Flat row used by [`ScriptModel::list_grouped_by_title`]. RecordId fields
/// are cast to plain strings so the row deserializes cleanly and so the
/// uploader is followed via SurrealDB's link traversal.
#[derive(Debug, Clone, Deserialize, SurrealValue)]
pub struct ScriptVersionRow {
    pub id: String,
    pub title: String,
    pub version: i64,
    pub file_url: String,
    pub file_size: i64,
    pub mime_type: String,
    pub visibility: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub uploader_username: Option<String>,
    pub uploader_name: Option<String>,
}

/// One title's worth of script versions, with the highest version pulled
/// out as `latest` and the rest in `older` (still version DESC).
#[derive(Debug, Clone)]
pub struct ScriptTitleGroup {
    pub title: String,
    pub latest: ScriptVersionRow,
    pub older: Vec<ScriptVersionRow>,
}

pub struct ScriptModel;

impl ScriptModel {
    /// Create a new script version, auto-incrementing the version number.
    ///
    /// `uploaded_by` must be a `person` [`RecordId`]: the schema types
    /// `production_script.uploaded_by` as `record<person>`, and SurrealDB
    /// 3.1+ rejects string-encoded ids ("person:xyz") with a coercion error
    /// where 3.0 silently accepted them.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        production_id: &RecordId,
        title: &str,
        file_url: &str,
        file_key: &str,
        file_size: i64,
        mime_type: &str,
        visibility: &str,
        uploaded_by: &RecordId,
        notes: Option<&str>,
    ) -> Result<ProductionScript, Error> {
        debug!(
            "Creating script '{}' for production {:?}",
            title, production_id
        );

        // Highest existing version for this (production, title), or None for
        // the first upload. Deliberately ORDER BY + LIMIT 1 instead of
        // `math::max(...) GROUP ALL`: SurrealDB 3.1 aggregates return
        // `-Infinity` for an empty group, which neither casts to int nor
        // deserializes — the plain ordered lookup has no empty-set edge case.
        let mut ver_result = DB
            .query("SELECT VALUE version FROM production_script WHERE production = $prod AND title = $title ORDER BY version DESC LIMIT 1")
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
            .bind(("uploaded_by", uploaded_by.clone()))
            .bind(("notes", notes.map(|s| s.to_string())))
            .await?
            .take(0)?;

        result.ok_or_else(|| Error::Internal("Failed to create script".to_string()))
    }

    /// Title under which a new upload should be versioned.
    ///
    /// Uploads carry no user-supplied title: the first upload starts the
    /// production's script document under `fallback_title` (the production's
    /// own title), and every subsequent upload continues the most recent
    /// document's version chain — even if the production was renamed in the
    /// meantime, so the chain never splits.
    pub async fn resolve_upload_title(
        production_id: &RecordId,
        fallback_title: &str,
    ) -> Result<String, Error> {
        // ORDER BY fields must appear in the SELECT list (SurrealDB v3),
        // so this projects a row instead of `SELECT VALUE title`.
        #[derive(serde::Deserialize, SurrealValue)]
        struct TitleRow {
            title: String,
            #[allow(dead_code)]
            created_at: DateTime<Utc>,
        }
        let mut result = DB
            .query(
                "SELECT title, created_at FROM production_script WHERE production = $prod ORDER BY created_at DESC LIMIT 1",
            )
            .bind(("prod", production_id.clone()))
            .await?;
        let latest: Option<TitleRow> = result.take(0)?;
        Ok(latest.map_or_else(|| fallback_title.to_string(), |row| row.title))
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

    /// All scripts for a production, grouped by title. Each group's `latest`
    /// is the highest version; `older` holds the rest in version DESC order.
    /// One DB round-trip; uploader info comes via record-link traversal.
    pub async fn list_grouped_by_title(
        production_id: &RecordId,
    ) -> Result<Vec<ScriptTitleGroup>, Error> {
        debug!("Listing grouped scripts for production {:?}", production_id);

        let rows: Vec<ScriptVersionRow> = DB
            .query(
                "SELECT \
                    <string> meta::id(id) AS id, \
                    title, \
                    version, \
                    file_url, \
                    file_size, \
                    mime_type, \
                    visibility, \
                    notes, \
                    created_at, \
                    uploaded_by.username AS uploader_username, \
                    uploaded_by.name AS uploader_name \
                 FROM production_script \
                 WHERE production = $prod \
                 ORDER BY title ASC, version DESC",
            )
            .bind(("prod", production_id.clone()))
            .await?
            .take(0)?;

        let mut groups: Vec<ScriptTitleGroup> = Vec::new();
        for row in rows {
            match groups.last_mut() {
                Some(g) if g.title == row.title => g.older.push(row),
                _ => groups.push(ScriptTitleGroup {
                    title: row.title.clone(),
                    latest: row,
                    older: Vec::new(),
                }),
            }
        }
        Ok(groups)
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
