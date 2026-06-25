//! Profile-completeness scoring.
//!
//! Drives the "complete your profile" meter on the owner's profile page and
//! the activation metrics in admin. Completeness is a weighted score over the
//! fields that make a member discoverable — a photo and a role/headline matter
//! most because they gate the verified carousel and rank in search.
//!
//! "Activated" is the retention-correlated bar: a profile with **a photo AND a
//! headline** (exactly what the homepage carousel + people search require).

/// One row of the completeness checklist.
#[derive(Debug, Clone)]
pub struct CompletenessItem {
    pub label: &'static str,
    pub href: &'static str,
    pub done: bool,
    /// Contribution to the overall percent (the six items sum to 100).
    pub weight: u8,
}

/// A profile's completeness score + checklist, for the owner-only meter.
#[derive(Debug, Clone)]
pub struct ProfileCompleteness {
    /// 0–100, weighted by field impact.
    pub percent: u8,
    /// True once every core item is done.
    pub complete: bool,
    /// Paid identity verification — a next-step nudge shown once the profile
    /// is complete; not part of `percent`.
    pub identity_verified: bool,
    pub items: Vec<CompletenessItem>,
}

/// The boolean profile signals completeness is scored from.
#[derive(Debug, Clone, Copy, Default)]
pub struct Signals {
    pub has_avatar: bool,
    pub has_headline: bool,
    pub has_skills: bool,
    pub has_credit_or_reel: bool,
    pub has_bio: bool,
    pub has_location: bool,
    pub identity_verified: bool,
}

/// The activation bar: a photo and a headline. This is the threshold that
/// makes a member discoverable (carousel + search), so it's the metric to
/// move.
pub fn is_activated(s: &Signals) -> bool {
    s.has_avatar && s.has_headline
}

/// Score a profile 0–100, weighted toward the fields that drive discovery.
pub fn compute(s: &Signals) -> ProfileCompleteness {
    let items = vec![
        CompletenessItem {
            label: "Add a profile photo",
            href: "/profile/edit",
            done: s.has_avatar,
            weight: 30,
        },
        CompletenessItem {
            label: "Add your role / headline",
            href: "/profile/edit",
            done: s.has_headline,
            weight: 20,
        },
        CompletenessItem {
            label: "List a few skills",
            href: "/profile/edit",
            done: s.has_skills,
            weight: 15,
        },
        CompletenessItem {
            label: "Add a credit or reel",
            href: "/profile/edit",
            done: s.has_credit_or_reel,
            weight: 15,
        },
        CompletenessItem {
            label: "Write a short bio",
            href: "/profile/edit",
            done: s.has_bio,
            weight: 10,
        },
        CompletenessItem {
            label: "Set your location",
            href: "/profile/edit",
            done: s.has_location,
            weight: 10,
        },
    ];
    let percent: u32 = items
        .iter()
        .filter(|i| i.done)
        .map(|i| i.weight as u32)
        .sum();
    ProfileCompleteness {
        percent: percent.min(100) as u8,
        complete: percent >= 100,
        identity_verified: s.identity_verified,
        items,
    }
}
