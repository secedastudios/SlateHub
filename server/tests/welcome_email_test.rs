//! Guards the founder welcome email copy (`welcome_email_bodies`): the
//! required messages must stay in, the share/profile links must interpolate,
//! the recipient's name must be HTML-escaped, and the prose must stay free of
//! AI tells (em dashes). Pure function, so no network/DB.

use slatehub::services::email::{FounderTag, WelcomeEmail, WelcomeVideo, welcome_email_bodies};

const INVITE: &str = "https://slatehub.com";
const PROFILE: &str = "https://slatehub.com/janedoe";
const CHRIS: &str = "https://slatehub.com/chris";
const TOM: &str = "https://slatehub.com/tom";
const WATCH: &str = "https://www.youtube.com/watch?v=otrrrEH8wUw";
const IG_URL: &str = "https://www.instagram.com/slatehubofficial";
const IG_HANDLE: &str = "@slatehubofficial";

fn bodies(first: Option<&str>) -> (String, String, String) {
    let founders = vec![
        FounderTag {
            name: "Chris Bruce".to_string(),
            title: "Co-founder".to_string(),
            avatar_url: "https://slatehub.com/api/media/chris.jpg".to_string(),
            profile_url: CHRIS.to_string(),
        },
        FounderTag {
            name: "Tom Gottschalk".to_string(),
            title: "Co-founder".to_string(),
            avatar_url: "https://slatehub.com/api/media/tom.jpg".to_string(),
            profile_url: TOM.to_string(),
        },
    ];
    welcome_email_bodies(&WelcomeEmail {
        recipient_name: first,
        invite_url: INVITE,
        profile_url: PROFILE,
        founders: &founders,
        video: Some(WelcomeVideo {
            thumbnail_url: "https://img.youtube.com/vi/otrrrEH8wUw/hqdefault.jpg",
            watch_url: WATCH,
        }),
        instagram_url: IG_URL,
        instagram_handle: IG_HANDLE,
    })
}

#[test]
fn subject_personalizes_with_first_name() {
    let (subject, _, _) = bodies(Some("Jane Doe"));
    assert_eq!(subject, "Welcome to SlateHub, Jane");
}

#[test]
fn subject_falls_back_without_name() {
    let (subject, _, _) = bodies(None);
    assert_eq!(subject, "Welcome to SlateHub");
}

#[test]
fn text_greeting_uses_first_name_only() {
    let (_, text, _) = bodies(Some("Jane Doe"));
    assert!(text.starts_with("Hey Jane,"), "got: {}", &text[..20]);
}

#[test]
fn carries_every_required_message() {
    // The brief: free + always free, open source, Linktree-style profile,
    // SEO/discoverability, optional one-time paid verification, invite CTA.
    let (_, text, html) = bodies(Some("Jane"));
    for body in [&text, &html] {
        assert!(body.contains("free to join and free to use"));
        assert!(body.contains("always will be"));
        assert!(body.contains("largest free directory"));
        assert!(body.contains("open source"));
        assert!(body.contains("Linktree"));
        assert!(body.contains("optimized for search"));
        assert!(body.contains("one-time payment"));
        assert!(body.contains("optional"));
        assert!(body.to_lowercase().contains("invite"));
        assert!(body.contains("Chris and Tom"));
        // Email verification (what they just did, free) is distinct from the
        // optional, paid identity verification.
        assert!(body.contains("email verification"));
        assert!(body.contains("identity verification"));
        // Actively building + open invitation to request features.
        assert!(body.contains("adding features and improvements"));
    }
}

#[test]
fn includes_founder_cards_linked_to_profiles() {
    let (_, text, html) = bodies(Some("Jane"));
    for body in [&text, &html] {
        assert!(body.contains("Chris Bruce"));
        assert!(body.contains("Tom Gottschalk"));
        assert!(body.contains(CHRIS));
        assert!(body.contains(TOM));
    }
    // HTML cards link to the profiles and show the avatars.
    assert!(html.contains(&format!(r#"href="{CHRIS}""#)));
    assert!(html.contains(&format!(r#"href="{TOM}""#)));
    assert!(html.contains(r#"src="https://slatehub.com/api/media/chris.jpg""#));
}

#[test]
fn includes_story_video_and_instagram() {
    let (_, text, html) = bodies(Some("Jane"));
    for body in [&text, &html] {
        assert!(body.contains(WATCH), "story video link missing");
        assert!(body.contains(IG_HANDLE), "instagram handle missing");
        assert!(body.contains(IG_URL), "instagram url missing");
    }
    // The HTML poster links to the video.
    assert!(html.contains(&format!(r#"href="{WATCH}""#)));
}

#[test]
fn video_is_optional() {
    // No video configured: no story-video link, but everything else renders.
    let founders: Vec<FounderTag> = vec![];
    let (_, text, _) = welcome_email_bodies(&WelcomeEmail {
        recipient_name: Some("Jane"),
        invite_url: INVITE,
        profile_url: PROFILE,
        founders: &founders,
        video: None,
        instagram_url: IG_URL,
        instagram_handle: IG_HANDLE,
    });
    assert!(!text.contains("youtube.com"));
    assert!(text.contains("Thanks for being one of the early ones."));
}

#[test]
fn interpolates_invite_and_profile_urls() {
    let (_, text, html) = bodies(Some("Jane"));
    assert!(text.contains(INVITE));
    assert!(text.contains(PROFILE));
    // HTML wires them into hrefs (the invite link is inline prose now, not a
    // button).
    assert!(html.contains(&format!(r#"href="{INVITE}""#)));
    assert!(html.contains(&format!(r#"href="{PROFILE}""#)));
}

#[test]
fn html_escapes_the_recipient_name() {
    let (_, _, html) = bodies(Some("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("Hey <script>,"));
}

#[test]
fn prose_has_no_em_dashes() {
    // humanizer: em dashes are a top AI-writing tell.
    let (subject, text, html) = bodies(Some("Jane"));
    for body in [&subject, &text, &html] {
        assert!(!body.contains('\u{2014}'), "em dash found");
        assert!(!body.contains('\u{2013}'), "en dash found");
    }
}
