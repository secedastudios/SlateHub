//! Read-side aggregation over `landing_event` for the admin landing-pages
//! funnel report. Mirrors [`crate::models::activity::ActivityModel`]: every
//! count is best-effort and degrades to 0/empty so the admin page never fails.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;
use tracing::debug;

use crate::db::DB;
use crate::services::landing::{self, event};

/// One campaign's funnel totals (all-time), assembled for the admin table.
#[derive(Debug, Clone, Serialize)]
pub struct CampaignFunnel {
    pub slug: String,
    pub title: String,
    /// `view` events (page loads, matching the Meta Pixel `PageView`).
    pub views: u64,
    /// `signup_started` events (reached /signup carrying the campaign).
    pub signups_started: u64,
    /// `signup_completed` events — the conversion (email verified).
    pub conversions: u64,
    /// `signups_started / views`, formatted `"12.3"`.
    pub start_rate: String,
    /// `conversions / views`, formatted `"4.5"` — the headline metric.
    pub conversion_rate: String,
    /// Whether this campaign is still in the in-code registry.
    pub registered: bool,
}

/// `(role, count)` for the signup-started role-chip breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct RoleStat {
    pub role: String,
    pub count: u64,
}

/// Per-day views vs conversions for the trailing window.
#[derive(Debug, Clone, Serialize)]
pub struct DayFunnel {
    pub day: String,
    pub views: u64,
    pub conversions: u64,
}

#[derive(Deserialize, SurrealValue)]
struct CampTypeCount {
    campaign: String,
    event_type: String,
    count: u64,
}

#[derive(Deserialize, SurrealValue)]
struct RoleCount {
    role: String,
    count: u64,
}

#[derive(Deserialize, SurrealValue)]
struct DayTypeCount {
    day: String,
    event_type: String,
    count: u64,
}

fn pct(num: u64, den: u64) -> String {
    if den == 0 {
        "0.0".to_string()
    } else {
        format!("{:.1}", (num as f64 / den as f64) * 100.0)
    }
}

/// Read-only aggregation surface over `landing_event`.
pub struct LandingModel;

impl LandingModel {
    /// All-time funnel totals per campaign. Campaigns are listed in registry
    /// order first (with titles), then any campaign slugs that exist only in
    /// the data (e.g. a retired campaign) are appended.
    pub async fn campaign_funnels() -> Vec<CampaignFunnel> {
        debug!("Aggregating landing-page funnels");
        let rows: Vec<CampTypeCount> = DB
            .query(
                "SELECT campaign, event_type, count() AS count FROM landing_event \
                 GROUP BY campaign, event_type",
            )
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .unwrap_or_default();

        let mut counts: HashMap<(String, String), u64> = HashMap::new();
        for r in rows {
            counts.insert((r.campaign, r.event_type), r.count);
        }

        let get = |slug: &str, ty: &str| {
            counts
                .get(&(slug.to_string(), ty.to_string()))
                .copied()
                .unwrap_or(0)
        };

        let build = |slug: &str, title: String, registered: bool| {
            let views = get(slug, event::VIEW);
            let started = get(slug, event::SIGNUP_STARTED);
            let conversions = get(slug, event::SIGNUP_COMPLETED);
            CampaignFunnel {
                slug: slug.to_string(),
                title,
                views,
                signups_started: started,
                conversions,
                start_rate: pct(started, views),
                conversion_rate: pct(conversions, views),
                registered,
            }
        };

        let mut out: Vec<CampaignFunnel> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for c in landing::CAMPAIGNS {
            seen.insert(c.slug.to_string());
            out.push(build(c.slug, c.title.to_string(), true));
        }
        // Campaigns present only in the data (deregistered) — keep them visible.
        let mut leftover: Vec<String> = counts
            .keys()
            .map(|(slug, _)| slug.clone())
            .filter(|slug| !seen.contains(slug))
            .collect();
        leftover.sort();
        leftover.dedup();
        for slug in leftover {
            out.push(build(&slug, slug.clone(), false));
        }
        out
    }

    /// `signup_started` counts by selected role chip for one campaign,
    /// highest first (best-effort; empty on error).
    pub async fn role_breakdown(campaign: &str) -> Vec<RoleStat> {
        let rows: Vec<RoleCount> = DB
            .query(
                "SELECT role, count() AS count FROM landing_event \
                 WHERE campaign = $c AND event_type = 'signup_started' AND role IS NOT NONE \
                 GROUP BY role ORDER BY count DESC",
            )
            .bind(("c", campaign.to_string()))
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .unwrap_or_default();
        rows.into_iter()
            .map(|r| RoleStat {
                role: r.role,
                count: r.count,
            })
            .collect()
    }

    /// Per-day views vs conversions for one campaign over the trailing
    /// `days` window, oldest first (best-effort; empty on error).
    pub async fn daily(campaign: &str, days: u32) -> Vec<DayFunnel> {
        let query = format!(
            "SELECT <string> time::floor(created_at, 1d) AS day, event_type, count() AS count \
             FROM landing_event WHERE campaign = $c AND created_at > time::now() - {}d \
             GROUP BY day, event_type ORDER BY day",
            days
        );
        let rows: Vec<DayTypeCount> = DB
            .query(&query)
            .bind(("c", campaign.to_string()))
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .unwrap_or_default();

        // Fold (day, event_type) rows into one entry per day, preserving the
        // ascending day order from the query.
        let mut order: Vec<String> = Vec::new();
        let mut by_day: HashMap<String, DayFunnel> = HashMap::new();
        for r in rows {
            let entry = by_day.entry(r.day.clone()).or_insert_with(|| {
                order.push(r.day.clone());
                DayFunnel {
                    day: r.day.clone(),
                    views: 0,
                    conversions: 0,
                }
            });
            match r.event_type.as_str() {
                event::VIEW => entry.views += r.count,
                event::SIGNUP_COMPLETED => entry.conversions += r.count,
                _ => {}
            }
        }
        order
            .into_iter()
            .filter_map(|d| by_day.remove(&d))
            .collect()
    }
}
