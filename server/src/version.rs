//! Crate version constant and the asset cache-buster derived from it.
//!
//! Captures the version from `Cargo.toml` at compile time. Surfaced in the
//! health-check JSON (`routes::pages`) and the admin build-info panel
//! (`routes::admin`). Templates use [`asset_version`] for `?v=` query
//! strings on static assets.

use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Application version from `Cargo.toml` (`CARGO_PKG_VERSION`), e.g. `"0.1.0"`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Cache-busting version for static-asset URLs: `"<semver>.<boot-epoch>"`.
///
/// Static files are served with `Cache-Control: immutable, max-age=1y`
/// (see `routes::app()`), so the `?v=` value MUST change whenever CSS/JS
/// changes or browsers keep stale assets forever. The crate version alone
/// isn't enough — assets routinely change without a semver bump — so the
/// process boot time is appended: every deploy/restart invalidates.
pub fn asset_version() -> &'static str {
    static ASSET_VERSION: LazyLock<String> = LazyLock::new(|| {
        let boot_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("{VERSION}.{boot_epoch}")
    });
    &ASSET_VERSION
}
