use crate::{db::DB, error::Error};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct PendingInvitation {
    pub id: RecordId,
    #[serde(default)]
    #[surreal(default)]
    pub email: Option<String>,
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
    pub production_roles: Option<Vec<String>>,
    #[serde(default)]
    #[surreal(default)]
    pub relation_type: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub department: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub token: Option<String>,
}

pub struct PendingInvitationModel;

/// Generate a short random token for invite links (8 chars, alphanumeric)
fn generate_invite_token() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..8).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect()
}

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
                    status: 'pending',
                    token: $invite_token
                }",
            )
            .bind(("email", email.to_string()))
            .bind(("target_type", target_type.to_string()))
            .bind(("target_id", target_id.to_string()))
            .bind(("target_name", target_name.to_string()))
            .bind(("target_slug", target_slug.to_string()))
            .bind(("role", role.to_string()))
            .bind(("invited_by", invited_by))
            .bind(("invite_token", generate_invite_token()))
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
        production_roles: Option<&[String]>,
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
                    production_roles: $production_roles,
                    token: $invite_token
                }",
            )
            .bind(("email", email.to_string()))
            .bind(("target_id", target_id.to_string()))
            .bind(("target_name", target_name.to_string()))
            .bind(("target_slug", target_slug.to_string()))
            .bind(("role", role.to_string()))
            .bind(("invited_by", invited_by))
            .bind(("production_roles", production_roles.map(|s| s.to_vec())))
            .bind(("invite_token", generate_invite_token()))
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

    /// Create a link-only invite (no email) for a production
    pub async fn create_link_invite(
        &self,
        target_id: &str,
        target_name: &str,
        target_slug: &str,
        role: &str,
        invited_by: &str,
        production_roles: Option<&[String]>,
    ) -> Result<PendingInvitation, Error> {
        debug!("Creating link-only invite for production '{}'", target_name);

        let invited_by =
            RecordId::parse_simple(invited_by).map_err(|e| Error::BadRequest(e.to_string()))?;

        let result: Option<PendingInvitation> = DB
            .query(
                "CREATE pending_invitation CONTENT {
                    target_type: 'production',
                    target_id: $target_id,
                    target_name: $target_name,
                    target_slug: $target_slug,
                    role: $role,
                    invited_by: $invited_by,
                    status: 'pending',
                    production_roles: $production_roles,
                    token: $invite_token
                }",
            )
            .bind(("target_id", target_id.to_string()))
            .bind(("target_name", target_name.to_string()))
            .bind(("target_slug", target_slug.to_string()))
            .bind(("role", role.to_string()))
            .bind(("invited_by", invited_by))
            .bind(("production_roles", production_roles.map(|s| s.to_vec())))
            .bind(("invite_token", generate_invite_token()))
            .await?
            .take(0)?;

        result.ok_or_else(|| Error::Internal("Failed to create link invite".to_string()))
    }

    /// Find a pending invitation by its token
    pub async fn find_by_token(
        &self,
        token: &str,
    ) -> Result<Option<PendingInvitation>, Error> {
        debug!("Finding pending invitation by token: {}", token);

        // Raw query first to check if record exists
        let exists: Option<serde_json::Value> = DB
            .query("SELECT <string> id AS id, `token`, status, target_slug FROM pending_invitation WHERE `token` = $inv_token AND status = 'pending' LIMIT 1")
            .bind(("inv_token", token.to_string()))
            .await?
            .take(0)?;
        tracing::warn!("INVITE_LOOKUP raw={:?}", exists);

        // Full deserialization
        let result: Option<PendingInvitation> = match DB
            .query("SELECT * FROM pending_invitation WHERE `token` = $inv_token AND status = 'pending' LIMIT 1")
            .bind(("inv_token", token.to_string()))
            .await
        {
            Ok(mut response) => {
                match response.take::<Option<PendingInvitation>>(0) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("INVITE_LOOKUP deser_fail token={} err={}", token, e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("INVITE_LOOKUP query_fail token={} err={}", token, e);
                return Err(Error::Database(e.to_string()));
            }
        };

        Ok(result)
    }

    /// Get all pending email invitations for a production
    pub async fn get_pending_for_production(
        &self,
        production_id: &str,
    ) -> Result<Vec<PendingInvitation>, Error> {
        debug!("Fetching pending email invitations for production: {}", production_id);

        let invitations: Vec<PendingInvitation> = DB
            .query(
                "SELECT * FROM pending_invitation WHERE target_id = $target_id AND target_type = 'production' AND status = 'pending' ORDER BY created_at DESC",
            )
            .bind(("target_id", production_id.to_string()))
            .await?
            .take(0)?;

        Ok(invitations)
    }
}
