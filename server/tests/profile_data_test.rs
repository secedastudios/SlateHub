//! Unit tests for `ProfileData::is_unset`, the single source of truth behind
//! the "Profile Not Set Up" empty state. `persons/profile.html` gates its
//! empty-state section on this method, and `routes::public_profiles` redirects
//! the owner straight to `/profile/edit` when it holds — so the two must agree.
//! These tests pin the contract: which fields count as "content" and which
//! (avatar, identity, account scaffolding) are intentionally ignored.

use slatehub::templates::{Education, InvolvementDisplay, PhotoDisplay, ProfileData, ReelDisplay};

/// A profile with no user-supplied content — every Option `None`, every Vec
/// empty. Only the bare account scaffolding (id/name/username/email/initials)
/// is populated. `is_unset()` must report `true` for this.
fn blank() -> ProfileData {
    ProfileData {
        id: "person:test".to_string(),
        name: "Test User".to_string(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        avatar: None,
        initials: "TU".to_string(),
        headline: None,
        bio: None,
        location: None,
        website: None,
        skills: vec![],
        languages: vec![],
        availability: None,
        involvements: vec![],
        education: vec![],
        social_links: vec![],
        reels: vec![],
        photos: vec![],
        is_own_profile: true,
        is_public: false,
        verification_status: "none".to_string(),
        gender: None,
        birthday: None,
        height_mm: None,
        weight_kg: None,
        body_type: None,
        hair_color: None,
        eye_color: None,
        ethnicity: vec![],
        acting_age_range_min: None,
        acting_age_range_max: None,
        acting_ethnicities: vec![],
        nationality: None,
        messaging_preference: "open".to_string(),
        phone: None,
    }
}

fn sample_involvement() -> InvolvementDisplay {
    InvolvementDisplay {
        involvement_id: "involvement:1".to_string(),
        role: None,
        relation_type: "cast".to_string(),
        department: None,
        verification_status: "none".to_string(),
        production_title: "A Film".to_string(),
        production_slug: "a-film".to_string(),
        production_type: "feature".to_string(),
        poster_url: None,
        tmdb_url: None,
        release_date: None,
        media_type: None,
        is_claimed: false,
    }
}

fn sample_education() -> Education {
    Education {
        institution: "Film School".to_string(),
        degree: None,
        field: None,
        dates: None,
    }
}

fn sample_reel() -> ReelDisplay {
    ReelDisplay {
        url: "https://youtu.be/abc".to_string(),
        title: "Reel".to_string(),
        platform: "youtube".to_string(),
        video_id: "abc".to_string(),
        thumbnail_url: String::new(),
        embed_url: String::new(),
        platform_name: "YouTube".to_string(),
    }
}

fn sample_photo() -> PhotoDisplay {
    PhotoDisplay {
        url: "https://example.com/p.jpg".to_string(),
        thumbnail_url: String::new(),
        caption: String::new(),
    }
}

#[test]
fn blank_profile_is_unset() {
    assert!(
        blank().is_unset(),
        "a profile with no content of any kind must read as unset"
    );
}

#[test]
fn avatar_does_not_count_as_content() {
    // A photo can be uploaded before anything else is filled in; the empty
    // state (and therefore the redirect) deliberately ignores the avatar so an
    // avatar-only profile is still steered into the edit form.
    let mut p = blank();
    p.avatar = Some("https://example.com/a.jpg".to_string());
    assert!(
        p.is_unset(),
        "an avatar alone must not mark a profile as set"
    );
}

#[test]
fn account_scaffolding_does_not_count_as_content() {
    // Identity status, visibility, and physical attributes are not the
    // "content" the empty state is about — none of them should flip is_unset.
    let mut p = blank();
    p.verification_status = "identity".to_string();
    p.is_public = true;
    p.gender = Some("female".to_string());
    p.height_mm = Some(1700);
    p.ethnicity = vec!["white".to_string()];
    p.nationality = Some("DE".to_string());
    assert!(
        p.is_unset(),
        "verification/visibility/physical attributes must not mark a profile as set"
    );
}

/// Apply `set` to a blank profile and assert that the single field it touched
/// flips `is_unset()` to false.
fn assert_field_marks_set(field: &str, set: impl FnOnce(&mut ProfileData)) {
    let mut p = blank();
    set(&mut p);
    assert!(
        !p.is_unset(),
        "setting `{field}` should mark the profile as set (is_unset must be false)"
    );
}

#[test]
fn each_content_field_marks_the_profile_set() {
    // Every field the template's empty-state condition checks must, on its own,
    // make is_unset() false. One assertion per field guarantees the method and
    // the template stay in lockstep.
    assert_field_marks_set("headline", |p| p.headline = Some("Director".into()));
    assert_field_marks_set("bio", |p| p.bio = Some("Hello".into()));
    assert_field_marks_set("location", |p| p.location = Some("Berlin".into()));
    assert_field_marks_set("website", |p| p.website = Some("https://x.dev".into()));
    assert_field_marks_set("availability", |p| {
        p.availability = Some("available".into())
    });
    assert_field_marks_set("skills", |p| p.skills = vec!["Editing".into()]);
    assert_field_marks_set("languages", |p| p.languages = vec!["German".into()]);
    assert_field_marks_set("involvements", |p| {
        p.involvements = vec![sample_involvement()]
    });
    assert_field_marks_set("education", |p| p.education = vec![sample_education()]);
    assert_field_marks_set("reels", |p| p.reels = vec![sample_reel()]);
    assert_field_marks_set("photos", |p| p.photos = vec![sample_photo()]);
}
