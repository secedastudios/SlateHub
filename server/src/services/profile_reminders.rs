//! Profile-completion reminder sweep.
//!
//! Verified accounts (email / sms / identity) that never built a profile get up
//! to three reminders — playful, then pointed, then a final notice — and, if
//! still empty after a grace window, are removed so the directory stays a real
//! listing of working talent and crew. Out of scope: unverified accounts (the
//! separate [`crate::models::person::Person::cleanup_unverified`] handles
//! those). Never deleted: anyone who has paid for or requested verification —
//! they're only reminded, mirroring `cleanup_unverified`'s protection.
//!
//! Cadence and the deletion gate come from the environment
//! ([`ReminderConfig::from_env`]). The whole sweep no-ops when no email provider
//! is configured, since we must never delete people we couldn't first remind.

use std::collections::HashSet;
use std::env;

use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{debug, info, warn};

use crate::db::DB;
use crate::models::person::Person;
use crate::record_id_ext::RecordIdExt;
use crate::services::email::EmailService;

/// Cadence and safety configuration for the reminder sweep.
#[derive(Debug, Clone)]
pub struct ReminderConfig {
    /// Account age (in days) before the first reminder goes out.
    pub initial_days: u32,
    /// Minimum days between consecutive reminders.
    pub gap_days: u32,
    /// Days after the final reminder before the account is removed.
    pub grace_days: u32,
    /// Max accounts touched per stage per run, so the first sweep drains a
    /// months-old backlog of empty accounts gradually instead of blasting cold
    /// email to thousands of dormant addresses at once (which would wreck
    /// sending reputation, including for verification codes).
    pub max_per_run: u32,
    /// Master switch for the destructive deletion step. Off by default so
    /// reminders can run for a while before anything is actually removed.
    pub delete_enabled: bool,
}

impl ReminderConfig {
    /// Read from the environment. Defaults: first reminder at 3 days, 4 days
    /// between reminders, 7-day grace, 250 accounts/stage/run, deletion disabled.
    pub fn from_env() -> Self {
        fn num(var: &str, default: u32) -> u32 {
            env::var(var)
                .ok()
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(default)
        }
        Self {
            initial_days: num("PROFILE_REMINDER_INITIAL_DAYS", 3),
            gap_days: num("PROFILE_REMINDER_GAP_DAYS", 4),
            grace_days: num("PROFILE_REMINDER_GRACE_DAYS", 7),
            max_per_run: num("PROFILE_REMINDER_MAX_PER_RUN", 250),
            delete_enabled: env::var("PROFILE_REMINDER_DELETE_ENABLED")
                .map(|v| v.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        }
    }
}

/// SurrealQL `WHERE` fragment: a verified account whose profile is empty (no
/// avatar and no headline — i.e. not discoverable in the directory).
const VERIFIED_EMPTY: &str = "verification_status != 'unverified' \
    AND (profile.avatar IS NONE OR profile.avatar = '') \
    AND (profile.headline IS NONE OR profile.headline = '')";

#[derive(Debug, Deserialize, SurrealValue)]
struct Candidate {
    id: RecordId,
    name: Option<String>,
    email: String,
}

/// Run one pass: send any due reminders, then (if enabled) remove accounts that
/// ignored the final notice. No-ops when no email provider is configured.
pub async fn run(cfg: &ReminderConfig) {
    let email = match EmailService::from_env() {
        Ok(e) => e,
        Err(e) => {
            debug!("profile_reminders: no email provider configured, skipping ({e})");
            return;
        }
    };
    let edit_url = format!(
        "{}/profile/edit",
        crate::config::app_url().trim_end_matches('/')
    );

    for n in 1u8..=3 {
        send_due_reminders(&email, n, &edit_url, cfg).await;
    }
    delete_expired(cfg).await;
}

/// Send reminder `n` (1–3) to every account due for it, then bump its counter
/// and timestamp so the next reminder is correctly spaced.
async fn send_due_reminders(email: &EmailService, n: u8, edit_url: &str, cfg: &ReminderConfig) {
    // Reminder 1 is gated on account age; later reminders on time since the
    // previous one (so they stay spaced out, even when backfilling old accounts).
    let time_cond = if n == 1 {
        format!("created_at < time::now() - {}d", cfg.initial_days)
    } else {
        format!("last_profile_reminder_at < time::now() - {}d", cfg.gap_days)
    };
    // `DEFAULT 0` only applies on write, so accounts that predate the migration
    // have NULL here. Treat NULL as 0 so reminder 1 still reaches them.
    let count_cond = if n == 1 {
        "(profile_reminders_sent IS NONE OR profile_reminders_sent = 0)".to_string()
    } else {
        format!("profile_reminders_sent = {}", n - 1)
    };
    let sql = format!(
        "SELECT id, name, email FROM person \
         WHERE {VERIFIED_EMPTY} AND {count_cond} AND {time_cond} \
         LIMIT {limit}",
        limit = cfg.max_per_run,
    );

    let candidates: Vec<Candidate> = match DB.query(&sql).await {
        Ok(mut r) => r.take(0).unwrap_or_default(),
        Err(e) => {
            warn!(reminder = n, error = %e, "profile_reminders: candidate query failed");
            return;
        }
    };
    if candidates.is_empty() {
        return;
    }

    let mut sent = 0usize;
    for c in &candidates {
        match email
            .send_profile_reminder(&c.email, c.name.as_deref(), n, edit_url, cfg.grace_days)
            .await
        {
            Ok(()) => {
                if let Err(e) = DB
                    .query("UPDATE $id SET profile_reminders_sent = $n, last_profile_reminder_at = time::now()")
                    .bind(("id", c.id.clone()))
                    .bind(("n", i64::from(n)))
                    .await
                {
                    warn!(person = %c.id.to_raw_string(), error = %e, "profile_reminders: failed to record reminder");
                } else {
                    sent += 1;
                }
            }
            Err(e) => warn!(email = %c.email, error = %e, "profile_reminders: send failed"),
        }
    }
    info!(reminder = n, sent, "profile_reminders: reminders sent");
}

/// Remove accounts that received all three reminders and stayed empty past the
/// grace window — skipping any protected (paid / verification-requested) user.
async fn delete_expired(cfg: &ReminderConfig) {
    if !cfg.delete_enabled {
        debug!("profile_reminders: deletion disabled (PROFILE_REMINDER_DELETE_ENABLED), skipping");
        return;
    }

    let sql = format!(
        "SELECT VALUE id FROM person \
         WHERE {VERIFIED_EMPTY} AND profile_reminders_sent >= 3 \
           AND last_profile_reminder_at < time::now() - {grace}d \
         LIMIT {limit}",
        grace = cfg.grace_days,
        limit = cfg.max_per_run,
    );
    let candidates: Vec<RecordId> = match DB.query(&sql).await {
        Ok(mut r) => r.take(0).unwrap_or_default(),
        Err(e) => {
            warn!(error = %e, "profile_reminders: delete query failed");
            return;
        }
    };
    if candidates.is_empty() {
        return;
    }

    // Never delete anyone who has paid for or requested verification — same
    // protection as Person::cleanup_unverified.
    let paid: Vec<RecordId> = DB
        .query("SELECT VALUE person FROM verification_payment")
        .await
        .ok()
        .and_then(|mut r| r.take(0).ok())
        .unwrap_or_default();
    let requested: Vec<RecordId> = DB
        .query("SELECT VALUE person FROM verification_request")
        .await
        .ok()
        .and_then(|mut r| r.take(0).ok())
        .unwrap_or_default();
    let protected: HashSet<String> = paid
        .into_iter()
        .chain(requested)
        .map(|r| r.to_raw_string())
        .collect();

    let to_delete: Vec<RecordId> = candidates
        .into_iter()
        .filter(|id| !protected.contains(&id.to_raw_string()))
        .collect();
    if to_delete.is_empty() {
        debug!("profile_reminders: all deletion candidates are payment-protected");
        return;
    }

    let (mut ok, mut failed) = (0usize, 0usize);
    for id in &to_delete {
        match Person::delete_with_cascade(id).await {
            Ok(()) => ok += 1,
            Err(e) => {
                failed += 1;
                warn!(person = %id.to_raw_string(), error = %e, "profile_reminders: cascade delete failed");
            }
        }
    }
    info!(
        ok,
        failed, "profile_reminders: removed empty accounts past grace"
    );
}
