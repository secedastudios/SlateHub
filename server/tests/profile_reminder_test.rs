//! Guards the profile-completion reminder copy (`profile_reminder_bodies`):
//! the tone escalates snarky -> serious across reminders 1-3, every one links
//! to the profile editor, only the final notice mentions the removal window,
//! and the prose stays free of em dashes. Pure function, no DB/network.

use slatehub::services::email::profile_reminder_bodies;

const EDIT: &str = "https://slatehub.com/profile/edit";

fn reminder(n: u8) -> (String, String, String) {
    profile_reminder_bodies(Some("Jane Doe"), n, EDIT, 7)
}

#[test]
fn greets_by_first_name_only() {
    let (_, text, html) = reminder(1);
    assert!(text.contains("Hey Jane,"));
    assert!(html.contains("Hey Jane,"));
}

#[test]
fn falls_back_without_a_name() {
    let (_, text, _) = profile_reminder_bodies(None, 1, EDIT, 7);
    assert!(text.contains("Hey,"));
}

#[test]
fn tone_escalates_from_snarky_to_serious() {
    let (s1, _, _) = reminder(1);
    let (_, t2, _) = reminder(2);
    let (s3, t3, _) = reminder(3);

    // 1: playful
    assert!(s1.contains("gloriously empty"));
    // 2: pointed — the discoverability argument
    assert!(t2.contains("don't show up"));
    // 3: serious final notice with the removal warning
    assert!(s3.to_lowercase().contains("final notice"));
    assert!(t3.contains("remove the account in 7 days"));
}

#[test]
fn every_reminder_links_to_the_editor() {
    for n in 1..=3u8 {
        let (_, text, html) = reminder(n);
        assert!(
            text.contains(EDIT),
            "reminder {n} text missing the edit link"
        );
        assert!(
            html.contains(&format!(r#"href="{EDIT}""#)),
            "reminder {n} html missing the edit-link button"
        );
    }
}

#[test]
fn cta_label_matches_the_tone() {
    assert!(reminder(1).2.contains("Finish my profile"));
    assert!(reminder(2).2.contains("Finish my profile"));
    assert!(reminder(3).2.contains("Keep my spot"));
}

#[test]
fn only_the_final_notice_mentions_the_grace_window() {
    assert!(!reminder(1).1.contains("7 days"));
    assert!(!reminder(2).1.contains("7 days"));
    assert!(reminder(3).1.contains("7 days"));
}

#[test]
fn grace_window_reflects_the_argument() {
    // The number is interpolated, not hard-coded.
    let (_, text, _) = profile_reminder_bodies(Some("Jane"), 3, EDIT, 10);
    assert!(text.contains("remove the account in 10 days"));
}

#[test]
fn no_em_dashes_anywhere() {
    for n in 1..=3u8 {
        let (subject, text, html) = reminder(n);
        for body in [&subject, &text, &html] {
            assert!(!body.contains('\u{2014}'), "em dash in reminder {n}");
            assert!(!body.contains('\u{2013}'), "en dash in reminder {n}");
        }
    }
}
