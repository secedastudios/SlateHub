use crate::{db::DB, error::Error};
use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;

pub struct AnalyticsModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferrerCount {
    pub source: String,
    pub count: u64,
}

#[derive(Debug, Clone)]
pub struct PeriodStat {
    pub current: u64,
    pub previous: u64,
}

impl PeriodStat {
    pub fn trend(&self) -> i64 {
        self.current as i64 - self.previous as i64
    }

    pub fn is_up(&self) -> bool {
        self.current > self.previous
    }

    pub fn is_down(&self) -> bool {
        self.current < self.previous
    }
}

#[derive(Debug, Clone)]
pub struct ProfileAnalytics {
    pub total_views: u64,
    pub unique_views: u64,
    pub likes_received: u64,
    pub views_30d: PeriodStat,
    pub views_90d: PeriodStat,
    pub views_1y: PeriodStat,
    pub referrer_breakdown: Vec<ReferrerCount>,
}

fn normalize_referrer(referrer: Option<&str>) -> String {
    match referrer {
        None | Some("") => "direct".to_string(),
        Some(r) => {
            let r_lower = r.to_lowercase();
            if r_lower.contains("google") {
                "google".to_string()
            } else if r_lower.contains("instagram") {
                "instagram".to_string()
            } else if r_lower.contains("twitter") || r_lower.contains("x.com") || r_lower.contains("t.co") {
                "twitter".to_string()
            } else if r_lower.contains("facebook") || r_lower.contains("fb.com") {
                "facebook".to_string()
            } else if r_lower.contains("linkedin") {
                "linkedin".to_string()
            } else if r_lower.contains("tiktok") {
                "tiktok".to_string()
            } else if r_lower.contains("youtube") {
                "youtube".to_string()
            } else {
                // Extract domain
                r_lower
                    .split("//")
                    .nth(1)
                    .and_then(|s| s.split('/').next())
                    .unwrap_or("other")
                    .to_string()
            }
        }
    }
}

impl AnalyticsModel {
    /// Record a profile view (fire-and-forget)
    pub async fn record_view(
        profile_id: &RecordId,
        viewer_id: Option<&RecordId>,
        referrer: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(), Error> {
        let source = normalize_referrer(referrer);

        let mut query_builder = DB
            .query("CREATE profile_view SET profile_id = $profile_id, viewer_id = $viewer_id, referrer = $referrer, referrer_source = $source, user_agent = $ua")
            .bind(("profile_id", profile_id.clone()))
            .bind(("referrer", referrer.map(|s| s.to_string())))
            .bind(("source", source))
            .bind(("ua", user_agent.map(|s| s.to_string())));

        if let Some(vid) = viewer_id {
            query_builder = query_builder.bind(("viewer_id", Some(vid.clone())));
        } else {
            query_builder = query_builder.bind(("viewer_id", None::<RecordId>));
        }

        query_builder
            .await
            .map_err(|e| Error::Database(format!("Failed to record view: {}", e)))?;

        Ok(())
    }

    /// Get total view count for a profile
    pub async fn get_total_views(profile_id: &RecordId) -> Result<u64, Error> {
        let mut result = DB
            .query("SELECT count() AS count FROM profile_view WHERE profile_id = $pid GROUP ALL")
            .bind(("pid", profile_id.clone()))
            .await
            .map_err(|e| Error::Database(format!("Failed to get total views: {}", e)))?;

        let row: Option<serde_json::Value> = result.take(0)?;
        Ok(row
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0))
    }

    /// Get unique viewer count for a profile
    pub async fn get_unique_views(profile_id: &RecordId) -> Result<u64, Error> {
        let mut result = DB
            .query("SELECT count() AS count FROM (SELECT viewer_id FROM profile_view WHERE profile_id = $pid AND viewer_id IS NOT NULL GROUP BY viewer_id)")
            .bind(("pid", profile_id.clone()))
            .await
            .map_err(|e| Error::Database(format!("Failed to get unique views: {}", e)))?;

        let row: Option<serde_json::Value> = result.take(0)?;
        Ok(row
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0))
    }

    /// Get likes received count (people who liked this profile)
    pub async fn get_likes_received(profile_id: &RecordId) -> Result<u64, Error> {
        let mut result = DB
            .query("SELECT count() AS count FROM likes WHERE out = $pid GROUP ALL")
            .bind(("pid", profile_id.clone()))
            .await
            .map_err(|e| Error::Database(format!("Failed to get likes received: {}", e)))?;

        let row: Option<serde_json::Value> = result.take(0)?;
        Ok(row
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0))
    }

    /// Get view count for a period, plus the equivalent previous period for comparison
    pub async fn get_views_for_period(profile_id: &RecordId, days: u32) -> Result<PeriodStat, Error> {
        let query = format!(
            "SELECT count() AS count FROM profile_view WHERE profile_id = $pid AND viewed_at > time::now() - {days}d GROUP ALL;\
             SELECT count() AS count FROM profile_view WHERE profile_id = $pid AND viewed_at > time::now() - {prev}d AND viewed_at <= time::now() - {days}d GROUP ALL;",
            days = days,
            prev = days * 2,
        );

        let mut result = DB
            .query(&query)
            .bind(("pid", profile_id.clone()))
            .await
            .map_err(|e| Error::Database(format!("Failed to get period views: {}", e)))?;

        let current_row: Option<serde_json::Value> = result.take(0)?;
        let current = current_row
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0);

        let previous_row: Option<serde_json::Value> = result.take(1)?;
        let previous = previous_row
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0);

        Ok(PeriodStat { current, previous })
    }

    /// Get referrer source breakdown
    pub async fn get_referrer_breakdown(profile_id: &RecordId) -> Result<Vec<ReferrerCount>, Error> {
        let mut result = DB
            .query("SELECT referrer_source AS source, count() AS count FROM profile_view WHERE profile_id = $pid GROUP BY source ORDER BY count DESC")
            .bind(("pid", profile_id.clone()))
            .await
            .map_err(|e| Error::Database(format!("Failed to get referrer breakdown: {}", e)))?;

        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let source = row.get("source")?.as_str().unwrap_or("unknown").to_string();
                let count = row.get("count")?.as_u64()?;
                Some(ReferrerCount { source, count })
            })
            .collect())
    }

    /// Get all analytics data for a profile
    pub async fn get_profile_analytics(profile_id: &RecordId) -> Result<ProfileAnalytics, Error> {
        let (total_views, unique_views, likes_received, views_30d, views_90d, views_1y, referrer_breakdown) = tokio::join!(
            Self::get_total_views(profile_id),
            Self::get_unique_views(profile_id),
            Self::get_likes_received(profile_id),
            Self::get_views_for_period(profile_id, 30),
            Self::get_views_for_period(profile_id, 90),
            Self::get_views_for_period(profile_id, 365),
            Self::get_referrer_breakdown(profile_id),
        );

        Ok(ProfileAnalytics {
            total_views: total_views.unwrap_or(0),
            unique_views: unique_views.unwrap_or(0),
            likes_received: likes_received.unwrap_or(0),
            views_30d: views_30d.unwrap_or(PeriodStat { current: 0, previous: 0 }),
            views_90d: views_90d.unwrap_or(PeriodStat { current: 0, previous: 0 }),
            views_1y: views_1y.unwrap_or(PeriodStat { current: 0, previous: 0 }),
            referrer_breakdown: referrer_breakdown.unwrap_or_default(),
        })
    }
}
