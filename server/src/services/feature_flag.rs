//! Feature flag service.
//!
//! Flags are *registered in code* and *configured in the database*. Adding
//! a new flag means:
//!   1. Append it to `FLAG_REGISTRY` below.
//!   2. On the next server boot, `register_flags()` seeds a row in the DB
//!      with `state = 'off'`.
//!   3. An admin can flip the state from /admin/feature-flags.
//!
//! At runtime, code asks `allows(key, user)` to know whether to expose a
//! feature. The four states map to:
//!   - Off — no one sees it
//!   - AdminOnly — only `is_admin = true` users
//!   - Verified — only `verification_status = 'identity'` users
//!   - All — everyone (including unauthenticated, where the route permits it)

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{debug, error, info};

use crate::db::DB;
use crate::error::Error;
use crate::models::person::{Person, SessionUser};

// ---------------------------------------------------------------------------
// Registry — the source of truth for which flags exist.
// ---------------------------------------------------------------------------

pub struct FlagDef {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub const FLAG_REGISTRY: &[FlagDef] = &[FlagDef {
    key: "identity_verification",
    name: "Paid Identity Verification",
    description: "Shows the paid Stripe Identity flow on /get-verified. When off, only manual verification requests are accepted.",
}];

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagState {
    Off,
    AdminOnly,
    Verified,
    All,
}

impl FlagState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::AdminOnly => "admin_only",
            Self::Verified => "verified",
            Self::All => "all",
        }
    }
}

impl FromStr for FlagState {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "off" => Ok(Self::Off),
            "admin_only" => Ok(Self::AdminOnly),
            "verified" => Ok(Self::Verified),
            "all" => Ok(Self::All),
            _ => Err(()),
        }
    }
}

impl fmt::Display for FlagState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Row + queries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, SurrealValue)]
pub struct FeatureFlagRow {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub state: String,
}

/// Seed any registry flag not yet present in the DB. Existing rows are left
/// alone — operators may have already adjusted the state. Called once at
/// boot from `main.rs`.
pub async fn register_flags() {
    for def in FLAG_REGISTRY {
        let result = DB
            .query(
                "IF (SELECT VALUE id FROM feature_flag WHERE key = $key LIMIT 1)[0] IS NONE THEN \
                     CREATE feature_flag SET key = $key, name = $name, description = $desc, state = 'off' \
                 ELSE \
                     UPDATE feature_flag SET name = $name, description = $desc WHERE key = $key \
                 END",
            )
            .bind(("key", def.key.to_string()))
            .bind(("name", def.name.to_string()))
            .bind(("desc", def.description.to_string()))
            .await;
        if let Err(e) = result {
            error!(flag = def.key, error = %e, "feature_flag: failed to register");
        } else {
            debug!(flag = def.key, "feature_flag: registered");
        }
    }
    info!(count = FLAG_REGISTRY.len(), "feature flags registered");
}

/// Read all flags (used by the admin page). Returns rows in registry order.
pub async fn list_flags() -> Vec<FeatureFlagRow> {
    let rows: Vec<FeatureFlagRow> = match DB
        .query("SELECT key, name, description, state FROM feature_flag")
        .await
    {
        Ok(mut r) => r.take(0).unwrap_or_default(),
        Err(e) => {
            error!(error = %e, "feature_flag: list query failed");
            return Vec::new();
        }
    };
    // Sort by registry order so the admin page is stable.
    let mut by_key: std::collections::HashMap<String, FeatureFlagRow> =
        rows.into_iter().map(|r| (r.key.clone(), r)).collect();
    FLAG_REGISTRY
        .iter()
        .filter_map(|def| by_key.remove(def.key))
        .collect()
}

/// Get a single flag's state. Returns `Off` if the flag isn't in the DB
/// (e.g. very-first boot before registration completes).
pub async fn get_state(key: &str) -> FlagState {
    let row: Option<FeatureFlagRow> = match DB
        .query("SELECT key, name, description, state FROM feature_flag WHERE key = $key LIMIT 1")
        .bind(("key", key.to_string()))
        .await
    {
        Ok(mut r) => r.take(0).ok().flatten(),
        Err(e) => {
            error!(flag = key, error = %e, "feature_flag: get_state query failed");
            None
        }
    };
    row.and_then(|r| FlagState::from_str(&r.state).ok())
        .unwrap_or(FlagState::Off)
}

/// Mutate state. `updated_by` is set on the row so admins can see who
/// changed what.
pub async fn set_state(
    key: &str,
    new_state: FlagState,
    updated_by: Option<RecordId>,
) -> Result<(), Error> {
    if FLAG_REGISTRY.iter().all(|f| f.key != key) {
        return Err(Error::BadRequest(format!("unknown feature flag: {key}")));
    }
    DB.query("UPDATE feature_flag SET state = $state, updated_by = $by WHERE key = $key")
        .bind(("key", key.to_string()))
        .bind(("state", new_state.as_str().to_string()))
        .bind(("by", updated_by))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    info!(flag = key, state = %new_state, "feature_flag: state changed");
    Ok(())
}

// ---------------------------------------------------------------------------
// Access check
// ---------------------------------------------------------------------------

/// Decide whether a feature is currently visible to the given session user.
/// `None` for `user` means an unauthenticated visitor.
///
/// Hits the DB once per call — cheap, and flags read on a serious hot path
/// can be cached at the call site if it ever matters.
pub async fn allows(flag_key: &str, user: Option<&SessionUser>) -> bool {
    match get_state(flag_key).await {
        FlagState::Off => false,
        FlagState::All => true,
        FlagState::AdminOnly => match user {
            Some(u) => user_is_admin(u).await,
            None => false,
        },
        FlagState::Verified => match user {
            Some(u) => user_is_identity_verified(u).await,
            None => false,
        },
    }
}

async fn user_is_admin(user: &SessionUser) -> bool {
    // SessionUser doesn't carry is_admin — load from the person row.
    match Person::find_by_id(&user.id).await {
        Ok(Some(_p)) => {
            // is_admin lives on the person record but isn't on the Person
            // struct directly. Query it explicitly.
            #[derive(Deserialize, SurrealValue)]
            struct Row {
                is_admin: Option<bool>,
            }
            let pid = match RecordId::parse_simple(&user.id) {
                Ok(r) => r,
                Err(_) => return false,
            };
            let mut response = match DB
                .query("SELECT is_admin FROM $pid")
                .bind(("pid", pid))
                .await
            {
                Ok(r) => r,
                Err(_) => return false,
            };
            let row: Option<Row> = response.take(0).ok().flatten();
            row.and_then(|r| r.is_admin).unwrap_or(false)
        }
        _ => false,
    }
}

async fn user_is_identity_verified(user: &SessionUser) -> bool {
    match Person::find_by_id(&user.id).await {
        Ok(Some(p)) => p.verification_status == "identity",
        _ => false,
    }
}
