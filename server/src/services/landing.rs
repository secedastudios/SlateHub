//! Landing-page campaign registry + funnel event recording.
//!
//! The `/a/{campaign}` ad landing pages are rendered from on-disk Askama
//! templates (not DB-driven). This module is the source of truth for *which*
//! campaigns exist ([`CAMPAIGNS`]) and writes the per-campaign funnel events
//! into the `landing_event` table.
//!
//! Writes follow the same fire-and-forget pattern as
//! [`crate::services::activity`] for the hot view path ([`record_event`]),
//! plus an awaitable variant ([`record_event_now`]) used at the conversion
//! (email-verification) step so it's deterministic and testable.

use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{trace, warn};

use crate::db::DB;

// ---------------------------------------------------------------------------
// Campaign registry
// ---------------------------------------------------------------------------

/// Compile-time definition of one on-disk ad landing page. Adding a campaign
/// = append an entry here, add a template renderer arm in
/// `routes/landing.rs`, and drop its art under
/// `static/images/landing/{slug}/`.
pub struct Campaign {
    /// URL slug — the page is served at `/a/{slug}`.
    pub slug: &'static str,
    /// Analytics campaign id stored on every `landing_event` (kept equal to
    /// `slug` for now, but separate so a slug can change without losing
    /// historical attribution).
    pub id: &'static str,
    /// `<title>` / OG title.
    pub title: &'static str,
    /// Meta + OG description.
    pub description: &'static str,
    /// Absolute path to the OG/share image.
    pub og_image: &'static str,
    /// YouTube id for the founders video embedded on the page.
    pub video_id: &'static str,
}

/// Source of truth for which landing pages exist.
pub const CAMPAIGNS: &[Campaign] = &[Campaign {
    slug: "not-on-set",
    id: "not-on-set",
    title: "When you're not on set, there's SlateHub",
    description: "The whole film industry in one free profile. Free for life — because actors and crew should never pay to be visible.",
    og_image: "/static/images/landing/not-on-set/hero-bg.jpg",
    video_id: "otrrrEH8wUw",
}];

/// Resolve a campaign by its URL slug.
pub fn find_campaign(slug: &str) -> Option<&'static Campaign> {
    CAMPAIGNS.iter().find(|c| c.slug == slug)
}

// ---------------------------------------------------------------------------
// Funnel event recording
// ---------------------------------------------------------------------------

/// One funnel stage. Stored as the `landing_event.event_type` string; the
/// schema ASSERTs this exact set.
pub mod event {
    pub const VIEW: &str = "view";
    pub const CTA_CLICK: &str = "cta_click";
    pub const SIGNUP_STARTED: &str = "signup_started";
    pub const SIGNUP_COMPLETED: &str = "signup_completed";
}

/// Owned, spawn-safe parameters for one funnel event.
#[derive(Default)]
pub struct Event {
    pub campaign: String,
    pub event_type: String,
    /// `person:key` or bare key; `None` for anonymous events.
    pub person_id: Option<String>,
    /// Selected role chip — analytics only, never applied to the account.
    pub role: Option<String>,
    /// Anonymous cookie id correlating a visitor's funnel steps.
    pub visitor_id: Option<String>,
    pub path: Option<String>,
}

async fn insert(ev: Event) -> Result<(), surrealdb::Error> {
    let person = ev.person_id.as_deref().map(|pid| {
        let key = pid.strip_prefix("person:").unwrap_or(pid);
        RecordId::new("person", key)
    });
    DB.query(
        "CREATE landing_event SET campaign = $campaign, event_type = $event_type, \
         person = $person, role = $role, visitor_id = $visitor_id, path = $path",
    )
    .bind(("campaign", ev.campaign))
    .bind(("event_type", ev.event_type))
    .bind(("person", person))
    .bind(("role", ev.role))
    .bind(("visitor_id", ev.visitor_id))
    .bind(("path", ev.path))
    .await?;
    Ok(())
}

/// Fire-and-forget: spawn a task to record the event so request latency is
/// never affected. Use on the hot view / signup-started paths.
pub fn record_event(ev: Event) {
    let (campaign, event_type) = (ev.campaign.clone(), ev.event_type.clone());
    tokio::spawn(async move {
        match insert(ev).await {
            Ok(_) => trace!(%campaign, %event_type, "landing event logged"),
            Err(e) => warn!(error = %e, %campaign, %event_type, "failed to log landing event"),
        }
    });
}

/// Awaitable record — used for the `signup_completed` conversion so it's
/// recorded deterministically before the response returns. Errors are logged,
/// never propagated (analytics must never break the funnel).
pub async fn record_event_now(ev: Event) {
    let (campaign, event_type) = (ev.campaign.clone(), ev.event_type.clone());
    if let Err(e) = insert(ev).await {
        warn!(error = %e, %campaign, %event_type, "failed to record landing conversion");
    }
}

// ---------------------------------------------------------------------------
// Attribution (person.signup_campaign)
// ---------------------------------------------------------------------------

/// Persist the campaign a person signed up through. Written at account
/// creation; read at email-verification to emit the conversion. Best-effort.
pub async fn set_signup_campaign(person_id: &str, campaign: &str) {
    let key = person_id.strip_prefix("person:").unwrap_or(person_id);
    let rid = RecordId::new("person", key);
    if let Err(e) = DB
        .query("UPDATE $id SET signup_campaign = $campaign")
        .bind(("id", rid))
        .bind(("campaign", campaign.to_string()))
        .await
    {
        warn!(error = %e, "failed to set signup_campaign");
    }
}

/// Read the campaign a person signed up through, if any. Returns `None` when
/// unset or on error.
pub async fn get_signup_campaign(person_id: &RecordId) -> Option<String> {
    DB.query("SELECT VALUE signup_campaign FROM $id")
        .bind(("id", person_id.clone()))
        .await
        .ok()
        .and_then(|mut r| r.take::<Vec<Option<String>>>(0).ok())
        .and_then(|v| v.into_iter().next().flatten())
}

// ---------------------------------------------------------------------------
// Dynamic page data (verified profiles + user count)
// ---------------------------------------------------------------------------

/// A verified profile rendered in the landing carousel + hero social-proof
/// avatars. Mirrors the homepage ticker query (`verification_status =
/// 'identity'`, must have an avatar + headline) so the same real people
/// surface on both surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct VerifiedProfile {
    pub username: String,
    pub name: String,
    pub headline: String,
    pub avatar: String,
}

/// Up to `limit` identity-verified profiles that have an avatar, shuffled
/// per call (`ORDER BY rand()`) so the landing carousel + hero avatars vary on
/// every page load. Best-effort: empty on error.
pub async fn verified_profiles(limit: usize) -> Vec<VerifiedProfile> {
    #[derive(Deserialize, SurrealValue)]
    struct Row {
        username: String,
        name: Option<String>,
        headline: Option<String>,
        avatar: Option<String>,
    }
    let q = format!(
        "SELECT username, profile.name AS name, profile.headline AS headline, profile.avatar AS avatar \
         FROM person WHERE profile.avatar IS NOT NONE \
         AND verification_status = 'identity' ORDER BY rand() LIMIT {};",
        limit
    );
    let rows: Vec<Row> = DB
        .query(&q)
        .await
        .ok()
        .and_then(|mut r| r.take(0).ok())
        .unwrap_or_default();
    rows.into_iter()
        .map(|r| VerifiedProfile {
            username: r.username.clone(),
            name: r.name.unwrap_or_else(|| r.username.clone()),
            headline: r
                .headline
                .unwrap_or_else(|| "Creative Professional".to_string()),
            avatar: r.avatar.unwrap_or_default(),
        })
        .collect()
}

/// Total number of `person` rows — the social-proof "N+" figure (shown
/// inflated-with-a-plus on purpose). Best-effort: 0 on error.
pub async fn total_user_count() -> u64 {
    #[derive(Deserialize, SurrealValue)]
    struct C {
        count: u64,
    }
    DB.query("SELECT count() AS count FROM person GROUP ALL")
        .await
        .ok()
        .and_then(|mut r| r.take::<Option<C>>(0).ok().flatten())
        .map(|c| c.count)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Founder cards ("from the founders" section)
// ---------------------------------------------------------------------------

/// A founder card, enriched from the founder's live `person` profile when
/// present, falling back to baked-in defaults so the section always renders
/// (e.g. on an unseeded DB or a missing field).
#[derive(Debug, Clone)]
pub struct FounderCard {
    pub username: String,
    pub name: String,
    pub title: String,
    pub avatar: String,
}

/// The SlateHub founders (chris, tom), with name/title/avatar read from their
/// profiles. Defaults cover an unseeded DB or a profile missing a field.
pub async fn founders() -> Vec<FounderCard> {
    // (username, fallback name, fallback title, fallback avatar)
    const FOUNDERS: &[(&str, &str, &str, &str)] = &[
        (
            "chris",
            "Chris Bruce",
            "Co-founder",
            "https://slatehub.com/api/media/profiles/ajf23cc8fxayeaehxtsu/01KKWK5BP899ZTPE9BXQQHV17Z.jpg",
        ),
        (
            "tom",
            "Tom Gottschalk",
            "Co-founder",
            "https://slatehub.com/api/media/profiles/dj03mga50rm3lw6c8r14/01KMGCJ4ZBKJEZMTQZFJG8JA0P.jpg",
        ),
    ];

    #[derive(Deserialize, SurrealValue)]
    struct Row {
        username: String,
        name: Option<String>,
        headline: Option<String>,
        avatar: Option<String>,
    }
    let rows: Vec<Row> = DB
        .query(
            "SELECT username, profile.name AS name, profile.headline AS headline, \
             profile.avatar AS avatar FROM person WHERE username IN ['chris', 'tom']",
        )
        .await
        .ok()
        .and_then(|mut r| r.take(0).ok())
        .unwrap_or_default();
    let by_user: std::collections::HashMap<String, Row> =
        rows.into_iter().map(|r| (r.username.clone(), r)).collect();

    // Non-empty DB value, else the baked-in fallback.
    fn pick(value: Option<&String>, fallback: &str) -> String {
        value
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    FOUNDERS
        .iter()
        .map(|(username, name, title, avatar)| {
            let row = by_user.get(*username);
            FounderCard {
                username: username.to_string(),
                name: pick(row.and_then(|r| r.name.as_ref()), name),
                title: pick(row.and_then(|r| r.headline.as_ref()), title),
                avatar: pick(row.and_then(|r| r.avatar.as_ref()), avatar),
            }
        })
        .collect()
}
