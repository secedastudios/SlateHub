//! Boot-time schema migrations.
//!
//! Every `.surql` file under `server/migrations/` is compiled into the binary
//! (see [`MIGRATIONS`]) and applied exactly once, in file-name order, when the
//! server starts — before any service touches the schema. Applied files are
//! recorded in the `schema_migration` table, keyed by file name, so a restart
//! or a redeploy only runs what is new. `db/schema.surql` is the squash
//! baseline for fresh installs (migrations 001–014 were folded into it on
//! 2026-10-18 and exist only in git history); everything above [`BASELINE`]
//! is applied from here.
//!
//! The data-safety contract (spelled out in `server/migrations/README.md`):
//!
//! * migrations are additive and idempotent, so a fresh install that already
//!   has the full schema can run them all without harm;
//! * anything that could remove data (`REMOVE TABLE|FIELD|DATABASE|NAMESPACE`,
//!   `DELETE`, `UNSET`) is refused unless the file carries an explicit
//!   authorization line ([`check_destructive`]) — nothing destructive runs by
//!   accident;
//! * each file runs in one transaction together with its ledger row, and a
//!   failure aborts boot rather than serving traffic on a half-migrated
//!   schema;
//! * a database the runner has never touched (empty ledger) must already be
//!   at the baseline — a restored pre-squash backup, or an empty database,
//!   is refused with a message naming what is missing ([`BASELINE_PROBES`]),
//!   since the embedded migrations cannot repair either.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{debug, info, warn};

use crate::db::DB;

/// Highest migration number folded into `db/schema.surql`. Files numbered at
/// or below this are part of the baseline and must not be re-added.
pub const BASELINE: u32 = 14;

/// A migration file embedded in the binary.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// File stem, e.g. `015_profile_public_email`. Doubles as the ledger key.
    pub name: &'static str,
    /// The file's SurrealQL, verbatim.
    pub sql: &'static str,
}

/// Every migration shipped with this build, in application order. Add new
/// files here; `tests/migrations_test.rs` fails if this list and the
/// `server/migrations/` directory disagree.
pub const MIGRATIONS: &[Migration] = &[Migration {
    name: "015_profile_public_email",
    sql: include_str!("../../migrations/015_profile_public_email.surql"),
}];

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// The embedded list itself is malformed (bad name, out of order, …).
    #[error("migration list invalid: {0}")]
    Invalid(String),
    /// The file contains a data-removing statement and no authorization line.
    #[error("migration {name} is destructive and not authorized: {detail}")]
    Unauthorized { name: String, detail: String },
    /// The ledger table could not be created or read, or `INFO FOR …` failed.
    #[error("migration ledger error: {0}")]
    Ledger(#[source] Box<surrealdb::Error>),
    /// First contact with a database that is empty or predates the squash
    /// baseline; the embedded migrations alone cannot bring it up to date.
    #[error("{0}")]
    BelowBaseline(String),
    /// A migration's SurrealQL failed; its transaction was rolled back.
    #[error("migration {name} failed (rolled back): {source}")]
    Failed {
        name: String,
        #[source]
        source: Box<surrealdb::Error>,
    },
}

/// What one [`run`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Applied during this run, in order.
    pub applied: Vec<String>,
    /// Already in the ledger with a matching checksum.
    pub already_applied: Vec<String>,
    /// In the ledger, but the embedded file no longer matches what ran.
    pub drifted: Vec<String>,
}

/// Apply every pending migration in [`MIGRATIONS`]. Called once from `main`
/// right after the database connection is established.
pub async fn run_pending() -> Result<Report, MigrationError> {
    run(MIGRATIONS).await
}

/// Apply every pending migration in `migrations`. The whole list is validated
/// first, so a malformed or unauthorized file means nothing runs at all.
pub async fn run(migrations: &[Migration]) -> Result<Report, MigrationError> {
    validate(migrations)?;
    for m in migrations {
        check_destructive(m)?;
    }

    // Read the ledger before creating it, so an empty database is still
    // recognisable as empty by the baseline check.
    let tables = info_keys("INFO FOR DB", "tables").await?;
    let ledger = if tables.contains("schema_migration") {
        applied_checksums().await?
    } else {
        HashMap::new()
    };
    if ledger.is_empty() {
        check_baseline(&tables).await?;
    }
    ensure_ledger().await?;

    let mut report = Report::default();
    for m in migrations {
        let sum = checksum(m.sql);
        match ledger.get(m.name) {
            Some(recorded) if *recorded == sum => {
                debug!(migration = m.name, "schema migration already applied");
                report.already_applied.push(m.name.to_string());
            }
            Some(_) => {
                warn!(
                    migration = m.name,
                    "schema migration file changed after it was applied — NOT re-running it; \
                     write a new migration instead"
                );
                report.drifted.push(m.name.to_string());
            }
            None => {
                info!(migration = m.name, "applying schema migration");
                apply(m, &sum).await?;
                report.applied.push(m.name.to_string());
            }
        }
    }
    Ok(report)
}

static NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{3})_[a-z0-9_]+$").expect("valid regex"));
static TRANSACTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(BEGIN|COMMIT|CANCEL)\b(\s+TRANSACTION)?\s*;").expect("valid regex")
});
static DESTRUCTIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(REMOVE\s+(TABLE|FIELD|DATABASE|NAMESPACE)|DELETE|UNSET)\b")
        .expect("valid regex")
});
static AUTHORIZED_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Horizontal whitespace only after "by": the name must be on the same line.
    Regex::new(r"(?im)^\s*--\s*destructive:[ \t]*authorized[ \t]+by[ \t]+\S").expect("valid regex")
});

/// Structural checks on a migration list: names are `NNN_snake_case`,
/// numbered above [`BASELINE`], strictly ascending (so also unique), and each
/// file has at least one statement and does not manage its own transaction.
pub fn validate(migrations: &[Migration]) -> Result<(), MigrationError> {
    let mut last: Option<u32> = None;
    for m in migrations {
        let invalid = |why: &str| MigrationError::Invalid(format!("`{}`: {why}", m.name));

        let caps = NAME_RE
            .captures(m.name)
            .ok_or_else(|| invalid("name must look like 015_snake_case_description"))?;
        let number: u32 = caps[1].parse().expect("three digits");
        if number <= BASELINE {
            return Err(invalid(&format!(
                "numbers up to {BASELINE:03} are squashed into db/schema.surql"
            )));
        }
        if last.is_some_and(|prev| number <= prev) {
            return Err(invalid(
                "migrations must be listed in strictly ascending order",
            ));
        }
        last = Some(number);

        let code = strip_comments(m.sql);
        if code.trim().is_empty() {
            return Err(invalid("file contains no statements"));
        }
        if TRANSACTION_RE.is_match(&code) {
            return Err(invalid(
                "do not BEGIN/COMMIT/CANCEL transactions — the runner wraps each file in one",
            ));
        }
    }
    Ok(())
}

/// Refuse data-removing statements unless the file carries an explicit
/// `-- destructive: authorized by <name>, <date> — <why>` line.
pub fn check_destructive(m: &Migration) -> Result<(), MigrationError> {
    let code = strip_comments(m.sql);
    if let Some(found) = DESTRUCTIVE_RE.find(&code)
        && !AUTHORIZED_RE.is_match(m.sql)
    {
        return Err(MigrationError::Unauthorized {
            name: m.name.to_string(),
            detail: format!(
                "contains `{}` but no `-- destructive: authorized by …` line",
                found.as_str()
            ),
        });
    }
    Ok(())
}

/// SHA-256 of the file with line endings normalized, hex-encoded. Stored in
/// the ledger so a file edited after it ran can be detected.
pub fn checksum(sql: &str) -> String {
    let normalized = sql.replace("\r\n", "\n");
    hex::encode(Sha256::digest(normalized.as_bytes()))
}

/// Drop `-- …` line comments so prose never trips the statement scans.
fn strip_comments(sql: &str) -> String {
    sql.lines()
        .map(|line| line.split("--").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Definitions `db/schema.surql` has carried since the squash baseline, one
/// per pre-squash migration era. Checked on a database whose ledger is still
/// empty — its first contact with this runner — to tell "at the baseline,
/// just never run by the runner" apart from "older than the baseline" (a
/// restored pre-squash backup) or "empty" (fresh install without `db-init`).
pub const BASELINE_PROBES: &[(&str, Option<&str>)] = &[
    ("oauth_client", None),                     // 008 OIDC provider
    ("verification_payment", None),             // 010 verification payments
    ("feature_flag", None),                     // 010 feature flags
    ("season", None),                           // 012 productions management
    ("landing_event", None),                    // 013 landing analytics
    ("person", Some("signup_campaign")),        // 013
    ("person", Some("profile_reminders_sent")), // 014 profile reminders
];

/// Refuse to run on a database that is empty or older than the baseline.
/// `tables` is the database's table set as read before the ledger was created.
async fn check_baseline(tables: &HashSet<String>) -> Result<(), MigrationError> {
    if tables.is_empty() {
        return Err(MigrationError::BelowBaseline(
            "the database is empty — import db/schema.surql first (`make db-init`)".to_string(),
        ));
    }
    let mut missing = Vec::new();
    for (table, field) in BASELINE_PROBES {
        if !tables.contains(*table) {
            missing.push(table.to_string());
            continue;
        }
        if let Some(field) = field {
            let fields = info_keys(&format!("INFO FOR TABLE {table}"), "fields").await?;
            if !fields.contains(*field) {
                missing.push(format!("{table}.{field}"));
            }
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    Err(MigrationError::BelowBaseline(format!(
        "the schema is older than the {BASELINE:03} migration baseline (squashed into \
         db/schema.surql on 2026-10-18) — missing: {}. Bring the database to the baseline \
         first (docs/PRODUCTION_DEPLOYMENT.md → \"Restoring an older backup\"), then restart",
        missing.join(", ")
    )))
}

/// Keys of one map in an `INFO FOR …` result (the `tables` of a database,
/// the `fields` of a table). `statement` is always a literal built from
/// [`BASELINE_PROBES`], never user input.
async fn info_keys(statement: &str, map: &str) -> Result<HashSet<String>, MigrationError> {
    let info: Option<serde_json::Value> = DB
        .query(statement)
        .await
        .map_err(ledger_err)?
        .take(0)
        .map_err(ledger_err)?;
    Ok(info
        .as_ref()
        .and_then(|v| v.get(map)?.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default())
}

/// Create the ledger table if this database predates it (idempotent; fresh
/// installs already have it from `db/schema.surql`).
async fn ensure_ledger() -> Result<(), MigrationError> {
    DB.query(
        "DEFINE TABLE IF NOT EXISTS schema_migration TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
         DEFINE FIELD IF NOT EXISTS name ON schema_migration TYPE string PERMISSIONS FULL;
         DEFINE FIELD IF NOT EXISTS checksum ON schema_migration TYPE string PERMISSIONS FULL;
         DEFINE FIELD IF NOT EXISTS applied_at ON schema_migration TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
         DEFINE INDEX IF NOT EXISTS idx_schema_migration_name ON schema_migration FIELDS name UNIQUE;",
    )
    .await
    .map_err(ledger_err)?
    .check()
    .map_err(ledger_err)?;
    Ok(())
}

fn ledger_err(e: surrealdb::Error) -> MigrationError {
    MigrationError::Ledger(Box::new(e))
}

#[derive(Debug, Deserialize, SurrealValue)]
struct LedgerRow {
    name: String,
    checksum: String,
}

async fn applied_checksums() -> Result<HashMap<String, String>, MigrationError> {
    let rows: Vec<LedgerRow> = DB
        .query("SELECT name, checksum FROM schema_migration")
        .await
        .map_err(ledger_err)?
        .take(0)
        .map_err(ledger_err)?;
    Ok(rows.into_iter().map(|r| (r.name, r.checksum)).collect())
}

/// Run one file and insert its ledger row in a single transaction, so the
/// ledger can never claim a migration that did not fully apply.
async fn apply(m: &Migration, sum: &str) -> Result<(), MigrationError> {
    let mut body = m.sql.trim().to_string();
    if !body.ends_with(';') {
        body.push(';');
    }
    let sql = format!(
        "BEGIN TRANSACTION;\n{body}\n\
         CREATE $mig_id CONTENT {{ name: $mig_name, checksum: $mig_checksum }};\n\
         COMMIT TRANSACTION;"
    );
    let name = m.name;
    let failed = move |source| MigrationError::Failed {
        name: name.to_string(),
        source: Box::new(source),
    };
    DB.query(sql)
        .bind(("mig_id", RecordId::new("schema_migration", name)))
        .bind(("mig_name", name.to_string()))
        .bind(("mig_checksum", sum.to_string()))
        .await
        .map_err(failed)?
        .check()
        .map_err(failed)?;
    Ok(())
}
