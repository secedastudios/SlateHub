//! The opt-in public contact email (`profile.public_email`): schema
//! validation, `Person::update_profile` round-trips, and the rendering
//! guarantee that a profile page shows an email only when the person chose
//! one — the sign-in email is no longer part of `ProfileData` at all, so no
//! template can display it.

mod common;

use askama::Template;
use slatehub::db::DB;
use slatehub::models::person::Person;
use slatehub::record_id_ext::RecordIdExt;
use slatehub::templates::{BaseContext, ProfileData, ProfileEditTemplate, ProfileTemplate};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(serde::Deserialize, SurrealValue)]
struct R {
    id: RecordId,
}

async fn create_person(username: &str) -> RecordId {
    let rows: Vec<R> = DB
        .query(
            "CREATE person CONTENT {
                username: $username, email: $email, password: 'h', name: 'Pat',
                verification_status: 'email',
                profile: { name: 'Pat', headline: 'Gaffer', skills: [], social_links: [],
                           ethnicity: [], unions: [], languages: [], education: [],
                           reels: [], media_other: [], awards: [] }
            } RETURN id",
        )
        .bind(("username", username.to_string()))
        .bind(("email", format!("{username}@signin.test")))
        .await
        .expect("create person")
        .take(0)
        .expect("take person");
    rows.into_iter().next().expect("one person").id
}

async fn public_email_of(id: &RecordId) -> Option<String> {
    let mut r = DB
        .query("SELECT VALUE profile.public_email FROM $id")
        .bind(("id", id.clone()))
        .await
        .expect("query");
    r.take::<Option<String>>(0).expect("take")
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

#[test]
fn schema_accepts_none_or_a_valid_address_and_rejects_garbage() {
    common::setup_test_db();
    common::clean_table("person");
    common::run(async {
        let id = create_person("pubmail_schema").await;
        assert_eq!(public_email_of(&id).await, None, "unset by default");

        let bad = DB
            .query("UPDATE $id SET profile.public_email = 'not-an-email'")
            .bind(("id", id.clone()))
            .await
            .expect("send")
            .check();
        assert!(bad.is_err(), "ASSERT must reject a malformed address");

        DB.query("UPDATE $id SET profile.public_email = 'hello@example.test'")
            .bind(("id", id.clone()))
            .await
            .expect("send")
            .check()
            .expect("a real address is accepted");
        assert_eq!(
            public_email_of(&id).await,
            Some("hello@example.test".to_string())
        );

        DB.query("UPDATE $id SET profile.public_email = NONE")
            .bind(("id", id.clone()))
            .await
            .expect("send")
            .check()
            .expect("clearing is allowed");
        assert_eq!(public_email_of(&id).await, None);
    });
    common::clean_table("person");
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// `Person::update_profile` with only the public email supplied.
async fn set_public_email(user_id: &str, value: Option<&str>) {
    Person::update_profile(
        user_id,
        None,
        None,
        None,
        None,
        None,
        value.map(String::from),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("update_profile")
    .expect("person exists");
}

#[test]
fn update_profile_sets_trims_keeps_and_clears_the_public_email() {
    common::setup_test_db();
    common::clean_table("person");
    common::run(async {
        let id = create_person("pubmail_update").await;
        let uid = id.to_raw_string();

        set_public_email(&uid, Some("  Hello@Example.test ")).await;
        assert_eq!(
            public_email_of(&id).await,
            Some("Hello@Example.test".to_string()),
            "stored trimmed, case preserved"
        );

        // Field absent from the form → untouched.
        set_public_email(&uid, None).await;
        assert_eq!(
            public_email_of(&id).await,
            Some("Hello@Example.test".to_string())
        );

        // Empty string → cleared.
        set_public_email(&uid, Some("")).await;
        assert_eq!(public_email_of(&id).await, None);

        // The rest of the profile survived all of that.
        let person = Person::find_by_username("pubmail_update")
            .await
            .expect("find")
            .expect("exists");
        assert_eq!(
            person.profile.and_then(|p| p.headline),
            Some("Gaffer".to_string())
        );
    });
    common::clean_table("person");
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

/// A public, email-verified profile with enough content to render the
/// details list. `is_public` is on — that flag used to expose the sign-in
/// email and must no longer reveal anything email-related on its own.
fn profile_data(public_email: Option<&str>) -> ProfileData {
    ProfileData {
        id: "person:pubmail".to_string(),
        name: "Pat Public".to_string(),
        username: "patpublic".to_string(),
        public_email: public_email.map(String::from),
        avatar: None,
        initials: "PP".to_string(),
        headline: Some("Gaffer".to_string()),
        bio: None,
        location: Some("Berlin".to_string()),
        website: None,
        skills: vec![],
        languages: vec![],
        availability: None,
        involvements: vec![],
        education: vec![],
        social_links: vec![],
        reels: vec![],
        photos: vec![],
        is_own_profile: false,
        is_public: true,
        verification_status: "email".to_string(),
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
        messaging_preference: "anyone".to_string(),
        phone: Some("+49 30 0000".to_string()),
    }
}

fn render_profile(profile: ProfileData) -> String {
    let base = BaseContext::new().with_page("profile");
    slatehub::with_base!(ProfileTemplate, base, {
        profile,
        is_liked: false,
        completeness: None,
    })
    .render()
    .expect("profile template renders")
}

fn render_edit(profile: ProfileData) -> String {
    let base = BaseContext::new().with_page("profile");
    slatehub::with_base!(ProfileEditTemplate, base, {
        photo_count: 0,
        reel_count: 0,
        photo_limit: None,
        reel_limit: None,
        is_identity_verified: false,
        profile,
        platforms: vec![],
        error: None,
        success: None,
        completeness: None,
    })
    .render()
    .expect("profile edit template renders")
}

#[test]
fn profile_page_shows_an_email_only_when_a_public_one_is_set() {
    let without = render_profile(profile_data(None));
    assert!(
        !without.contains("mailto:"),
        "no public email → no mailto link anywhere"
    );
    assert!(
        !without.contains("<dt>Email</dt>"),
        "no public email → no Email row"
    );
    assert!(
        without.contains("+49 30 0000"),
        "sanity: the details list did render (phone is shown on public profiles)"
    );

    let with = render_profile(profile_data(Some("pub@example.test")));
    assert!(with.contains(r#"href="mailto:pub@example.test""#));
    assert!(with.contains("<dt>Email</dt>"));
    assert!(
        with.contains(r#""email": "pub@example.test""#),
        "JSON-LD carries the public email"
    );
}

#[test]
fn edit_form_offers_a_clearly_marked_public_email_field() {
    let html = render_edit(profile_data(Some("pub@example.test")));
    assert!(html.contains(r#"name="public_email""#));
    assert!(html.contains(r#"type="email""#));
    assert!(
        html.contains(r#"value="pub@example.test""#),
        "current value is prefilled"
    );
    assert!(
        html.contains("(displayed publicly)"),
        "the label says so in as many words"
    );
    assert!(
        html.contains("sign-in email is never shown"),
        "help text explains the split"
    );

    let blank = render_edit(profile_data(None));
    assert!(blank.contains(r#"name="public_email""#));
    assert!(!blank.contains("pub@example.test"));
}
