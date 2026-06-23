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
//!
//! Reads always go to the `feature_flag` table (no in-process cache), and
//! every failure path fails *closed*: an unknown key, a missing row, an
//! unparseable `state` string, or a DB error all evaluate as `Off`.

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

/// Compile-time definition of one feature flag. The registry entry is the
/// flag's identity; the DB row only carries its current state.
pub struct FlagDef {
    /// Stable lookup key passed to [`allows`] / [`get_state`] (snake_case).
    pub key: &'static str,
    /// Human-readable name shown on the admin page.
    pub name: &'static str,
    /// What the flag gates, shown on the admin page.
    pub description: &'static str,
    /// Seed value used by `register_flags()` when this flag has never been
    /// stored before. Existing rows are *not* overwritten — an operator
    /// who flips a flag in the admin UI keeps that change across reboots.
    pub initial_state: FlagState,
}

/// Source of truth for which flags exist. [`set_state`] rejects keys not
/// listed here, and [`list_flags`] returns rows in this order.
pub const FLAG_REGISTRY: &[FlagDef] = &[
    FlagDef {
        key: "identity_verification",
        name: "Paid Identity Verification",
        description: "Shows the paid Stripe Identity flow on /get-verified. When off, only manual verification requests are accepted.",
        initial_state: FlagState::Off,
    },
    FlagDef {
        key: "production_management",
        name: "Production Management",
        description: "Master switch for the production-management workspace (`/productions/{slug}/manage/*`). Required for script versioning UI, breakdown, scheduling, and call sheets to render. Locked to slatehub admins until promoted.",
        initial_state: FlagState::AdminOnly,
    },
    FlagDef {
        key: "script_breakdown",
        name: "Script Breakdown",
        description: "Enables the aristotle-powered automated breakdown action on scripts. Gated separately so the master `production_management` flag can stay on while this subsystem is disabled or vice versa.",
        initial_state: FlagState::AdminOnly,
    },
    FlagDef {
        key: "call_sheet_email",
        name: "Call Sheet Email Delivery",
        description: "Permits the 'Publish & Email' call sheet action to send real emails to recipients. With this off, call sheets can still be generated and downloaded as PDFs but no email is sent.",
        initial_state: FlagState::AdminOnly,
    },
];

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Visibility state of a flag. Stored in the DB as the snake_case strings
/// produced by [`FlagState::as_str`] (`off`, `admin_only`, `verified`,
/// `all`); see [`allows`] for the exact evaluation rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagState {
    /// Feature hidden from everyone. Also the effective state for unknown
    /// or unreadable flags (fail closed).
    Off,
    /// Visible only to signed-in users whose `person` row has
    /// `is_admin = true`.
    AdminOnly,
    /// Visible only to signed-in users whose `person.verification_status`
    /// is `'identity'` (paid identity verification).
    Verified,
    /// Visible to everyone, including unauthenticated visitors where the
    /// route itself permits anonymous access.
    All,
}

impl FlagState {
    /// The snake_case string stored in the `feature_flag.state` column —
    /// the inverse of the `FromStr` impl.
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

/// One row of the `feature_flag` table as read for the admin page and state
/// checks. `state` stays a raw string here (the `SurrealValue` derive can't
/// handle enums); parse it with `FlagState::from_str` when needed.
#[derive(Debug, Clone, Deserialize, SurrealValue)]
pub struct FeatureFlagRow {
    /// Registry key ([`FlagDef::key`]).
    pub key: String,
    /// Display name (kept in sync with the registry on every boot).
    pub name: String,
    /// Display description (kept in sync with the registry on every boot).
    pub description: Option<String>,
    /// Current state as its snake_case string form.
    pub state: String,
}

/// Seed any registry flag not yet present in the DB. Existing rows are left
/// alone — operators may have already adjusted the state. Called once at
/// boot from `main.rs`.
pub async fn register_flags() {
    for def in FLAG_REGISTRY {
        // If the row doesn't exist yet, create it with the registry's
        // declared `initial_state`. If it already exists, update only the
        // name + description (cosmetic metadata) and leave `state` alone —
        // operators who flipped the flag in the admin UI keep their setting.
        let result = DB
            .query(
                "IF (SELECT VALUE id FROM feature_flag WHERE key = $key LIMIT 1)[0] IS NONE THEN \
                     CREATE feature_flag SET key = $key, name = $name, description = $desc, state = $initial \
                 ELSE \
                     UPDATE feature_flag SET name = $name, description = $desc WHERE key = $key \
                 END",
            )
            .bind(("key", def.key.to_string()))
            .bind(("name", def.name.to_string()))
            .bind(("desc", def.description.to_string()))
            .bind(("initial", def.initial_state.as_str().to_string()))
            .await;
        if let Err(e) = result {
            error!(flag = def.key, error = %e, "feature_flag: failed to register");
        } else {
            debug!(flag = def.key, initial = %def.initial_state, "feature_flag: registered");
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
///
/// # Errors
///
/// * `Error::BadRequest` if `key` is not in [`FLAG_REGISTRY`] (unknown
///   flags can't be created through the admin UI).
/// * `Error::Database` if the UPDATE fails.
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
/// Exact semantics per state (state comes from [`get_state`], so unknown
/// flags and DB errors behave as `Off`):
///
/// * `Off` → `false` for everyone.
/// * `All` → `true` for everyone, including `None` (anonymous).
/// * `AdminOnly` → `true` only when `user` is `Some` *and* their `person`
///   row has `is_admin = true` (looked up fresh from the DB; any lookup
///   failure → `false`).
/// * `Verified` → `true` only when `user` is `Some` *and* their `person`
///   row has `verification_status = 'identity'` (email-verified is not
///   enough; lookup failure → `false`).
///
/// Hits the DB once per call (plus a person lookup for the gated states) —
/// cheap, and flags read on a serious hot path can be cached at the call
/// site if it ever matters.
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
