//! OAuth/OIDC client model — one per organization.

use crate::auth::{hash_password, verify_password};
use crate::db::DB;
use crate::error::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

/// Default scopes a client may request without admin opt-in.
pub const DEFAULT_ALLOWED_SCOPES: &[&str] = &[
    "openid",
    "profile",
    "email",
    "offline_access",
    "slatehub:org_membership",
];

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct OauthClient {
    pub id: RecordId,
    pub organization: RecordId,
    pub client_id: String,
    pub client_secret_hash: String,
    #[serde(default)]
    #[surreal(default)]
    pub client_secret_hash_previous: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub previous_expires_at: Option<DateTime<Utc>>,
    pub name: String,
    #[serde(default)]
    #[surreal(default)]
    pub logo_uri: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    #[surreal(default)]
    pub post_logout_redirect_uris: Vec<String>,
    #[serde(default)]
    #[surreal(default)]
    pub allowed_scopes: Vec<String>,
    #[serde(default = "default_auth_method")]
    #[surreal(default = "default_auth_method")]
    pub token_endpoint_auth_method: String,
    #[serde(default = "default_true")]
    #[surreal(default = "default_true")]
    pub require_pkce: bool,
    #[serde(default)]
    #[surreal(default)]
    pub ssf_receiver_endpoint: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub ssf_receiver_jwks_uri: Option<String>,
    #[serde(default = "default_push")]
    #[surreal(default = "default_push")]
    pub ssf_delivery_method: String,
    #[serde(default)]
    #[surreal(default)]
    pub ssf_events_subscribed: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    #[surreal(default)]
    pub rotated_at: Option<DateTime<Utc>>,
}

fn default_auth_method() -> String {
    "client_secret_basic".to_string()
}
fn default_push() -> String {
    "push".to_string()
}
fn default_true() -> bool {
    true
}

/// Generated identifiers for a freshly-created client. The plaintext secret is
/// returned only at creation time; subsequent reads see only the hash.
pub struct NewClientCredentials {
    pub client: OauthClient,
    pub plaintext_secret: String,
}

/// Generate a random opaque token of the given length using a base32 alphabet.
fn random_token(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

pub struct OauthClientModel;

impl OauthClientModel {
    pub fn new() -> Self {
        Self
    }

    /// Find the client (if any) belonging to the given organization.
    pub async fn get_for_organization(&self, org_id: &RecordId) -> Result<Option<OauthClient>> {
        let mut resp = DB
            .query("SELECT * FROM oauth_client WHERE organization = $org LIMIT 1")
            .bind(("org", org_id.clone()))
            .await?;
        let rows: Vec<OauthClient> = resp.take(0).unwrap_or_default();
        Ok(rows.into_iter().next())
    }

    pub async fn get_by_client_id(&self, client_id: &str) -> Result<Option<OauthClient>> {
        let mut resp = DB
            .query("SELECT * FROM oauth_client WHERE client_id = $cid LIMIT 1")
            .bind(("cid", client_id.to_string()))
            .await?;
        let rows: Vec<OauthClient> = resp.take(0).unwrap_or_default();
        Ok(rows.into_iter().next())
    }

    /// Create a new client for the org. Returns the model + plaintext secret
    /// (shown to admin once). Errors if a client already exists.
    pub async fn create(
        &self,
        org_id: &RecordId,
        display_name: &str,
    ) -> Result<NewClientCredentials> {
        if self.get_for_organization(org_id).await?.is_some() {
            return Err(Error::Conflict(
                "An OAuth client already exists for this organization".into(),
            ));
        }

        let client_id = format!("sh_{}", random_token(28));
        let plaintext = random_token(48);
        let secret_hash = hash_password(&plaintext)?;
        let allowed_scopes: Vec<String> = DEFAULT_ALLOWED_SCOPES
            .iter()
            .map(|s| s.to_string())
            .collect();

        debug!(client_id = %client_id, "Creating oauth_client");

        let mut resp = DB
            .query(
                "CREATE oauth_client CONTENT {
                    organization: $org,
                    client_id: $cid,
                    client_secret_hash: $hash,
                    name: $name,
                    redirect_uris: [],
                    post_logout_redirect_uris: [],
                    allowed_scopes: $scopes,
                    token_endpoint_auth_method: 'client_secret_basic',
                    require_pkce: true,
                    ssf_delivery_method: 'push',
                    ssf_events_subscribed: []
                }",
            )
            .bind(("org", org_id.clone()))
            .bind(("cid", client_id))
            .bind(("hash", secret_hash))
            .bind(("name", display_name.to_string()))
            .bind(("scopes", allowed_scopes))
            .await?;
        let created: Vec<OauthClient> = resp.take(0).unwrap_or_default();
        let client = created
            .into_iter()
            .next()
            .ok_or_else(|| Error::Internal("oauth_client create returned no row".into()))?;

        Ok(NewClientCredentials {
            client,
            plaintext_secret: plaintext,
        })
    }

    /// Rotate the secret. Old hash is preserved for `grace_hours` to allow
    /// rolling deploys; both validate during the window.
    pub async fn rotate_secret(&self, client_id: &RecordId, grace_hours: i64) -> Result<String> {
        let plaintext = random_token(48);
        let new_hash = hash_password(&plaintext)?;
        let previous_expires = Utc::now() + Duration::hours(grace_hours);
        DB.query(
            "UPDATE $id SET
                client_secret_hash_previous = client_secret_hash,
                previous_expires_at = $prev_exp,
                client_secret_hash = $hash,
                rotated_at = time::now(),
                updated_at = time::now()",
        )
        .bind(("id", client_id.clone()))
        .bind(("prev_exp", previous_expires))
        .bind(("hash", new_hash))
        .await?;
        Ok(plaintext)
    }

    /// Verify a plaintext secret against the stored hashes (current + grace-period previous).
    pub fn verify_secret(client: &OauthClient, plaintext: &str) -> bool {
        if verify_password(plaintext, &client.client_secret_hash).unwrap_or(false) {
            return true;
        }
        if let (Some(prev_hash), Some(expires)) = (
            client.client_secret_hash_previous.as_ref(),
            client.previous_expires_at,
        ) && expires > Utc::now()
            && verify_password(plaintext, prev_hash).unwrap_or(false)
        {
            return true;
        }
        false
    }

    pub async fn delete(&self, client_id: &RecordId) -> Result<()> {
        DB.query("DELETE $id")
            .bind(("id", client_id.clone()))
            .await?;
        Ok(())
    }

    pub async fn update_redirect_uris(
        &self,
        client_id: &RecordId,
        uris: Vec<String>,
    ) -> Result<()> {
        DB.query("UPDATE $id SET redirect_uris = $uris, updated_at = time::now()")
            .bind(("id", client_id.clone()))
            .bind(("uris", uris))
            .await?;
        Ok(())
    }

    pub async fn update_post_logout_uris(
        &self,
        client_id: &RecordId,
        uris: Vec<String>,
    ) -> Result<()> {
        DB.query("UPDATE $id SET post_logout_redirect_uris = $uris, updated_at = time::now()")
            .bind(("id", client_id.clone()))
            .bind(("uris", uris))
            .await?;
        Ok(())
    }

    pub async fn update_allowed_scopes(
        &self,
        client_id: &RecordId,
        scopes: Vec<String>,
    ) -> Result<()> {
        DB.query("UPDATE $id SET allowed_scopes = $scopes, updated_at = time::now()")
            .bind(("id", client_id.clone()))
            .bind(("scopes", scopes))
            .await?;
        Ok(())
    }

    pub async fn update_ssf(
        &self,
        client_id: &RecordId,
        endpoint: Option<String>,
        delivery_method: String,
        events: Vec<String>,
    ) -> Result<()> {
        DB.query(
            "UPDATE $id SET
                ssf_receiver_endpoint = $ep,
                ssf_delivery_method = $dm,
                ssf_events_subscribed = $events,
                updated_at = time::now()",
        )
        .bind(("id", client_id.clone()))
        .bind(("ep", endpoint))
        .bind(("dm", delivery_method))
        .bind(("events", events))
        .await?;
        Ok(())
    }
}

impl Default for OauthClientModel {
    fn default() -> Self {
        Self::new()
    }
}
