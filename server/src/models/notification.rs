//! In-app notifications (the bell menu).
//!
//! Owns the `notification` table. Rows are created by whatever flow needs to
//! notify someone — invitations (`services/invitation.rs`), membership and
//! production routes, messages, job applications, webhooks — and read/managed
//! by `routes/notifications.rs` plus the unread-count badge in
//! `templates.rs`.

use crate::{db::DB, error::Error};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

/// One notification row for one person.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Notification {
    pub id: RecordId,
    pub person_id: RecordId,
    /// One of "invitation" | "invitation_accepted" | "member_joined" |
    /// "general" | "message" | "job_application" | "application_update" |
    /// "join_request" (schema ASSERT on `notification.notification_type`).
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

/// Query/mutation surface for the `notification` table.
pub struct NotificationModel;

impl Default for NotificationModel {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationModel {
    /// Construct the (stateless) model handle.
    pub fn new() -> Self {
        Self
    }

    /// Create an unread notification for a person. `notification_type` must
    /// satisfy the schema ASSERT (see [`Notification::notification_type`]);
    /// `related_id` is a free-form correlation key used later by
    /// [`Self::delete_by_related`].
    pub async fn create(
        &self,
        person_id: &str,
        notification_type: &str,
        title: &str,
        message: &str,
        link: Option<&str>,
        related_id: Option<&str>,
    ) -> Result<(), Error> {
        debug!("Creating notification for person {}: {}", person_id, title);

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

    /// Count a person's unread notifications (`GROUP ALL` aggregate so the
    /// count comes back as a single row).
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

    /// Fetch a person's most recent notifications, newest first.
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

    /// Mark one notification read; the `WHERE person_id = $person_id` guard
    /// makes it a no-op unless the caller owns it.
    pub async fn mark_read(&self, id: &str, person_id: &str) -> Result<(), Error> {
        debug!("Marking notification as read: {}", id);

        let id = RecordId::parse_simple(id).map_err(|e| Error::BadRequest(e.to_string()))?;
        let person_id =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query("UPDATE $id SET read = true WHERE person_id = $person_id")
            .bind(("id", id))
            .bind(("person_id", person_id))
            .await?;

        Ok(())
    }

    /// Mark every unread notification for a person as read.
    pub async fn mark_all_read(&self, person_id: &str) -> Result<(), Error> {
        debug!(
            "Marking all notifications as read for person: {}",
            person_id
        );

        let person_id =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query(
            "UPDATE notification SET read = true WHERE person_id = $person_id AND read = false",
        )
        .bind(("person_id", person_id))
        .await?;

        Ok(())
    }

    /// Delete one notification; the `WHERE person_id = $person_id` guard
    /// makes it a no-op unless the caller owns it.
    pub async fn delete(&self, id: &str, person_id: &str) -> Result<(), Error> {
        debug!("Deleting notification: {}", id);

        let id = RecordId::parse_simple(id).map_err(|e| Error::BadRequest(e.to_string()))?;
        let person_id =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query("DELETE $id WHERE person_id = $person_id")
            .bind(("id", id))
            .bind(("person_id", person_id))
            .await?;

        Ok(())
    }

    /// Delete all notifications matching a related_id and notification_type
    pub async fn delete_by_related(
        &self,
        related_id: &str,
        notification_type: &str,
    ) -> Result<(), Error> {
        debug!(
            "Deleting notifications with related_id={} type={}",
            related_id, notification_type
        );

        DB.query(
            "DELETE notification WHERE related_id = $related_id AND notification_type = $ntype",
        )
        .bind(("related_id", related_id.to_string()))
        .bind(("ntype", notification_type.to_string()))
        .await?;

        Ok(())
    }

    /// Delete every notification belonging to a person (used by account
    /// cleanup and the "clear all" action).
    pub async fn delete_all(&self, person_id: &str) -> Result<(), Error> {
        debug!("Deleting all notifications for person: {}", person_id);

        let person_id =
            RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query("DELETE notification WHERE person_id = $person_id")
            .bind(("person_id", person_id))
            .await?;

        Ok(())
    }
}
