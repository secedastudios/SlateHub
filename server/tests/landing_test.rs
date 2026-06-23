//! Integration tests for the landing-page analytics + attribution feature
//! (`services::landing` + `models::landing`).
//!
//! Covers: campaign registry lookup, funnel event recording, funnel
//! aggregation + conversion-rate maths, registry campaigns surfacing even with
//! zero data, role breakdown (analytics-only role chips), campaign attribution
//! round-trip on the person row, the verified-profile filter, and the total
//! user count. Requires the test SurrealDB (`make test-services test-db-init`).

mod common;

use slatehub::db::DB;
use slatehub::models::landing::LandingModel;
use slatehub::record_id_ext::RecordIdExt;
use slatehub::services::landing::{self, Event, event};
use surrealdb::types::{RecordId, SurrealValue};

const CAMPAIGN: &str = "not-on-set";

fn clean() {
    common::clean_table("landing_event");
    common::clean_table("person");
}

/// Create a person row and return its `RecordId`. `avatar`/`headline` are
/// `None` to exercise the verified-profile filter (which requires both).
async fn mk_person(
    username: &str,
    verification_status: &str,
    avatar: Option<&str>,
    headline: Option<&str>,
) -> RecordId {
    #[derive(serde::Deserialize, SurrealValue)]
    struct R {
        id: RecordId,
    }
    let rows: Vec<R> = DB
        .query(
            "CREATE person CONTENT {
                username: $u, email: $e, password: 'hashed', name: $u,
                verification_status: $vs,
                profile: { name: $u, avatar: $avatar, headline: $headline, skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
            } RETURN id",
        )
        .bind(("u", username.to_string()))
        .bind(("e", format!("{username}@lp.test")))
        .bind(("vs", verification_status.to_string()))
        .bind(("avatar", avatar.map(|s| s.to_string())))
        .bind(("headline", headline.map(|s| s.to_string())))
        .await
        .expect("create person")
        .take(0)
        .expect("take person");
    rows.into_iter().next().expect("one person").id
}

fn ev(event_type: &str) -> Event {
    Event {
        campaign: CAMPAIGN.to_string(),
        event_type: event_type.to_string(),
        ..Default::default()
    }
}

#[test]
fn test_find_campaign_registry() {
    // Pure registry lookup — no DB needed.
    let c = landing::find_campaign(CAMPAIGN).expect("not-on-set is registered");
    assert_eq!(c.id, CAMPAIGN);
    assert!(!c.video_id.is_empty(), "founders video id is set");
    assert!(landing::find_campaign("nope-not-real").is_none());
}

#[test]
fn test_funnel_aggregation_and_rates() {
    common::setup_test_db();
    clean();
    common::run(async {
        for _ in 0..5 {
            landing::record_event_now(Event {
                visitor_id: Some("v".into()),
                ..ev(event::VIEW)
            })
            .await;
        }
        for _ in 0..3 {
            landing::record_event_now(ev(event::SIGNUP_STARTED)).await;
        }
        for _ in 0..2 {
            landing::record_event_now(ev(event::SIGNUP_COMPLETED)).await;
        }

        let funnels = LandingModel::campaign_funnels().await;
        let f = funnels
            .iter()
            .find(|f| f.slug == CAMPAIGN)
            .expect("campaign present in funnel");
        assert_eq!(f.views, 5);
        assert_eq!(f.signups_started, 3);
        assert_eq!(f.conversions, 2);
        assert_eq!(f.start_rate, "60.0", "3/5 started");
        assert_eq!(f.conversion_rate, "40.0", "2/5 converted");
        assert!(f.registered);
    });
}

#[test]
fn test_registered_campaign_listed_with_zero_data() {
    common::setup_test_db();
    clean();
    common::run(async {
        let funnels = LandingModel::campaign_funnels().await;
        let f = funnels
            .iter()
            .find(|f| f.slug == CAMPAIGN)
            .expect("registry campaign listed even with no events");
        assert_eq!(f.views, 0);
        assert_eq!(f.conversions, 0);
        assert_eq!(f.conversion_rate, "0.0", "no divide-by-zero");
        assert!(f.registered);
    });
}

#[test]
fn test_role_breakdown_counts_and_orders() {
    common::setup_test_db();
    clean();
    common::run(async {
        for _ in 0..2 {
            landing::record_event_now(Event {
                role: Some("actor".into()),
                ..ev(event::SIGNUP_STARTED)
            })
            .await;
        }
        landing::record_event_now(Event {
            role: Some("crew".into()),
            ..ev(event::SIGNUP_STARTED)
        })
        .await;
        // A roleless start must not appear in the breakdown.
        landing::record_event_now(ev(event::SIGNUP_STARTED)).await;

        let roles = LandingModel::role_breakdown(CAMPAIGN).await;
        assert_eq!(roles.len(), 2, "only roles with a value, highest first");
        assert_eq!(roles[0].role, "actor");
        assert_eq!(roles[0].count, 2);
        assert_eq!(roles[1].role, "crew");
        assert_eq!(roles[1].count, 1);
    });
}

#[test]
fn test_signup_campaign_attribution_roundtrip() {
    common::setup_test_db();
    clean();
    common::run(async {
        let rid = mk_person("attr_user", "unverified", None, None).await;
        // Unset by default.
        assert!(landing::get_signup_campaign(&rid).await.is_none());

        // set takes a "person:key" string (as the signup handler has it).
        landing::set_signup_campaign(&rid.to_raw_string(), CAMPAIGN).await;
        assert_eq!(
            landing::get_signup_campaign(&rid).await.as_deref(),
            Some(CAMPAIGN),
            "attribution persists on the person row"
        );
    });
}

#[test]
fn test_verified_profiles_filter_and_count() {
    common::setup_test_db();
    clean();
    common::run(async {
        // Included: identity-verified with a photo.
        mk_person("v_inc", "identity", Some("/api/media/a.jpg"), Some("Cinematographer")).await;
        // Included: identity-verified with a photo but no headline — only
        // photo + verified are required; headline falls back at render time.
        mk_person("v_nohl", "identity", Some("/api/media/d.jpg"), None).await;
        // Excluded: identity but missing photo.
        mk_person("v_noavatar", "identity", None, Some("Director")).await;
        // Excluded: email-verified (not identity).
        mk_person("v_email", "email", Some("/api/media/b.jpg"), Some("Actor")).await;
        // Excluded: unverified.
        mk_person("v_unv", "unverified", Some("/api/media/c.jpg"), Some("Gaffer")).await;

        let profiles = landing::verified_profiles(10).await;
        let names: Vec<&str> = profiles.iter().map(|p| p.username.as_str()).collect();
        assert!(names.contains(&"v_inc"), "identity + photo included");
        assert!(names.contains(&"v_nohl"), "identity + photo (no headline) included");
        assert!(!names.contains(&"v_noavatar"), "missing photo excluded");
        assert!(!names.contains(&"v_email"), "email-only verification excluded");
        assert!(!names.contains(&"v_unv"), "unverified excluded");

        let inc = profiles.iter().find(|p| p.username == "v_inc").unwrap();
        assert_eq!(inc.headline, "Cinematographer");
        assert_eq!(inc.avatar, "/api/media/a.jpg");
        // Missing headline → display fallback.
        let nohl = profiles.iter().find(|p| p.username == "v_nohl").unwrap();
        assert_eq!(nohl.headline, "Creative Professional");

        // Count is all person rows (the inflated-with-a-plus social proof figure).
        assert_eq!(landing::total_user_count().await, 5);
    });
}

#[test]
fn test_event_persists_person_and_role() {
    common::setup_test_db();
    clean();
    common::run(async {
        let rid = mk_person("ev_user", "identity", None, None).await;
        landing::record_event_now(Event {
            campaign: CAMPAIGN.to_string(),
            event_type: event::SIGNUP_COMPLETED.to_string(),
            person_id: Some(rid.to_raw_string()),
            role: Some("director".into()),
            visitor_id: Some("vid123".into()),
            path: Some("/verify-email".into()),
        })
        .await;

        #[derive(serde::Deserialize, SurrealValue)]
        struct Row {
            campaign: String,
            event_type: String,
            role: Option<String>,
            visitor_id: Option<String>,
        }
        let rows: Vec<Row> = DB
            .query("SELECT campaign, event_type, role, visitor_id FROM landing_event")
            .await
            .unwrap()
            .take(0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].campaign, CAMPAIGN);
        assert_eq!(rows[0].event_type, "signup_completed");
        assert_eq!(rows[0].role.as_deref(), Some("director"));
        assert_eq!(rows[0].visitor_id.as_deref(), Some("vid123"));

        // The person link is set (RecordId, not a string).
        #[derive(serde::Deserialize, SurrealValue)]
        struct C {
            c: i64,
        }
        let linked: Vec<C> = DB
            .query("SELECT count() AS c FROM landing_event WHERE person IS NOT NONE GROUP ALL")
            .await
            .unwrap()
            .take(0)
            .unwrap();
        assert_eq!(linked.first().map(|r| r.c).unwrap_or(0), 1);
    });
}

// ---------------------------------------------------------------------------
// Render tests — no DB; exercise the full Askama page (extends _layout, all
// includes) and assert the mockup content + funnel wiring are present.
// ---------------------------------------------------------------------------

fn sample_user(has_avatar: bool) -> slatehub::templates::User {
    slatehub::templates::User {
        id: "person:abc".into(),
        name: "Jane Doe".into(),
        email: "jane@example.test".into(),
        avatar: "/api/avatar?id=person:abc".into(),
        avatar_url: if has_avatar {
            Some("/api/media/jane.jpg".into())
        } else {
            None
        },
        initials: "JD".into(),
        notification_count: 0,
        is_identity_verified: has_avatar,
        is_admin: false,
        can_manage_productions: false,
    }
}

fn sample_template(
    pixel_id: Option<String>,
    profiles: Vec<slatehub::services::landing::VerifiedProfile>,
    user: Option<slatehub::templates::User>,
) -> slatehub::templates::NotOnSetLandingTemplate {
    slatehub::templates::NotOnSetLandingTemplate {
        app_name: "SlateHub".into(),
        year: 2026,
        version: "test".into(),
        active_page: "landing".into(),
        user,
        campaign_id: "not-on-set".into(),
        video_id: "otrrrEH8wUw".into(),
        pixel_id,
        og_title: "When you're not on set, there's SlateHub".into(),
        og_description: "The whole film industry in one free profile.".into(),
        og_image: "/static/images/landing/not-on-set/hero-bg.jpg".into(),
        path: "/a/not-on-set".into(),
        profiles,
        founders: vec![
            slatehub::services::landing::FounderCard {
                username: "chris".into(),
                name: "Chris Bruce".into(),
                title: "Co-founder".into(),
                avatar: "/chris.jpg".into(),
            },
            slatehub::services::landing::FounderCard {
                username: "tom".into(),
                name: "Tom Gottschalk".into(),
                title: "Co-founder".into(),
                avatar: "/tom.jpg".into(),
            },
        ],
        community_label: "5,892+".into(),
    }
}

#[test]
fn test_landing_page_renders_mockup_content_and_funnel_wiring() {
    use askama::Template;
    use slatehub::services::landing::VerifiedProfile;

    let profiles = vec![
        VerifiedProfile {
            username: "renjith".into(),
            name: "Renjith R.".into(),
            headline: "Cinematographer".into(),
            avatar: "/api/media/r.jpg".into(),
        },
        VerifiedProfile {
            username: "mariia".into(),
            name: "Mariia S.".into(),
            headline: "Actress".into(),
            avatar: "/api/media/m.jpg".into(),
        },
    ];
    let html = sample_template(Some("1356698509457684".into()), profiles, None)
        .render()
        .expect("landing page renders");

    // Mockup-identical copy.
    assert!(html.contains("When you're not on set"));
    assert!(html.contains("Free for life"));
    // Email capture is a no-JS GET to /signup carrying the campaign + role chips.
    assert!(html.contains(r#"action="/signup""#));
    assert!(html.contains(r#"name="campaign" value="not-on-set""#));
    assert!(html.contains(r#"name="role" value="actor""#));
    // Dynamic social proof + verified carousel linking to /{username}.
    assert!(html.contains("5,892+"));
    assert!(html.contains(r#"href="/renjith""#));
    assert!(html.contains("Cinematographer"));
    // Verified badge is the site-standard blue check, not a custom tick.
    assert!(html.contains(r#"data-role="verified-badge""#));
    assert!(!html.contains("lp-verified-tick"));
    // Final CTA carries the campaign for attribution.
    assert!(html.contains("/signup?campaign=not-on-set"));
    // Meta Pixel preserved + paid-LP noindex.
    assert!(html.contains("fbq('init', '1356698509457684')"));
    assert!(html.contains("noindex"));
    // Founders video: real id, English watch link.
    assert!(html.contains("otrrrEH8wUw"));
    assert!(html.contains("Watch on YouTube"));
    // Founders are linked person cards pointing at their profiles.
    assert!(html.contains(r#"class="lp-founder-card" href="/chris""#));
    assert!(html.contains(r#"class="lp-founder-card" href="/tom""#));
}

#[test]
fn test_landing_pixel_omitted_when_unconfigured() {
    use askama::Template;
    let html = sample_template(None, vec![], None)
        .render()
        .expect("renders without a pixel");
    assert!(
        !html.contains("fbevents.js"),
        "pixel snippet omitted when META_PIXEL_ID is unset"
    );
    // Page still renders its structure without any verified profiles.
    assert!(html.contains("When you're not on set"));
}

#[test]
fn test_logged_in_without_avatar_shows_finish_profile_cta() {
    use askama::Template;
    let html = sample_template(None, vec![], Some(sample_user(false)))
        .render()
        .expect("renders for a logged-in user without an avatar");
    // Quirky "finish your profile + add a photo" CTA → profile edit.
    assert!(html.contains("Finish my profile"));
    assert!(html.contains(r#"class="lp-cta" href="/profile/edit""#));
    assert!(html.contains("Add a photo"));
    // No anonymous signup form / CTA for a logged-in user.
    assert!(!html.contains(r#"name="role" value="actor""#));
    assert!(!html.contains("Create my free profile"));
    assert!(!html.contains("Find collaborators"));
}

#[test]
fn test_logged_in_with_avatar_shows_find_collaborators_cta() {
    use askama::Template;
    let html = sample_template(None, vec![], Some(sample_user(true)))
        .render()
        .expect("renders for a logged-in user with an avatar");
    // Established user → "find collaborators" CTA → people directory.
    assert!(html.contains("Find collaborators"));
    assert!(html.contains(r#"class="lp-cta" href="/people""#));
    // Not the new-user or anonymous CTAs.
    assert!(!html.contains("Finish my profile"));
    assert!(!html.contains("Create my free profile"));
}
