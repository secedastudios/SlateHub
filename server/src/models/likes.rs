use crate::{db::DB, error::Error, record_id_ext::RecordIdExt};
use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;
use tracing::debug;

pub struct LikesModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikedPerson {
    pub id: String,
    pub username: String,
    pub name: String,
    pub avatar: Option<String>,
    pub headline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikedLocation {
    pub id: String,
    pub name: String,
    pub city: String,
    pub state: String,
    pub profile_photo: Option<String>,
}

impl LikesModel {
    /// Toggle a like. Returns true if now liked, false if unliked.
    pub async fn toggle(person_id: &RecordId, target_id: &RecordId) -> Result<bool, Error> {
        debug!(
            "Toggling like: {} -> {}",
            person_id.display(),
            target_id.display()
        );

        // Check if like already exists
        let exists = Self::is_liked(person_id, target_id).await?;

        if exists {
            // Delete the like
            let query = "DELETE likes WHERE in = $person_id AND out = $target_id";
            DB.query(query)
                .bind(("person_id", person_id.clone()))
                .bind(("target_id", target_id.clone()))
                .await
                .map_err(|e| Error::Database(format!("Failed to delete like: {}", e)))?;
            Ok(false)
        } else {
            // Create the like
            let query = "RELATE $person_id -> likes -> $target_id SET created_at = time::now()";
            DB.query(query)
                .bind(("person_id", person_id.clone()))
                .bind(("target_id", target_id.clone()))
                .await
                .map_err(|e| Error::Database(format!("Failed to create like: {}", e)))?;
            Ok(true)
        }
    }

    /// Check if a person has liked a target
    pub async fn is_liked(person_id: &RecordId, target_id: &RecordId) -> Result<bool, Error> {
        let query = "SELECT count() AS count FROM likes WHERE in = $person_id AND out = $target_id";
        let mut result = DB
            .query(query)
            .bind(("person_id", person_id.clone()))
            .bind(("target_id", target_id.clone()))
            .await
            .map_err(|e| Error::Database(format!("Failed to check like: {}", e)))?;

        let count: Option<serde_json::Value> = result.take(0)?;
        Ok(count
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0)
            > 0)
    }

    /// Get liked target IDs for a person, filtered to given targets.
    /// Returns a set of raw ID strings like "person:abc" or "location:xyz".
    pub async fn get_liked_ids(
        person_id: &RecordId,
        target_ids: &[RecordId],
    ) -> Result<Vec<String>, Error> {
        if target_ids.is_empty() {
            return Ok(vec![]);
        }

        let query = "SELECT VALUE out FROM likes WHERE in = $person_id AND out IN $target_ids";
        let mut result = DB
            .query(query)
            .bind(("person_id", person_id.clone()))
            .bind(("target_ids", target_ids.to_vec()))
            .await
            .map_err(|e| Error::Database(format!("Failed to check likes: {}", e)))?;

        let ids: Vec<RecordId> = result.take(0).unwrap_or_default();
        Ok(ids.iter().map(|id| id.to_raw_string()).collect())
    }

    /// Count liked people for a user
    pub async fn count_liked_people(person_id: &RecordId) -> Result<usize, Error> {
        let query = format!("SELECT count() AS count FROM {}->likes->person GROUP ALL", person_id.display());
        let mut result = DB.query(&query).await
            .map_err(|e| Error::Database(format!("Failed to count liked people: {}", e)))?;
        let row: Option<serde_json::Value> = result.take(0)?;
        Ok(row.and_then(|v| v.get("count").and_then(|c| c.as_u64())).unwrap_or(0) as usize)
    }

    /// Count liked locations for a user
    pub async fn count_liked_locations(person_id: &RecordId) -> Result<usize, Error> {
        let query = format!("SELECT count() AS count FROM {}->likes->location GROUP ALL", person_id.display());
        let mut result = DB.query(&query).await
            .map_err(|e| Error::Database(format!("Failed to count liked locations: {}", e)))?;
        let row: Option<serde_json::Value> = result.take(0)?;
        Ok(row.and_then(|v| v.get("count").and_then(|c| c.as_u64())).unwrap_or(0) as usize)
    }

    /// Get all liked people for a user using graph traversal
    pub async fn get_liked_people(person_id: &RecordId) -> Result<Vec<LikedPerson>, Error> {
        // Build query with record ID directly — bind params don't work in graph traversal FROM position
        let query = format!(
            "SELECT <string> id AS id, username, name, profile.avatar AS avatar, profile.headline AS headline FROM {}->likes->person",
            person_id.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(format!("Failed to get liked people: {}", e)))?;

        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        let people = rows
            .into_iter()
            .filter_map(|row| {
                let id = row.get("id")?;
                let id_str = if let Some(s) = id.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string(id).unwrap_or_default().trim_matches('"').to_string()
                };

                Some(LikedPerson {
                    id: id_str,
                    username: row.get("username").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                    name: row.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                    avatar: row.get("avatar").and_then(|v| v.as_str()).map(String::from),
                    headline: row.get("headline").and_then(|v| v.as_str()).map(String::from),
                })
            })
            .collect();

        Ok(people)
    }

    /// Get all liked locations for a user using graph traversal
    pub async fn get_liked_locations(person_id: &RecordId) -> Result<Vec<LikedLocation>, Error> {
        let query = format!(
            "SELECT <string> id AS id, name, city, state, profile_photo FROM {}->likes->location",
            person_id.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(format!("Failed to get liked locations: {}", e)))?;

        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();
        debug!("get_liked_locations rows: {:?}", rows);

        let locations = rows
            .into_iter()
            .filter_map(|row| {
                let id = row.get("id")?;
                let id_str = if let Some(s) = id.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string(id).unwrap_or_default().trim_matches('"').to_string()
                };

                // Strip the "location:" prefix for template use
                let id_clean = id_str.strip_prefix("location:").unwrap_or(&id_str).to_string();

                Some(LikedLocation {
                    id: id_clean,
                    name: row.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                    city: row.get("city").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    state: row.get("state").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    profile_photo: row.get("profile_photo").and_then(|v| v.as_str()).map(String::from),
                })
            })
            .collect();

        Ok(locations)
    }
}
