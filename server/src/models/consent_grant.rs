//! Consent-grant model. A `consent_grant` is a graph edge:
//! `RELATE person -> consent_grant -> oauth_client` recording the scopes the
//! user has approved for that client. New scopes prompt the consent screen
//! again; previously-granted scopes are skipped.

use crate::db::DB;
use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Clone, SurrealValue, Serialize, Deserialize)]
pub struct ConsentGrant {
    pub id: RecordId,
    #[serde(rename = "in")]
    pub person: RecordId,
    pub out: RecordId,
    pub scopes: Vec<String>,
    pub granted_at: DateTime<Utc>,
    #[serde(default)]
    #[surreal(default)]
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn get_for(person: &RecordId, client: &RecordId) -> Result<Option<ConsentGrant>> {
    let mut resp = DB
        .query(
            "SELECT * FROM consent_grant \
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
        DB.query("UPDATE $id SET scopes = $scopes")
            .bind(("id", existing.id))
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
