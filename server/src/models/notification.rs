use crate::{db::DB, error::Error};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Notification {
    pub id: RecordId,
    pub person_id: RecordId,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub link: Option<String>,
    pub read: bool,
    pub related_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct CountResult {
    count: u32,
}

pub struct NotificationModel;

impl NotificationModel {
    pub fn new() -> Self {
        Self
    }

    pub async fn create(
        &self,
        person_id: &str,
        notification_type: &str,
        title: &str,
        message: &str,
        link: Option<&str>,
        related_id: Option<&str>,
    ) -> Result<(), Error> {
        debug!(
            "Creating notification for person {}: {}",
            person_id, title
        );

        let person_id =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query(
            "CREATE notification CONTENT {
                person_id: $person_id,
                notification_type: $notification_type,
                title: $title,
                message: $message,
                link: $link,
                related_id: $related_id,
                read: false
            }",
        )
        .bind(("person_id", person_id))
        .bind(("notification_type", notification_type.to_string()))
        .bind(("title", title.to_string()))
        .bind(("message", message.to_string()))
        .bind(("link", link.map(|s| s.to_string())))
        .bind(("related_id", related_id.map(|s| s.to_string())))
        .await?;

        Ok(())
    }

    pub async fn get_unread_count(&self, person_id: &str) -> Result<u32, Error> {
        let person_id =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        let result: Option<CountResult> = DB
            .query(
                "SELECT count() AS count FROM notification WHERE person_id = $person_id AND read = false GROUP ALL",
            )
            .bind(("person_id", person_id))
            .await?
            .take(0)?;

        Ok(result.map(|r| r.count).unwrap_or(0))
    }

    pub async fn get_recent(
        &self,
        person_id: &str,
        limit: u32,
    ) -> Result<Vec<Notification>, Error> {
        let person_id =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        let notifications: Vec<Notification> = DB
            .query(
                "SELECT * FROM notification WHERE person_id = $person_id ORDER BY created_at DESC LIMIT $limit",
            )
            .bind(("person_id", person_id))
            .bind(("limit", limit))
            .await?
            .take(0)?;

        Ok(notifications)
    }

    pub async fn mark_read(&self, id: &str) -> Result<(), Error> {
        debug!("Marking notification as read: {}", id);

        let id = RecordId::parse_simple(id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query("UPDATE $id SET read = true")
            .bind(("id", id))
            .await?;

        Ok(())
    }

    pub async fn mark_all_read(&self, person_id: &str) -> Result<(), Error> {
        debug!("Marking all notifications as read for person: {}", person_id);

        let person_id =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query("UPDATE notification SET read = true WHERE person_id = $person_id AND read = false")
            .bind(("person_id", person_id))
            .await?;

        Ok(())
    }
}
