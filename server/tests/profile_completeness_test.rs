//! Unit tests for profile-completeness scoring
//! (`services::profile_completeness`). Pure functions — no DB.

use slatehub::services::profile_completeness::{Signals, compute, is_activated};

#[test]
fn empty_profile_scores_zero() {
    let c = compute(&Signals::default());
    assert_eq!(c.percent, 0);
    assert!(!c.complete);
    assert_eq!(c.items.len(), 6, "six core checklist items");
    assert!(c.items.iter().all(|i| !i.done));
}

#[test]
fn photo_plus_headline_is_half_and_activated() {
    let s = Signals {
        has_avatar: true,
        has_headline: true,
        ..Default::default()
    };
    let c = compute(&s);
    assert_eq!(c.percent, 50, "photo (30) + headline (20)");
    assert!(!c.complete);
    assert!(
        is_activated(&s),
        "photo + headline clears the discoverability bar"
    );
}

#[test]
fn photo_or_headline_alone_is_not_activated() {
    let photo_only = Signals {
        has_avatar: true,
        ..Default::default()
    };
    assert_eq!(compute(&photo_only).percent, 30);
    assert!(!is_activated(&photo_only), "no headline → not activated");

    let headline_only = Signals {
        has_headline: true,
        ..Default::default()
    };
    assert!(!is_activated(&headline_only), "no photo → not activated");
}

#[test]
fn all_core_fields_is_complete() {
    let s = Signals {
        has_avatar: true,
        has_headline: true,
        has_skills: true,
        has_credit_or_reel: true,
        has_bio: true,
        has_location: true,
        identity_verified: false,
    };
    let c = compute(&s);
    assert_eq!(c.percent, 100);
    assert!(c.complete);
    assert!(c.items.iter().all(|i| i.done));
    // Verification is a separate next-step nudge, not part of the percent.
    assert!(!c.identity_verified);
}
