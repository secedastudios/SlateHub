//! Site-wide activity metrics for the admin dashboard.
//!
//! Read-side aggregation over the `activity_event` time-series table
//! (page views, logins, etc. — events are written by the tracking
//! middleware, not here), plus a retention `cleanup`. Called by
//! `routes/admin.rs` for the dashboard and by the periodic cleanup task
//! spawned in `main.rs`. Aggregate queries use `GROUP ALL` (or a `GROUP BY`
//! subquery for distinct counts) so `count()` comes back as a single row.

use crate::db::DB;
use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;
use tracing::debug;

/// Row shape for `SELECT count() AS count ... GROUP ALL` aggregates.
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CountResult {
    pub count: u64,
}

/// Per-path page-view total (last 30 days), for the "top pages" table.
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct PageStat {
    pub path: String,
    pub views: u64,
}

/// Events per calendar day; `day` is `time::floor(created_at, 1d)` cast to
/// `<string>` in the query (RecordId/datetime values can't deserialize into
/// plain `String` fields otherwise).
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct DayStat {
    pub day: String,
    pub events: u64,
}

/// Engagement metrics for the admin dashboard.
/// Percentages are pre-formatted as strings for templates.
#[derive(Debug, Clone, Serialize)]
pub struct EngagementMetrics {
    pub total_users: u64,
    pub dau: u64,
    pub wau: u64,
    pub mau: u64,
    pub stickiness: String,
    pub monthly_active_rate: String,
    pub weekly_active_rate: String,
    pub daily_active_rate: String,
    pub new_users_7d: u64,
    pub new_user_rate: String,
    pub retention_rate: String,
    pub page_views_today: String,
    pub page_views_7d: String,
    pub page_views_30d: String,
    pub unique_visitors_today: String,
    pub unique_visitors_7d: String,
    pub unique_visitors_30d: String,
}

/// Abbreviate large counts for display: 1532 → "1.5k", 2_000_000 → "2M".
fn abbr(n: u64) -> String {
    if n >= 1_000_000 {
        let v = n as f64 / 1_000_000.0;
        let s = format!("{:.1}", v);
        format!("{}M", s.strip_suffix(".0").unwrap_or(&s))
    } else if n >= 1_000 {
        let v = n as f64 / 1_000.0;
        let s = format!("{:.1}", v);
        format!("{}k", s.strip_suffix(".0").unwrap_or(&s))
    } else {
        n.to_string()
    }
}

/// Read-only aggregation surface over `activity_event` (plus `cleanup`).
pub struct ActivityModel;

impl ActivityModel {
    /// Run a `count() AS count` query, swallowing errors as 0 (metrics are
    /// best-effort and must never fail the dashboard).
    async fn count(query: &str) -> u64 {
        let result: Option<CountResult> = DB
            .query(query)
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .flatten();
        result.map(|r| r.count).unwrap_or(0)
    }

    /// Count distinct signed-in people with any event in the trailing
    /// `duration` (a SurrealQL duration literal like "7d").
    async fn active_users(duration: &str) -> u64 {
        let query = format!(
            "SELECT count() AS count FROM (SELECT person_id FROM activity_event WHERE person_id IS NOT NONE AND created_at > time::now() - {} GROUP BY person_id)",
            duration
        );
        Self::count(&query).await
    }

    /// Compute the full admin-dashboard engagement block (DAU/WAU/MAU,
    /// stickiness, retention, page views, unique visitors) with all count
    /// queries issued concurrently via `tokio::join!`. Infallible: each
    /// failed sub-count degrades to 0.
    pub async fn engagement_metrics() -> EngagementMetrics {
        debug!("Fetching engagement metrics");

        let (total_users, dau, wau, mau, new_users_7d, new_users_30d, retained, pv_today, pv_7d, pv_30d, uv_today, uv_7d, uv_30d) = tokio::join!(
            Self::count("SELECT count() AS count FROM person GROUP ALL"),
            Self::active_users("1d"),
            Self::active_users("7d"),
            Self::active_users("30d"),
            // New users active in last 7 days (created in last 7d and had activity)
            Self::count(
                "SELECT count() AS count FROM (
                    SELECT person_id FROM activity_event
                    WHERE person_id IS NOT NONE AND created_at > time::now() - 7d
                    GROUP BY person_id
                ) WHERE person_id.created_at > time::now() - 7d"
            ),
            // New users in last 30 days who were active
            Self::count(
                "SELECT count() AS count FROM (
                    SELECT person_id FROM activity_event
                    WHERE person_id IS NOT NONE AND created_at > time::now() - 30d
                    GROUP BY person_id
                ) WHERE person_id.created_at > time::now() - 30d"
            ),
            // Retention: users active 30-60d ago who are also active in last 30d
            Self::count(
                "SELECT count() AS count FROM (
                    SELECT person_id FROM activity_event
                    WHERE person_id IS NOT NONE AND created_at > time::now() - 60d AND created_at < time::now() - 30d
                    GROUP BY person_id
                ) WHERE person_id IN (
                    SELECT VALUE person_id FROM activity_event
                    WHERE person_id IS NOT NONE AND created_at > time::now() - 30d
                    GROUP BY person_id
                )"
            ),
            // Total page views (all visitors, including anonymous)
            Self::count("SELECT count() AS count FROM activity_event WHERE event_type = 'page_view' AND created_at > time::now() - 1d GROUP ALL"),
            Self::count("SELECT count() AS count FROM activity_event WHERE event_type = 'page_view' AND created_at > time::now() - 7d GROUP ALL"),
            Self::count("SELECT count() AS count FROM activity_event WHERE event_type = 'page_view' AND created_at > time::now() - 30d GROUP ALL"),
            // Unique visitors (distinct person_ids with page views — authenticated users only)
            Self::count("SELECT count() AS count FROM (SELECT person_id FROM activity_event WHERE event_type = 'page_view' AND person_id IS NOT NONE AND created_at > time::now() - 1d GROUP BY person_id)"),
            Self::count("SELECT count() AS count FROM (SELECT person_id FROM activity_event WHERE event_type = 'page_view' AND person_id IS NOT NONE AND created_at > time::now() - 7d GROUP BY person_id)"),
            Self::count("SELECT count() AS count FROM (SELECT person_id FROM activity_event WHERE event_type = 'page_view' AND person_id IS NOT NONE AND created_at > time::now() - 30d GROUP BY person_id)"),
        );

        let pct = |num: u64, den: u64| -> f64 {
            if den == 0 {
                0.0
            } else {
                (num as f64 / den as f64) * 100.0
            }
        };

        // Users active 30-60 days ago (denominator for retention)
        let prev_period_active = Self::active_users("60d").await.saturating_sub(mau);

        let fmt = |v: f64| format!("{:.1}", v);

        EngagementMetrics {
            total_users,
            dau,
            wau,
            mau,
            stickiness: fmt(if mau == 0 {
                0.0
            } else {
                (dau as f64 / mau as f64) * 100.0
            }),
            monthly_active_rate: fmt(pct(mau, total_users)),
            weekly_active_rate: fmt(pct(wau, total_users)),
            daily_active_rate: fmt(pct(dau, total_users)),
            new_users_7d,
            new_user_rate: fmt(pct(new_users_30d, mau)),
            retention_rate: fmt(pct(retained, prev_period_active)),
            page_views_today: abbr(pv_today),
            page_views_7d: abbr(pv_7d),
            page_views_30d: abbr(pv_30d),
            unique_visitors_today: abbr(uv_today),
            unique_visitors_7d: abbr(uv_7d),
            unique_visitors_30d: abbr(uv_30d),
        }
    }

    /// Most-viewed paths over the last 30 days (best-effort; empty on error).
    pub async fn top_pages(limit: u32) -> Vec<PageStat> {
        debug!("Fetching top pages");
        DB.query("SELECT path, count() AS views FROM activity_event WHERE event_type = 'page_view' AND created_at > time::now() - 30d GROUP BY path ORDER BY views DESC LIMIT $limit")
            .bind(("limit", limit))
            .await
            .ok()
            .and_then(|mut r| r.take::<Vec<PageStat>>(0).ok())
            .unwrap_or_default()
    }

    /// Event totals per day for the trailing `days` window, oldest first
    /// (best-effort; empty on error).
    pub async fn daily_activity(days: u32) -> Vec<DayStat> {
        debug!("Fetching daily activity");
        let query = format!(
            "SELECT <string> time::floor(created_at, 1d) AS day, count() AS events FROM activity_event WHERE created_at > time::now() - {}d GROUP BY day ORDER BY day",
            days
        );
        DB.query(&query)
            .await
            .ok()
            .and_then(|mut r| r.take::<Vec<DayStat>>(0).ok())
            .unwrap_or_default()
    }

    /// 30-day totals per `event_type` ("page_view", "login", …), highest
    /// first (best-effort; empty on error).
    pub async fn event_counts() -> Vec<(String, u64)> {
        debug!("Fetching event type counts");
        #[derive(Deserialize, SurrealValue)]
        struct TypeCount {
            event_type: String,
            count: u64,
        }
        let results: Vec<TypeCount> = DB
            .query("SELECT event_type, count() AS count FROM activity_event WHERE created_at > time::now() - 30d GROUP BY event_type ORDER BY count DESC")
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .unwrap_or_default();
        results
            .into_iter()
            .map(|r| (r.event_type, r.count))
            .collect()
    }

    /// Delete events older than `days`; errors are logged, never returned
    /// (runs unattended from the background task in `main.rs`).
    pub async fn cleanup(days: u32) {
        debug!("Cleaning up activity events older than {} days", days);
        let query = format!(
            "DELETE FROM activity_event WHERE created_at < time::now() - {}d",
            days
        );
        if let Err(e) = DB.query(&query).await {
            tracing::error!("Failed to cleanup activity events: {}", e);
        }
    }
}
