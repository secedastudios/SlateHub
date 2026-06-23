//! Public ad landing pages served at `/a/{campaign}` from on-disk Askama
//! templates (not DB-driven). Each render logs a `view` funnel event, sets an
//! anonymous visitor cookie + a campaign-attribution cookie, and shows live
//! verified-profile social proof. The campaign registry lives in
//! [`crate::services::landing`].

use askama::Template;
use axum::{
    Router,
    extract::{Path, Request},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use tracing::error;
use uuid::Uuid;

use crate::{
    error::Error,
    middleware::UserExtractor,
    services::landing::{self, Event},
    templates::{BaseContext, NotOnSetLandingTemplate, User},
};

/// Anonymous visitor id — correlates a visitor's funnel steps (view →
/// signup_started → signup_completed).
const VISITOR_COOKIE: &str = "lp_vid";
/// The campaign a visitor arrived through. Read at `/signup` as an attribution
/// fallback when the `?campaign=` URL param is absent.
pub const CAMPAIGN_COOKIE: &str = "lp_campaign";

pub fn router() -> Router {
    Router::new().route("/a/{campaign}", get(landing))
}

async fn landing(
    Path(slug): Path<String>,
    jar: CookieJar,
    request: Request,
) -> Result<Response, Error> {
    let Some(campaign) = landing::find_campaign(&slug) else {
        return Err(Error::NotFound);
    };

    // Reuse the visitor cookie or mint a new one.
    let (visitor_id, new_visitor) = match jar.get(VISITOR_COOKIE) {
        Some(c) => (c.value().to_string(), false),
        None => (Uuid::new_v4().to_string(), true),
    };

    let mut base = BaseContext::new().with_page("landing");
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    // Real identity-verified profiles that have a photo — no placeholder
    // fallback; the section shows only real verified people.
    let profiles = landing::verified_profiles(8).await;
    let founders = landing::founders().await;
    let community_label = format!("{}+", group_thousands(landing::total_user_count().await));

    // Funnel: record the page view (fire-and-forget, keyed by visitor id).
    landing::record_event(Event {
        campaign: campaign.id.to_string(),
        event_type: landing::event::VIEW.to_string(),
        visitor_id: Some(visitor_id.clone()),
        path: Some(format!("/a/{}", slug)),
        ..Default::default()
    });

    let html = match campaign.slug {
        "not-on-set" => {
            let tpl = crate::with_base!(NotOnSetLandingTemplate, base, {
                campaign_id: campaign.id.to_string(),
                video_id: campaign.video_id.to_string(),
                pixel_id: crate::config::meta_pixel_id(),
                og_title: campaign.title.to_string(),
                og_description: campaign.description.to_string(),
                og_image: campaign.og_image.to_string(),
                path: format!("/a/{}", slug),
                profiles: profiles,
                founders: founders,
                community_label: community_label,
            });
            tpl.render().map_err(|e| {
                error!("Failed to render landing template {}: {}", slug, e);
                Error::template(e.to_string())
            })?
        }
        // Registered campaign without a renderer arm — treat as not found.
        _ => return Err(Error::NotFound),
    };

    // Session cookies: the campaign also rides in every CTA's `?campaign=` URL,
    // and the durable attribution record is `person.signup_campaign`, so these
    // are only a same-session safety net.
    let mut jar = jar.add(session_cookie(CAMPAIGN_COOKIE, campaign.id.to_string()));
    if new_visitor {
        jar = jar.add(session_cookie(VISITOR_COOKIE, visitor_id));
    }

    Ok((jar, Html(html)).into_response())
}

fn session_cookie(name: &'static str, value: String) -> Cookie<'static> {
    Cookie::build((name, value))
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(std::env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) != "false")
        .build()
}

/// Group a non-negative integer with comma thousands separators: `5892` →
/// `"5,892"`.
fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
