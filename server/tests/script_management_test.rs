//! Integration tests for `ScriptModel` — the data layer behind the
//! production-management Script tab.
//!
//! Covers:
//!   - Auto-versioning per (production, title)
//!   - Per-production isolation of version sequences
//!   - `list_grouped_by_title`: empty case, single title, multi-title order,
//!     latest-vs-older split, version ordering within `older`, uploader fields
//!   - `update_visibility` round-trip
//!   - `delete` returning `file_key` for downstream S3 cleanup, row removed
//!   - Idempotent delete of an unknown id
//!
//! Per the project's testing convention (model/contract level, not HTTP),
//! every test exercises `ScriptModel` directly against a real test DB.

mod common;

use slatehub::db::DB;
use slatehub::models::script::ScriptModel;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(serde::Deserialize, SurrealValue)]
struct R {
    id: RecordId,
}

async fn seed_person(username: &str) -> RecordId {
    let rows: Vec<R> = DB
        .query(
            "CREATE person CONTENT {
                username: $username, email: $email, password: 'h', name: $username,
                profile: { name: $username, skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
            } RETURN id",
        )
        .bind(("username", username.to_string()))
        .bind(("email", format!("{}@sm.test", username)))
        .await
        .expect("create person")
        .take(0)
        .expect("take person");
    rows.into_iter().next().expect("one").id
}

async fn seed_production(slug: &str) -> RecordId {
    let rows: Vec<R> = DB
        .query(
            "CREATE production CONTENT {
                title: $slug, slug: $slug, type: 'Feature Film',
                status: 'in_development', source: 'manual'
            } RETURN id",
        )
        .bind(("slug", slug.to_string()))
        .await
        .expect("create production")
        .take(0)
        .expect("take production");
    rows.into_iter().next().expect("one").id
}

async fn upload(
    prod: &RecordId,
    uploader: &RecordId,
    title: &str,
    file_key: &str,
    visibility: &str,
    notes: Option<&str>,
) {
    ScriptModel::create(
        prod,
        title,
        &format!("/api/media/{file_key}"),
        file_key,
        2048,
        "application/pdf",
        visibility,
        uploader,
        notes,
    )
    .await
    .expect("create script");
}

fn clean() {
    common::clean_table("production_script");
    common::clean_table("production");
    common::clean_table("person");
}

// ---------------------------------------------------------------------------
// list_grouped_by_title — empty + happy paths
// ---------------------------------------------------------------------------

#[test]
fn list_grouped_is_empty_when_production_has_no_scripts() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-empty").await;
        let groups = ScriptModel::list_grouped_by_title(&prod)
            .await
            .expect("list ok");
        assert!(groups.is_empty(), "no scripts → no groups");
    });
}

#[test]
fn list_grouped_returns_single_group_with_no_older_for_one_upload() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-one").await;
        let alice = seed_person("sm_alice_one").await;
        upload(
            &prod,
            &alice,
            "Pilot",
            "scripts/pilot_v1.pdf",
            "members",
            None,
        )
        .await;

        let groups = ScriptModel::list_grouped_by_title(&prod)
            .await
            .expect("list ok");
        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert_eq!(g.title, "Pilot");
        assert_eq!(g.latest.version, 1);
        assert_eq!(g.latest.visibility, "members");
        assert!(g.older.is_empty(), "single upload → no older versions");
    });
}

#[test]
fn list_grouped_splits_latest_from_older_when_a_title_has_multiple_versions() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-multi").await;
        let alice = seed_person("sm_alice_multi").await;

        // Three versions of the same title.
        upload(
            &prod,
            &alice,
            "Pilot",
            "scripts/pilot_v1.pdf",
            "members",
            Some("first"),
        )
        .await;
        upload(
            &prod,
            &alice,
            "Pilot",
            "scripts/pilot_v2.pdf",
            "members",
            Some("second"),
        )
        .await;
        upload(
            &prod,
            &alice,
            "Pilot",
            "scripts/pilot_v3.pdf",
            "public",
            Some("third"),
        )
        .await;

        let groups = ScriptModel::list_grouped_by_title(&prod)
            .await
            .expect("list ok");
        assert_eq!(groups.len(), 1, "still one title group");
        let g = &groups[0];
        assert_eq!(g.latest.version, 3, "latest is highest version");
        assert_eq!(g.latest.visibility, "public");
        assert_eq!(g.older.len(), 2, "two older versions");

        // older is sorted DESC: v2, then v1
        assert_eq!(g.older[0].version, 2);
        assert_eq!(g.older[1].version, 1);
    });
}

#[test]
fn list_grouped_returns_multiple_titles_in_alphabetical_order() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-titles").await;
        let alice = seed_person("sm_alice_titles").await;

        upload(
            &prod,
            &alice,
            "Outline",
            "scripts/outline.pdf",
            "members",
            None,
        )
        .await;
        upload(&prod, &alice, "Bible", "scripts/bible.pdf", "members", None).await;
        upload(&prod, &alice, "Pilot", "scripts/pilot.pdf", "members", None).await;

        let groups = ScriptModel::list_grouped_by_title(&prod)
            .await
            .expect("list ok");
        let titles: Vec<&str> = groups.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Bible", "Outline", "Pilot"],
            "title groups are ordered alphabetically"
        );
    });
}

#[test]
fn list_grouped_populates_uploader_username_and_name() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-uploader").await;
        let alice = seed_person("sm_alice_uploader").await;

        upload(&prod, &alice, "Pilot", "scripts/pilot.pdf", "members", None).await;

        let groups = ScriptModel::list_grouped_by_title(&prod)
            .await
            .expect("list ok");
        let v = &groups[0].latest;
        assert_eq!(
            v.uploader_username.as_deref(),
            Some("sm_alice_uploader"),
            "uploader username should be followed via the record link"
        );
        assert_eq!(
            v.uploader_name.as_deref(),
            Some("sm_alice_uploader"),
            "uploader name should be followed via the record link"
        );
    });
}

// ---------------------------------------------------------------------------
// Auto-versioning + per-production isolation
// ---------------------------------------------------------------------------

#[test]
fn create_auto_increments_version_per_title() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-incr").await;
        let alice = seed_person("sm_alice_incr").await;

        for i in 1..=4 {
            upload(
                &prod,
                &alice,
                "Pilot",
                &format!("scripts/pilot_v{i}.pdf"),
                "members",
                None,
            )
            .await;
        }
        let versions = ScriptModel::get_versions(&prod, "Pilot")
            .await
            .expect("versions ok");
        let nums: Vec<i64> = versions.iter().map(|v| v.version).collect();
        assert_eq!(nums, vec![4, 3, 2, 1], "DESC and contiguous");
    });
}

#[test]
fn version_sequences_are_isolated_per_production_and_per_title() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod_a = seed_production("sm-iso-a").await;
        let prod_b = seed_production("sm-iso-b").await;
        let alice = seed_person("sm_alice_iso").await;

        // Two versions of "Pilot" on A
        upload(
            &prod_a,
            &alice,
            "Pilot",
            "scripts/a_pilot_v1.pdf",
            "members",
            None,
        )
        .await;
        upload(
            &prod_a,
            &alice,
            "Pilot",
            "scripts/a_pilot_v2.pdf",
            "members",
            None,
        )
        .await;
        // One version of "Pilot" on B (must start at v1, NOT v3)
        upload(
            &prod_b,
            &alice,
            "Pilot",
            "scripts/b_pilot_v1.pdf",
            "members",
            None,
        )
        .await;
        // And a different title on A — also starts at v1
        upload(
            &prod_a,
            &alice,
            "Outline",
            "scripts/a_outline_v1.pdf",
            "members",
            None,
        )
        .await;

        let a_pilot = ScriptModel::get_versions(&prod_a, "Pilot")
            .await
            .expect("versions ok");
        let b_pilot = ScriptModel::get_versions(&prod_b, "Pilot")
            .await
            .expect("versions ok");
        let a_outline = ScriptModel::get_versions(&prod_a, "Outline")
            .await
            .expect("versions ok");

        assert_eq!(
            a_pilot.iter().map(|v| v.version).collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert_eq!(b_pilot.len(), 1, "production B has its own sequence");
        assert_eq!(b_pilot[0].version, 1, "starts at v1 on production B");
        assert_eq!(
            a_outline[0].version, 1,
            "different title on the same production starts at v1"
        );
    });
}

// ---------------------------------------------------------------------------
// update_visibility + delete
// ---------------------------------------------------------------------------

#[test]
fn update_visibility_flips_members_to_public_and_back() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-vis").await;
        let alice = seed_person("sm_alice_vis").await;
        upload(&prod, &alice, "Pilot", "scripts/pilot.pdf", "members", None).await;

        let scripts = ScriptModel::get_latest_for_production(&prod)
            .await
            .expect("get latest ok");
        let id = scripts.into_iter().next().expect("one").id;

        ScriptModel::update_visibility(&id, "public")
            .await
            .expect("update ok");
        let after = ScriptModel::get(&id)
            .await
            .expect("get ok")
            .expect("exists");
        assert_eq!(after.visibility, "public");

        ScriptModel::update_visibility(&id, "members")
            .await
            .expect("update ok");
        let after = ScriptModel::get(&id)
            .await
            .expect("get ok")
            .expect("exists");
        assert_eq!(after.visibility, "members");
    });
}

#[test]
fn delete_returns_file_key_and_removes_the_row() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-del").await;
        let alice = seed_person("sm_alice_del").await;
        upload(
            &prod,
            &alice,
            "Pilot",
            "scripts/pilot_to_delete.pdf",
            "members",
            None,
        )
        .await;

        let scripts = ScriptModel::get_latest_for_production(&prod)
            .await
            .expect("get latest ok");
        let id = scripts.into_iter().next().expect("one").id;

        let file_key = ScriptModel::delete(&id).await.expect("delete ok");
        assert_eq!(
            file_key.as_deref(),
            Some("scripts/pilot_to_delete.pdf"),
            "delete returns the file_key for S3 cleanup"
        );

        let after = ScriptModel::get(&id).await.expect("get ok");
        assert!(after.is_none(), "row is gone after delete");
    });
}

#[test]
fn delete_of_unknown_id_returns_none_without_error() {
    common::setup_test_db();
    clean();
    common::run(async {
        let bogus = RecordId::new("production_script", "does_not_exist");
        let file_key = ScriptModel::delete(&bogus).await.expect("must not error");
        assert!(
            file_key.is_none(),
            "deleting a non-existent script returns Ok(None)"
        );
    });
}

// ---------------------------------------------------------------------------
// list_grouped_by_title against legacy get_latest_for_production
// ---------------------------------------------------------------------------

#[test]
fn list_grouped_latest_matches_get_latest_for_production() {
    // Both APIs should agree on which row is the "latest" per title.
    // list_grouped_by_title is the new path; get_latest_for_production is
    // what the public production page still uses. Cross-check them so they
    // can't silently diverge.
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-parity").await;
        let alice = seed_person("sm_alice_parity").await;

        upload(
            &prod,
            &alice,
            "Pilot",
            "scripts/pilot_v1.pdf",
            "members",
            None,
        )
        .await;
        upload(
            &prod,
            &alice,
            "Pilot",
            "scripts/pilot_v2.pdf",
            "members",
            None,
        )
        .await;
        upload(
            &prod,
            &alice,
            "Outline",
            "scripts/outline.pdf",
            "members",
            None,
        )
        .await;

        let groups = ScriptModel::list_grouped_by_title(&prod)
            .await
            .expect("grouped ok");
        let latest = ScriptModel::get_latest_for_production(&prod)
            .await
            .expect("latest ok");

        // Build (title -> version) from both APIs and compare.
        let from_groups: std::collections::HashMap<String, i64> = groups
            .iter()
            .map(|g| (g.title.clone(), g.latest.version))
            .collect();
        let from_latest: std::collections::HashMap<String, i64> = latest
            .iter()
            .map(|s| (s.title.clone(), s.version))
            .collect();
        assert_eq!(
            from_groups, from_latest,
            "both APIs must agree on latest version per title"
        );
    });
}

// ---------------------------------------------------------------------------
// resolve_upload_title — title-less uploads version into the existing chain
// ---------------------------------------------------------------------------

#[test]
fn resolve_upload_title_falls_back_to_production_title_for_first_upload() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-resolve-empty").await;
        let title = ScriptModel::resolve_upload_title(&prod, "My Feature Film")
            .await
            .expect("resolve ok");
        assert_eq!(title, "My Feature Film");
    });
}

#[test]
fn resolve_upload_title_continues_the_latest_chain_even_after_rename() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = seed_production("sm-resolve-chain").await;
        let alice = seed_person("sm_alice_resolve").await;

        // First upload started the chain under the then-current title.
        upload(&prod, &alice, "Original Title", "s/v1.pdf", "members", None).await;

        // Production has since been renamed — uploads must still continue
        // the existing document's chain, not start a parallel one.
        let title = ScriptModel::resolve_upload_title(&prod, "Renamed Production")
            .await
            .expect("resolve ok");
        assert_eq!(title, "Original Title");
    });
}
