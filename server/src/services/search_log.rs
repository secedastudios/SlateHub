//! Fire-and-forget search query logging into the `search_log` table.
//!
//! Called by the web search routes and the MCP search tools after results
//! are computed. The write runs in a `tokio::spawn`ed task on the global
//! [`crate::db::DB`] connection; failures are logged at `warn` and dropped.
//! No init or env vars.

use tracing::warn;

use crate::db::DB;

/// Fire-and-forget search log entry. Spawns a background task so it never blocks search results.
///
/// `source` identifies the entry point (e.g. `"web"`, `"mcp"`), `category`
/// the search vertical (`"people"`, `"productions"`, …), and
/// `result_count` how many hits were returned (when known).
pub fn log_search(query: &str, source: &str, category: &str, result_count: Option<usize>) {
    let query = query.to_string();
    let source = source.to_string();
    let category = category.to_string();
    let result_count = result_count.map(|c| c as i64);

    tokio::spawn(async move {
        let res = DB
            .query(
                "CREATE search_log SET query = $query, source = $source, category = $category, result_count = $result_count"
            )
            .bind(("query", query))
            .bind(("source", source))
            .bind(("category", category))
            .bind(("result_count", result_count))
            .await;

        if let Err(e) = res {
            warn!(error = %e, "Failed to log search query");
        }
    });
}
