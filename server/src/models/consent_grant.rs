//! Consent-grant model. A `consent_grant` is a graph edge:
//! `RELATE person -> consent_grant -> oauth_client` recording the scopes the
//! user has approved for that client. New scopes prompt the consent screen
//! again; previously-granted scopes are skipped.

use crate::db::DB;
use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// View of a consent grant. We deliberately omit the relation's `in`/`out`
/// fields here because the SurrealDB v3 SDK can't deserialize a relation row
/// with `in`/`out` RecordId fields into an arbitrary struct (the underlying
/// `take` then surfaces a "Tried to take only a single result from a query
/// that contains multiple" error). Callers already know the person + client.
#[derive(Debug, Clone, SurrealValue, Serialize, Deserialize)]
pub struct ConsentGrant {
    /// Stringified record id (use `RecordId::parse_simple` if you need the typed form).
    pub id: String,
    pub scopes: Vec<String>,
    pub granted_at: DateTime<Utc>,
    #[serde(default)]
    #[surreal(default)]
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn get_for(person: &RecordId, client: &RecordId) -> Result<Option<ConsentGrant>> {
    let mut resp = DB
        .query(
            "SELECT <string> id AS id, scopes, granted_at, revoked_at \
             FROM consent_grant \
             WHERE in = $person AND out = $client AND revoked_at IS NONE LIMIT 1",
        )
        .bind(("person", person.clone()))
        .bind(("client", client.clone()))
        .await?;
    let rows: Vec<ConsentGrant> = resp.take(0).unwrap_or_default();
    Ok(rows.into_iter().next())
}

/// Upsert a grant: if one exists, merge new scopes into it; otherwise create.
pub async fn upsert_grant(person: &RecordId, client: &RecordId, scopes: &[String]) -> Result<()> {
    if let Some(existing) = get_for(person, client).await? {
        let mut merged = existing.scopes;
        for s in scopes {
            if !merged.contains(s) {
                merged.push(s.clone());
            }
        }
        let id = RecordId::parse_simple(&existing.id)
            .map_err(|e| crate::error::Error::Internal(format!("bad consent_grant id: {e}")))?;
        DB.query("UPDATE $id SET scopes = $scopes")
            .bind(("id", id))
            .bind(("scopes", merged))
            .await?;
    } else {
        DB.query("RELATE $person->consent_grant->$client SET scopes = $scopes")
            .bind(("person", person.clone()))
            .bind(("client", client.clone()))
            .bind(("scopes", scopes.to_vec()))
            .await?;
    }
    Ok(())
}

/// Returns scopes from `requested` that the user has not yet approved.
pub fn scopes_needing_consent(
    existing: &Option<ConsentGrant>,
    requested: &[String],
) -> Vec<String> {
    match existing {
        None => requested.to_vec(),
        Some(g) => requested
            .iter()
            .filter(|s| !g.scopes.contains(s))
            .cloned()
            .collect(),
    }
}
