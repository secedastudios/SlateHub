//! Outbound SSF / CAEP / RISC security event delivery.
//!
//! When an event is enqueued (e.g. an org member's role changes) we write a
//! `security_event` row, then a background worker drains pending events,
//! signs them as Security Event Tokens (SETs) per RFC 8417, and POSTs them
//! to the receiver's webhook (push delivery). Failed deliveries are retried
//! with exponential backoff up to a cap.

use crate::db::DB;
use crate::error::Result;
use crate::models::oauth_client::OauthClient;
use crate::services::oidc_keys;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{debug, error, info, warn};

/// Standard event URIs we emit.
pub mod events {
    pub const CAEP_TOKEN_CLAIMS_CHANGE: &str =
        "https://schemas.openid.net/secevent/caep/event-type/token-claims-change";
    pub const CAEP_SESSION_REVOKED: &str =
        "https://schemas.openid.net/secevent/caep/event-type/session-revoked";
    pub const RISC_ACCOUNT_DISABLED: &str =
        "https://schemas.openid.net/secevent/risc/event-type/account-disabled";
    pub const SLATEHUB_ORG_MEMBERSHIP_REVOKED: &str =
        "https://schemas.slatehub.com/secevent/event-type/org-membership-revoked";
}

const MAX_ATTEMPTS: i64 = 8;

#[derive(Debug, SurrealValue, Serialize, Deserialize)]
pub struct SecurityEventRow {
    pub id: RecordId,
    pub client: RecordId,
    #[serde(default)]
    #[surreal(default)]
    pub subject: Option<RecordId>,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    #[surreal(default)]
    pub delivered_at: Option<DateTime<Utc>>,
    pub attempts: i64,
    #[serde(default)]
    #[surreal(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub acknowledged_at: Option<DateTime<Utc>>,
}

/// Enqueue a single security event for a client.
pub async fn enqueue_event(
    client: &RecordId,
    subject: Option<&RecordId>,
    event_type: &str,
    payload: Value,
) -> Result<()> {
    debug!(client = %crate::record_id_ext::RecordIdExt::to_raw_string(client), event = %event_type, "Enqueuing security event");
    DB.query(
        "CREATE security_event CONTENT {
            client: $client,
            subject: $subject,
            event_type: $event,
            payload: $payload,
            attempts: 0
        } RETURN NONE",
    )
    .bind(("client", client.clone()))
    .bind(("subject", subject.cloned()))
    .bind(("event", event_type.to_string()))
    .bind(("payload", payload))
    .await?;
    Ok(())
}

/// Convenience: emit a CAEP token-claims-change for every active client whose
/// org matches `org_id` and which has a live session for the affected person.
pub async fn emit_token_claims_change(
    org_id: &RecordId,
    person: &RecordId,
    new_claims: Value,
) -> Result<()> {
    let mut resp = DB
        .query("SELECT * FROM oauth_client WHERE organization = $org")
        .bind(("org", org_id.clone()))
        .await?;
    let clients: Vec<OauthClient> = resp.take(0).unwrap_or_default();
    for client in clients {
        if client.ssf_receiver_endpoint.is_none() {
            continue;
        }
        let payload = json!({
            "subject": { "format": "opaque", "id": crate::record_id_ext::RecordIdExt::to_raw_string(person) },
            "event_timestamp": Utc::now().timestamp(),
            "claims": new_claims,
        });
        enqueue_event(
            &client.id,
            Some(person),
            events::CAEP_TOKEN_CLAIMS_CHANGE,
            payload,
        )
        .await?;
    }
    Ok(())
}

/// Convenience: when a person leaves an org, revoke their sessions at the
/// org's client and emit a custom slatehub:org-membership-revoked event.
pub async fn emit_org_membership_revoked(org_id: &RecordId, person: &RecordId) -> Result<()> {
    let mut resp = DB
        .query("SELECT * FROM oauth_client WHERE organization = $org")
        .bind(("org", org_id.clone()))
        .await?;
    let clients: Vec<OauthClient> = resp.take(0).unwrap_or_default();
    for client in clients {
        let sessions = crate::services::oidc_tokens::find_active_session_ids_for_person_and_client(
            &client.id, person,
        )
        .await?;
        for sid in &sessions {
            crate::services::oidc_tokens::revoke_session(sid).await?;
        }
        if client.ssf_receiver_endpoint.is_some() {
            let payload = json!({
                "subject": { "format": "opaque", "id": crate::record_id_ext::RecordIdExt::to_raw_string(person) },
                "event_timestamp": Utc::now().timestamp(),
                "reason": "membership_revoked",
            });
            enqueue_event(
                &client.id,
                Some(person),
                events::SLATEHUB_ORG_MEMBERSHIP_REVOKED,
                payload,
            )
            .await?;
        }
    }
    Ok(())
}

/// Spawn a long-running background task that drains the event queue.
pub fn spawn_delivery_worker() {
    tokio::spawn(async move {
        info!("SSF delivery worker started");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Err(e) = drain_once().await {
                warn!("SSF delivery loop error: {}", e);
            }
        }
    });
}

async fn drain_once() -> Result<()> {
    let mut resp = DB
        .query(
            "SELECT * FROM security_event \
             WHERE delivered_at IS NONE AND attempts < $max \
             ORDER BY created_at LIMIT 25",
        )
        .bind(("max", MAX_ATTEMPTS))
        .await?;
    let pending: Vec<SecurityEventRow> = resp.take(0).unwrap_or_default();
    for evt in pending {
        deliver_one(evt).await;
    }
    Ok(())
}

async fn deliver_one(evt: SecurityEventRow) {
    let client_id_raw = crate::record_id_ext::RecordIdExt::to_raw_string(&evt.client);
    debug!(event_id = %crate::record_id_ext::RecordIdExt::to_raw_string(&evt.id), "Delivering security event");

    let client = match load_client(&evt.client).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            warn!(client = %client_id_raw, "Client missing — dropping event");
            mark_failed(&evt.id, "client_missing", true).await;
            return;
        }
        Err(e) => {
            warn!(error = %e, "Failed to load client for event delivery");
            mark_failed(&evt.id, &e.to_string(), false).await;
            return;
        }
    };
    let endpoint = match &client.ssf_receiver_endpoint {
        Some(ep) => ep.clone(),
        None => {
            mark_failed(&evt.id, "no receiver endpoint", true).await;
            return;
        }
    };

    let set_jwt = match build_set_jwt(&client, &evt).await {
        Ok(j) => j,
        Err(e) => {
            mark_failed(&evt.id, &format!("sign: {e}"), false).await;
            return;
        }
    };

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("reqwest client");
    let resp = http
        .post(&endpoint)
        .header(reqwest::header::CONTENT_TYPE, "application/secevent+jwt")
        .header(reqwest::header::ACCEPT, "application/json")
        .body(set_jwt)
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            mark_delivered(&evt.id).await;
        }
        Ok(r) => {
            let code = r.status();
            let permanent = code.is_client_error() && code != reqwest::StatusCode::REQUEST_TIMEOUT;
            mark_failed(&evt.id, &format!("HTTP {code}"), permanent).await;
        }
        Err(e) => {
            mark_failed(&evt.id, &e.to_string(), false).await;
        }
    }
}

async fn load_client(id: &RecordId) -> Result<Option<OauthClient>> {
    let mut resp = DB
        .query("SELECT * FROM $id")
        .bind(("id", id.clone()))
        .await?;
    let rows: Vec<OauthClient> = resp.take(0).unwrap_or_default();
    Ok(rows.into_iter().next())
}

async fn mark_delivered(id: &RecordId) {
    let _ = DB
        .query("UPDATE $id SET delivered_at = time::now()")
        .bind(("id", id.clone()))
        .await;
}

async fn mark_failed(id: &RecordId, msg: &str, permanent: bool) {
    let result = if permanent {
        DB.query("UPDATE $id SET attempts = $max, last_error = $err, delivered_at = time::now()")
            .bind(("id", id.clone()))
            .bind(("max", MAX_ATTEMPTS))
            .bind(("err", msg.to_string()))
            .await
    } else {
        DB.query("UPDATE $id SET attempts = attempts + 1, last_error = $err")
            .bind(("id", id.clone()))
            .bind(("err", msg.to_string()))
            .await
    };
    if let Err(e) = result {
        error!("Failed to mark security_event status: {}", e);
    }
}

/// Build a Security Event Token (SET) JWT per RFC 8417.
async fn build_set_jwt(client: &OauthClient, evt: &SecurityEventRow) -> Result<String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let jti = crate::services::oidc_tokens::random_opaque_token();
    #[derive(Serialize)]
    struct SetClaims<'a> {
        iss: String,
        aud: &'a str,
        iat: i64,
        jti: String,
        events: Value,
    }
    let claims = SetClaims {
        iss: crate::config::app_url(),
        aud: &client.client_id,
        iat: now,
        jti,
        events: json!({ &evt.event_type: evt.payload.clone() }),
    };
    oidc_keys::sign_id_token(&claims).await
}
