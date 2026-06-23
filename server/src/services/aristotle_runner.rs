//! Thin slatehub-side wrapper around the inlined `crate::aristotle` module.
//!
//! Provides:
//!   * A single entry point [`run_breakdown`] that takes the script bytes
//!     and returns a structured per-scene breakdown.
//!   * A concurrency cap so we never have more than `MAX_CONCURRENT` heavy
//!     parse+breakdown jobs running at once on the server runtime.
//!   * `spawn_blocking` for the CPU-bound parse step so it doesn't block
//!     the cooperative scheduler.
//!
//! The aristotle module is storage-agnostic; this wrapper translates
//! `crate::aristotle::*` errors into the project [`Error`] type and returns
//! plain data that handler code persists into slatehub's own
//! `scene` / `breakdown_item` tables.

use std::sync::LazyLock;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use crate::error::{Error, Result};

/// Cap on concurrent breakdown jobs. Parse + tier 0–2 breakdown is CPU-bound;
/// without a cap, a flood of upload requests would starve the HTTP runtime.
/// Tunable later via env var if we observe real bottlenecks; 4 is a sane
/// default on a typical 4–8-core box.
const MAX_CONCURRENT: usize = 4;

static SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(MAX_CONCURRENT));

/// Output of a complete parse + breakdown run. Plain data — handler code
/// is responsible for persisting these into slatehub's scene + breakdown_item
/// tables.
#[derive(Debug, Clone)]
pub struct BreakdownOutput {
    /// Title-page metadata extracted by the parser (title, writers, draft
    /// date, contact info, …).
    pub metadata: crate::aristotle::ScriptMetadata,
    /// One breakdown per parsed scene, in script order.
    pub scenes: Vec<crate::aristotle::SceneBreakdown>,
}

/// Parse `bytes` as a screenplay (format inferred from `filename` extension)
/// and produce a per-scene breakdown using aristotle's deterministic
/// tier 0–2 pipeline.
///
/// Concurrency-capped via a global semaphore so we never have more than
/// [`MAX_CONCURRENT`] heavy jobs running. CPU-bound work is dispatched via
/// `spawn_blocking` so the async runtime stays responsive.
///
/// `job_id` is forwarded to aristotle as a correlation string — it shows
/// up in the produced `SceneBreakdown.job_id` field and in trace logs.
///
/// # Errors
///
/// * `Error::ExternalService` — the screenplay failed to parse (unsupported
///   format, corrupt bytes).
/// * `Error::Internal` — the semaphore was closed or the `spawn_blocking`
///   task failed to join (panic/cancellation); both indicate runtime
///   trouble, not bad input.
pub async fn run_breakdown(
    filename: String,
    bytes: Vec<u8>,
    job_id: String,
) -> Result<BreakdownOutput> {
    let permit = SEMAPHORE
        .acquire()
        .await
        .map_err(|e| Error::Internal(format!("semaphore closed: {e}")))?;

    debug!(filename = %filename, job_id = %job_id, "aristotle: starting breakdown");

    let result = tokio::task::spawn_blocking(move || run_blocking(&filename, &bytes, &job_id))
        .await
        .map_err(|e| Error::Internal(format!("aristotle spawn_blocking join error: {e}")))??;

    drop(permit);
    info!(
        scenes = result.scenes.len(),
        "aristotle: breakdown complete"
    );
    Ok(result)
}

/// The blocking inner pipeline. Pure CPU; called via `spawn_blocking`.
fn run_blocking(filename: &str, bytes: &[u8], job_id: &str) -> Result<BreakdownOutput> {
    let mut parsed = crate::aristotle::parse_screenplay(filename, bytes)
        .map_err(|e| Error::ExternalService(format!("aristotle parse: {e}")))?;

    let context = crate::aristotle::BreakdownContext::from_scenes(&parsed.scenes);
    let mut scenes_out = Vec::with_capacity(parsed.scenes.len());

    for scene in parsed.scenes.iter_mut() {
        // Tier 0–2 only by default. Tier 4 (LLM) is gated behind a
        // future env-driven Policy flip; deterministic is the safe default.
        let breakdown = crate::aristotle::run_breakdown(
            job_id,
            scene,
            &context,
            crate::aristotle::BreakdownPolicy::DeterministicOnly,
        );
        scenes_out.push(breakdown);
    }

    if scenes_out.is_empty() {
        warn!(
            filename,
            "aristotle: parse produced zero scenes — likely an empty or malformed script"
        );
    }

    Ok(BreakdownOutput {
        metadata: parsed.metadata,
        scenes: scenes_out,
    })
}
