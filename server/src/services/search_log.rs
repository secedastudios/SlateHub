use tracing::warn;

use crate::db::DB;

/// Fire-and-forget search log entry. Spawns a background task so it never blocks search results.
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
