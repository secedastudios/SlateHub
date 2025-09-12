//! Media model for handling uploaded files and profile images
//!
//! This module manages media uploads, storage in MinIO/S3, and database records.

use crate::db::DB;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use tracing::{debug, info};
use uuid::Uuid;

/// Media record structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    /// The unique identifier for the media record
    pub id: RecordId,
    /// Type of media (e.g., "profile_image", "reel", "resume")
    pub media_type: String,
    /// Original filename
    pub filename: String,
    /// MIME type (e.g., "image/jpeg")
    pub mime_type: String,
    /// File size in bytes
    pub size: i64,
    /// S3/MinIO bucket name
    pub bucket: String,
    /// S3/MinIO object key/path
    pub object_key: String,
    /// Public URL if available
    pub url: Option<String>,
    /// Thumbnail URL if applicable
    pub thumbnail_url: Option<String>,
    /// Image dimensions if applicable
    pub dimensions: Option<MediaDimensions>,
    /// Upload timestamp
    pub uploaded_at: String,
    /// Owner of the media (person record ID)
    pub uploaded_by: RecordId,
}

/// Media dimensions for images/videos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDimensions {
    pub width: u32,
    pub height: u32,
}

/// Input for creating a new media record
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMediaInput {
    pub media_type: String,
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub bucket: String,
    pub object_key: String,
    pub url: Option<String>,
    pub dimensions: Option<MediaDimensions>,
    pub uploaded_by: String, // Person ID as string
}

impl Media {
    /// Create a new media record in the database and return the full record ID
    pub async fn create(input: CreateMediaInput) -> Result<String> {
        debug!("Creating media record for file: {}", input.filename);

        // Generate our own ID to avoid deserialization issues
        let id_part = Uuid::new_v4().to_string().replace("-", "");
        let media_id = format!("media:{}", id_part);

        // Ensure uploaded_by is the full record ID like "person:id"
        let uploaded_by_record = if input.uploaded_by.starts_with("person:") {
            input.uploaded_by.clone()
        } else {
            format!("person:{}", input.uploaded_by)
        };

        // Create the media record using the SDK's create method
        #[derive(serde::Serialize)]
        struct MediaData {
            media_type: String,
            filename: String,
            mime_type: String,
            size: i64,
            bucket: String,
            object_key: String,
            url: Option<String>,
            dimensions: Option<MediaDimensions>,
            uploaded_at: String,
            uploaded_by: String,
        }

        let data = MediaData {
            media_type: input.media_type,
            filename: input.filename,
            mime_type: input.mime_type,
            size: input.size,
            bucket: input.bucket,
            object_key: input.object_key,
            url: input.url,
            dimensions: input.dimensions,
            uploaded_at: chrono::Utc::now().to_rfc3339(),
            uploaded_by: uploaded_by_record,
        };

        // Use the SDK's create method with a specific ID
        let _result: Option<serde_json::Value> = DB
            .create(("media", id_part.clone()))
            .content(data)
            .await
            .map_err(|e| Error::database(format!("Failed to create media record: {}", e)))?;

        info!("Created media record with ID: {}", media_id);
        Ok(media_id)
    }

    /// Find a media record by ID
    pub async fn find_by_id(id: &str) -> Result<Option<Self>> {
        debug!("Finding media by ID: {}", id);

        let sql = "SELECT * FROM media WHERE id = type::thing('media', $id)";

        let mut response = DB.query(sql).bind(("id", id.to_string())).await?;

        let media: Vec<Self> = response.take(0)?;
        Ok(media.into_iter().next())
    }

    /// Set a media record as a person's profile image
    pub async fn set_as_profile_image(media_id: &str, person_id: &str) -> Result<()> {
        debug!(
            "Setting media {} as profile image for person {}",
            media_id, person_id
        );

        // Ensure we have full record IDs
        let person_record = if person_id.starts_with("person:") {
            person_id.to_string()
        } else {
            format!("person:{}", person_id)
        };

        let media_record = if media_id.starts_with("media:") {
            media_id.to_string()
        } else {
            format!("media:{}", media_id)
        };

        // First, remove any existing profile image relationship
        let remove_sql = format!("DELETE person_profile_image WHERE in = {}", person_record);
        DB.query(&remove_sql).await?;

        // Create the new relationship with RETURN NONE to avoid deserialization issues
        let relate_sql = format!(
            "RELATE {}->person_profile_image->{} SET created_at = time::now() RETURN NONE",
            person_record, media_record
        );
        DB.query(&relate_sql).await?;
        debug!(
            "Created relationship between {} and {}",
            person_record, media_record
        );

        // Update the person's profile.avatar field with RETURN NONE
        let update_sql = format!(
            "UPDATE {} SET profile.avatar = {} RETURN NONE",
            person_record, media_record
        );
        DB.query(&update_sql).await?;
        debug!("Updated profile.avatar for {}", person_record);

        info!("Profile image updated for person {}", person_id);
        Ok(())
    }

    /// Get a person's current profile image
    pub async fn get_profile_image(person_id: &str) -> Result<Option<Self>> {
        debug!("Getting profile image for person {}", person_id);

        // Ensure we have full record ID
        let person_record = if person_id.starts_with("person:") {
            person_id.to_string()
        } else {
            format!("person:{}", person_id)
        };

        let sql = format!(
            "
            SELECT out.* FROM person_profile_image
            WHERE in = {}
            LIMIT 1
        ",
            person_record
        );

        let mut response = DB.query(&sql).await?;

        let media: Vec<Self> = response.take(0)?;
        Ok(media.into_iter().next())
    }

    /// Delete a media record and its S3 object
    pub async fn delete(id: &str) -> Result<()> {
        debug!("Deleting media record: {}", id);

        // TODO: Delete the actual file from S3/MinIO
        // This will require the S3 client to be passed in or available globally

        let sql = "DELETE type::thing('media', $id)";

        DB.query(sql).bind(("id", id.to_string())).await?;

        info!("Media record {} deleted", id);
        Ok(())
    }

    /// Get all media for a person
    pub async fn get_person_media(person_id: &str, media_type: Option<&str>) -> Result<Vec<Self>> {
        debug!("Getting media for person: {}", person_id);

        let sql = if let Some(_mt) = media_type {
            "SELECT * FROM media WHERE uploaded_by = type::thing('person', $person_id) AND media_type = $media_type ORDER BY uploaded_at DESC"
        } else {
            "SELECT * FROM media WHERE uploaded_by = type::thing('person', $person_id) ORDER BY uploaded_at DESC"
        };

        let mut query = DB.query(sql).bind(("person_id", person_id.to_string()));

        if let Some(mt) = media_type {
            query = query.bind(("media_type", mt.to_string()));
        }

        let mut response = query.await?;
        let media: Vec<Self> = response.take(0)?;

        Ok(media)
    }
}

// TODO: Future enhancements
// - Add image processing (resize, optimize)
// - Generate thumbnails automatically
// - Add virus scanning
// - Add file type validation
// - Add storage quota management
// - Add CDN URL generation
// - Add batch upload support
// - Add media metadata extraction (EXIF, etc.)
