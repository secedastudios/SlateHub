//! Listmonk mailing-list integration.
//!
//! The app's source of truth for users is the SurrealDB `person` table.
//! `ListmonkService` is a best-effort side-effect fan-out to a self-hosted
//! Listmonk instance. Sink failures are logged but never propagated to the
//! caller — Listmonk outages must not block signup.
//!
//! Required env vars (when any are missing, calls are no-ops):
//!   LISTMONK_URL          base URL, e.g. https://news.example.com
//!   LISTMONK_USER         basic-auth username (Listmonk API user)
//!   LISTMONK_API_KEY      basic-auth password / API token
//!   LISTMONK_LIST_IDS     comma-separated numeric list ids, e.g. "1,3"

use std::time::Duration;

use serde::Serialize;
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct ListmonkService {
    http: reqwest::Client,
    /// Base URL with no trailing slash, e.g. `https://news.example.com`.
    base: String,
    user: String,
    token: String,
    list_ids: Vec<i64>,
}

impl ListmonkService {
    /// Build from env. Returns `None` (and logs) when required vars are
    /// missing or empty so callers can no-op gracefully.
    pub fn from_env() -> Option<Self> {
        let base = std::env::var("LISTMONK_URL").ok()?;
        let user = std::env::var("LISTMONK_USER").ok()?;
        let token = std::env::var("LISTMONK_API_KEY").ok()?;
        let list_raw = std::env::var("LISTMONK_LIST_IDS").ok()?;

        if base.is_empty() || user.is_empty() || token.is_empty() || list_raw.is_empty() {
            return None;
        }

        // reqwest needs an absolute URL with a scheme. Catch this at config
        // time rather than letting every subscribe call fail with the
        // opaque "builder error: relative URL without a base".
        let base = base.trim_end_matches('/').to_string();
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            warn!(
                base = %base,
                "listmonk: LISTMONK_URL must include a scheme (http:// or https://); sink disabled"
            );
            return None;
        }

        let list_ids: Vec<i64> = list_raw
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<i64>().ok())
            .collect();
        if list_ids.is_empty() {
            warn!(
                raw = %list_raw,
                "listmonk: LISTMONK_LIST_IDS contained no parseable numeric ids; sink disabled"
            );
            return None;
        }

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .ok()?;

        Some(Self {
            http,
            base,
            user,
            token,
            list_ids,
        })
    }

    pub fn list_ids(&self) -> &[i64] {
        &self.list_ids
    }

    /// Subscribe one address to the configured lists. Fire-and-forget: always
    /// returns `Ok(true)` on success / dup, `Ok(false)` on a non-fatal sink
    /// failure (so callers can count successes for admin UI feedback).
    pub async fn subscribe(&self, name: &str, email: &str) -> bool {
        let trimmed = name.trim();
        let display_name = if trimmed.is_empty() {
            email
                .split('@')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("Subscriber")
                .to_string()
        } else {
            trimmed.to_string()
        };

        let payload = SubscribePayload {
            email,
            name: &display_name,
            status: "enabled",
            lists: &self.list_ids,
            preconfirm_subscriptions: true,
        };

        let url = format!("{}/api/subscribers", self.base);
        let resp = self
            .http
            .post(&url)
            .basic_auth(&self.user, Some(&self.token))
            .json(&payload)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status();
                if status.is_success() {
                    debug!(email = %email, "listmonk: subscribed");
                    return true;
                }
                let body = r.text().await.unwrap_or_default();
                let looks_dup = body.contains("already exists")
                    || body.contains("already subscribed")
                    || status.as_u16() == 409;
                if looks_dup {
                    debug!(email = %email, "listmonk: already subscribed");
                    true
                } else {
                    warn!(
                        email = %email,
                        status = %status,
                        body = %body,
                        "listmonk subscribe failed"
                    );
                    false
                }
            }
            Err(e) => {
                warn!(
                    email = %email,
                    url = %url,
                    error = %e,
                    "listmonk transport failed"
                );
                false
            }
        }
    }
}

/// Convenience: subscribe in a spawned task. Used at signup time so the
/// caller is never blocked by Listmonk latency.
pub fn spawn_subscribe(name: String, email: String) {
    let Some(svc) = ListmonkService::from_env() else {
        debug!(email = %email, "listmonk: env incomplete, skipping subscribe");
        return;
    };
    tokio::spawn(async move {
        let _ = svc.subscribe(&name, &email).await;
    });
}

/// Log once at boot so operators can confirm whether Listmonk is wired up.
pub fn log_status() {
    match ListmonkService::from_env() {
        Some(s) => info!(list_ids = ?s.list_ids, "listmonk: configured"),
        None => info!("listmonk: env incomplete, signups will not be forwarded"),
    }
}

#[derive(Serialize)]
struct SubscribePayload<'a> {
    email: &'a str,
    name: &'a str,
    status: &'a str,
    lists: &'a [i64],
    preconfirm_subscriptions: bool,
}
