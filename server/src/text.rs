//! Dependency-free text formatting helpers shared across layers.
//!
//! Lives at the crate root (rather than in `templates` or a model) because
//! both data-layer code (slug generation in models) and presentation code
//! (Askama filters, the stats endpoint, request logging) need these, and
//! none of those layers should depend on another just for a string helper.

/// Derive a URL-safe slug from free-form text.
///
/// Lowercases, replaces every non-alphanumeric run with a single `-`, and
/// trims leading/trailing dashes: `"The Last Deposit!"` → `"the-last-deposit"`.
///
/// This is the canonical implementation — `production`, `location`, and the
/// script-upload file-key builder all previously carried byte-identical
/// copies. Uniqueness (e.g. `-2` suffixes on collision) remains the caller's
/// concern; this function is purely lexical.
pub fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Format a byte count as a human-readable label: `1.5 MB`, `820 KB`, `42 B`.
///
/// Binary-prefix scaling (1024) with one decimal for MB/GB, none for KB/B —
/// matching what the admin stats page and request logs have always shown.
pub fn format_bytes(bytes: u64) -> String {
    const GIB: u64 = 1_073_741_824;
    const MIB: u64 = 1_048_576;
    const KIB: u64 = 1_024;
    if bytes >= GIB {
        format!("{:.1} GB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.0} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// [`format_bytes`] for signed sizes (negative clamps to `0 B`).
///
/// Exists because file sizes arrive from SurrealDB as `i64`; the Askama
/// `human_bytes` filter delegates here.
pub fn format_bytes_i64(bytes: i64) -> String {
    format_bytes(u64::try_from(bytes).unwrap_or(0))
}
