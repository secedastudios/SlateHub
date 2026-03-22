use crate::db::DB;
use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;
use tracing::debug;

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CountResult {
    pub count: u64,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct PageStat {
    pub path: String,
    pub views: u64,
}

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
}

pub struct ActivityModel;

impl ActivityModel {
    async fn count(query: &str) -> u64 {
        let result: Option<CountResult> = DB
            .query(query)
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .flatten();
        result.map(|r| r.count).unwrap_or(0)
    }

    async fn active_users(duration: &str) -> u64 {
        let query = format!(
            "SELECT count() AS count FROM (SELECT person_id FROM activity_event WHERE person_id IS NOT NONE AND created_at > time::now() - {} GROUP BY person_id)",
            duration
        );
        Self::count(&query).await
    }

    pub async fn engagement_metrics() -> EngagementMetrics {
        debug!("Fetching engagement metrics");

        let (total_users, dau, wau, mau, new_users_7d, new_users_30d, retained) = tokio::join!(
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
        );

        let pct = |num: u64, den: u64| -> f64 {
            if den == 0 { 0.0 } else { (num as f64 / den as f64) * 100.0 }
        };

        // Users active 30-60 days ago (denominator for retention)
        let prev_period_active = Self::active_users("60d").await.saturating_sub(mau);

        let fmt = |v: f64| format!("{:.1}", v);

        EngagementMetrics {
            total_users,
            dau,
            wau,
            mau,
            stickiness: fmt(if mau == 0 { 0.0 } else { (dau as f64 / mau as f64) * 100.0 }),
            monthly_active_rate: fmt(pct(mau, total_users)),
            weekly_active_rate: fmt(pct(wau, total_users)),
            daily_active_rate: fmt(pct(dau, total_users)),
            new_users_7d,
            new_user_rate: fmt(pct(new_users_30d, mau)),
            retention_rate: fmt(pct(retained, prev_period_active)),
        }
    }

    pub async fn top_pages(limit: u32) -> Vec<PageStat> {
        debug!("Fetching top pages");
        DB.query("SELECT path, count() AS views FROM activity_event WHERE event_type = 'page_view' AND created_at > time::now() - 30d GROUP BY path ORDER BY views DESC LIMIT $limit")
            .bind(("limit", limit))
            .await
            .ok()
            .and_then(|mut r| r.take::<Vec<PageStat>>(0).ok())
            .unwrap_or_default()
    }

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
        results.into_iter().map(|r| (r.event_type, r.count)).collect()
    }

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
