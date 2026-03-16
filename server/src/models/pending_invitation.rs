use crate::{db::DB, error::Error};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct PendingInvitation {
    pub id: RecordId,
    pub email: String,
    pub target_type: String,
    pub target_id: String,
    pub target_name: String,
    pub target_slug: String,
    pub role: String,
    pub invited_by: RecordId,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    // Production invite extras
    #[serde(default)]
    #[surreal(default)]
    pub production_role: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub relation_type: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub department: Option<String>,
}

pub struct PendingInvitationModel;

impl PendingInvitationModel {
    pub fn new() -> Self {
        Self
    }

    pub async fn create(
        &self,
        email: &str,
        target_type: &str,
        target_id: &str,
        target_name: &str,
        target_slug: &str,
        role: &str,
        invited_by: &str,
    ) -> Result<PendingInvitation, Error> {
        debug!(
            "Creating pending invitation for {} to {} '{}'",
            email, target_type, target_name
        );

        let invited_by =
            RecordId::parse_simple(invited_by).map_err(|e| Error::BadRequest(e.to_string()))?;

        let result: Option<PendingInvitation> = DB
            .query(
                "CREATE pending_invitation CONTENT {
                    email: $email,
                    target_type: $target_type,
                    target_id: $target_id,
                    target_name: $target_name,
                    target_slug: $target_slug,
                    role: $role,
                    invited_by: $invited_by,
                    status: 'pending'
                }",
            )
            .bind(("email", email.to_string()))
            .bind(("target_type", target_type.to_string()))
            .bind(("target_id", target_id.to_string()))
            .bind(("target_name", target_name.to_string()))
            .bind(("target_slug", target_slug.to_string()))
            .bind(("role", role.to_string()))
            .bind(("invited_by", invited_by))
            .await?
            .take(0)?;

        result.ok_or_else(|| Error::Internal("Failed to create pending invitation".to_string()))
    }

    pub async fn find_pending_by_email(
        &self,
        email: &str,
    ) -> Result<Vec<PendingInvitation>, Error> {
        debug!("Finding pending invitations for email: {}", email);

        let invitations: Vec<PendingInvitation> = DB
            .query(
                "SELECT * FROM pending_invitation WHERE email = $email AND status = 'pending' ORDER BY created_at DESC",
            )
            .bind(("email", email.to_string()))
            .await?
            .take(0)?;

        Ok(invitations)
    }

    pub async fn mark_accepted(&self, id: &str) -> Result<(), Error> {
        debug!("Marking pending invitation as accepted: {}", id);

        let id = RecordId::parse_simple(id).map_err(|e| Error::BadRequest(e.to_string()))?;

        DB.query("UPDATE $id SET status = 'accepted'")
            .bind(("id", id))
            .await?;

        Ok(())
    }

    pub async fn create_for_production(
        &self,
        email: &str,
        target_id: &str,
        target_name: &str,
        target_slug: &str,
        role: &str,
        invited_by: &str,
        production_role: Option<&str>,
    ) -> Result<PendingInvitation, Error> {
        debug!(
            "Creating pending production invitation for {} to '{}'",
            email, target_name
        );

        let invited_by =
            RecordId::parse_simple(invited_by).map_err(|e| Error::BadRequest(e.to_string()))?;

        let result: Option<PendingInvitation> = DB
            .query(
                "CREATE pending_invitation CONTENT {
                    email: $email,
                    target_type: 'production',
                    target_id: $target_id,
                    target_name: $target_name,
                    target_slug: $target_slug,
                    role: $role,
                    invited_by: $invited_by,
                    status: 'pending',
                    production_role: $production_role
                }",
            )
            .bind(("email", email.to_string()))
            .bind(("target_id", target_id.to_string()))
            .bind(("target_name", target_name.to_string()))
            .bind(("target_slug", target_slug.to_string()))
            .bind(("role", role.to_string()))
            .bind(("invited_by", invited_by))
            .bind(("production_role", production_role.map(|s| s.to_string())))
            .await?
            .take(0)?;

        result.ok_or_else(|| Error::Internal("Failed to create pending invitation".to_string()))
    }

    pub async fn find_existing(
        &self,
        email: &str,
        target_id: &str,
    ) -> Result<Option<PendingInvitation>, Error> {
        debug!(
            "Checking for existing pending invitation: {} -> {}",
            email, target_id
        );

        let result: Option<PendingInvitation> = DB
            .query(
                "SELECT * FROM pending_invitation WHERE email = $email AND target_id = $target_id AND status = 'pending' LIMIT 1",
            )
            .bind(("email", email.to_string()))
            .bind(("target_id", target_id.to_string()))
            .await?
            .take(0)?;

        Ok(result)
    }
}
