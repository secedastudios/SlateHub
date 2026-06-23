//! Access-gate tests for the production-management workspace.
//!
//! The route gate in `routes/productions_manage.rs::require_member` is a
//! straight conjunction of two checks:
//!
//!   1. `feature_flag::allows("production_management", user)`
//!   2. `ProductionModel::get_role(prod_id, user_id)` returns `Some(_)`
//!
//! Step 1 is exhaustively tested in `tests/feature_flag_test.rs`
//! (the four-state × user-type matrix). This file pins down step 2 —
//! the `get_role` contract — plus the combined `is_member_of` helper
//! used by the public production page to decide whether to render the
//! "Manage" toggle.
//!
//! Per the project's testing convention (model/contract level, not HTTP),
//! these tests exercise the same functions the route handler calls.

mod common;

use slatehub::db::DB;
use slatehub::models::person::SessionUser;
use slatehub::models::production::ProductionModel;
use slatehub::record_id_ext::RecordIdExt;
use slatehub::services::feature_flag::FlagState;
use surrealdb::types::{RecordId, SurrealValue};

const FLAG_KEY: &str = "production_management";

#[derive(serde::Deserialize, SurrealValue)]
struct R {
    id: RecordId,
}

async fn create_person(
    username: &str,
    email: &str,
    is_admin: bool,
    verification_status: &str,
) -> SessionUser {
    let rows: Vec<R> = DB
        .query(
            "CREATE person CONTENT {
                username: $username, email: $email, password: 'h', name: $username,
                is_admin: $is_admin, verification_status: $vs,
                profile: { name: $username, skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
            } RETURN id",
        )
        .bind(("username", username.to_string()))
        .bind(("email", email.to_string()))
        .bind(("is_admin", is_admin))
        .bind(("vs", verification_status.to_string()))
        .await
        .expect("create person")
        .take(0)
        .expect("take person");
    let id = rows.into_iter().next().expect("one").id;
    SessionUser {
        id: id.to_raw_string(),
        username: username.to_string(),
        email: email.to_string(),
        name: username.to_string(),
    }
}

async fn create_production(slug: &str) -> RecordId {
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

async fn relate_member(person_id: &str, prod: &RecordId, role: &str) {
    // RELATE needs the literal record ids in the query string in v3
    // (binding doesn't work for graph traversal targets — see
    // feedback-record-id-handling memory note).
    let q = format!(
        "RELATE {}->member_of->{} SET role = $role, invitation_status = 'accepted'",
        person_id,
        prod.to_raw_string()
    );
    DB.query(&q)
        .bind(("role", role.to_string()))
        .await
        .expect("relate member");
}

async fn ensure_flag(state: FlagState) {
    // Seed the production_management row (idempotent via DELETE + CREATE)
    // so each test starts from a known state regardless of what the
    // session's bootstrap registered.
    DB.query("DELETE feature_flag WHERE key = $key")
        .bind(("key", FLAG_KEY.to_string()))
        .await
        .ok();
    DB.query(
        "CREATE feature_flag SET key = $key, name = 'Production Management', \
         description = 'test', state = $state",
    )
    .bind(("key", FLAG_KEY.to_string()))
    .bind(("state", state.as_str().to_string()))
    .await
    .expect("seed flag");
}

fn clean() {
    common::clean_table("member_of");
    common::clean_table("production");
    common::clean_table("person");
    common::clean_table("feature_flag");
}

// ---------------------------------------------------------------------------
// get_role contract — the per-production membership lookup
// ---------------------------------------------------------------------------

#[test]
fn get_role_returns_owner_when_user_is_owner() {
    common::setup_test_db();
    clean();
    common::run(async {
        let user = create_person("ma_owner", "owner@m.test", false, "identity").await;
        let prod = create_production("ma-owned").await;
        relate_member(&user.id, &prod, "owner").await;

        let role = ProductionModel::get_role(&prod, &user.id)
            .await
            .expect("query ok");
        assert_eq!(role.as_deref(), Some("owner"));
    });
}

#[test]
fn get_role_returns_member_when_user_is_plain_member() {
    common::setup_test_db();
    clean();
    common::run(async {
        let user = create_person("ma_member", "member@m.test", false, "identity").await;
        let prod = create_production("ma-membered").await;
        relate_member(&user.id, &prod, "member").await;

        let role = ProductionModel::get_role(&prod, &user.id)
            .await
            .expect("query ok");
        assert_eq!(role.as_deref(), Some("member"));
    });
}

#[test]
fn get_role_returns_none_when_user_is_not_a_member() {
    common::setup_test_db();
    clean();
    common::run(async {
        let user = create_person("ma_nope", "nope@m.test", false, "identity").await;
        let prod = create_production("ma-nope").await;
        // No relate — user has no edge to the production.

        let role = ProductionModel::get_role(&prod, &user.id)
            .await
            .expect("query ok");
        assert!(role.is_none(), "non-member must have no role");
    });
}

// ---------------------------------------------------------------------------
// Combined access matrix: feature flag × membership
// ---------------------------------------------------------------------------
//
// is_member_of(prod, user) == feature_flag::allows("production_management", user)
//                          && get_role(prod, user).is_some()
//
// We test every meaningful combination.

use slatehub::routes::productions_manage as helpers;

#[test]
fn flag_off_denies_everyone_even_admin_member() {
    common::setup_test_db();
    clean();
    common::run(async {
        ensure_flag(FlagState::Off).await;
        let admin = create_person("ma_adm_off", "a@off.test", true, "identity").await;
        let prod = create_production("flag-off-prod").await;
        relate_member(&admin.id, &prod, "owner").await;

        let allowed = helpers::is_member_of(&prod, &admin).await;
        assert!(!allowed, "flag=off must block even admin owners");
    });
}

#[test]
fn flag_admin_only_allows_admin_member() {
    common::setup_test_db();
    clean();
    common::run(async {
        ensure_flag(FlagState::AdminOnly).await;
        let admin = create_person("ma_adm", "a@admin.test", true, "identity").await;
        let prod = create_production("admin-only-prod").await;
        relate_member(&admin.id, &prod, "owner").await;

        let allowed = helpers::is_member_of(&prod, &admin).await;
        assert!(
            allowed,
            "admin owner with flag=admin_only should be allowed"
        );
    });
}

#[test]
fn flag_admin_only_blocks_non_admin_even_when_owner() {
    common::setup_test_db();
    clean();
    common::run(async {
        ensure_flag(FlagState::AdminOnly).await;
        let user = create_person("ma_nadm", "n@a.test", false, "identity").await;
        let prod = create_production("admin-only-block").await;
        relate_member(&user.id, &prod, "owner").await;

        let allowed = helpers::is_member_of(&prod, &user).await;
        assert!(
            !allowed,
            "non-admin must be blocked when flag is admin_only, even if owner"
        );
    });
}

#[test]
fn flag_admin_only_blocks_admin_who_is_not_a_member() {
    common::setup_test_db();
    clean();
    common::run(async {
        ensure_flag(FlagState::AdminOnly).await;
        let admin = create_person("ma_adm_nm", "a@nm.test", true, "identity").await;
        let _prod = create_production("admin-but-not-member").await;
        // No relate — admin user is not on this production.

        let prod_id = surrealdb::types::RecordId::new("production", "doesnt-matter-for-this-test");
        // We pass an unrelated production id; admin has no member_of edge to it.
        let allowed = helpers::is_member_of(&prod_id, &admin).await;
        assert!(
            !allowed,
            "admin who isn't a member of the production must be blocked"
        );
    });
}

#[test]
fn flag_verified_allows_verified_member() {
    common::setup_test_db();
    clean();
    common::run(async {
        ensure_flag(FlagState::Verified).await;
        let user = create_person("ma_ver", "v@m.test", false, "identity").await;
        let prod = create_production("verified-prod").await;
        relate_member(&user.id, &prod, "member").await;

        let allowed = helpers::is_member_of(&prod, &user).await;
        assert!(allowed, "verified plain member should be allowed");
    });
}

#[test]
fn flag_verified_blocks_unverified_member() {
    common::setup_test_db();
    clean();
    common::run(async {
        ensure_flag(FlagState::Verified).await;
        let user = create_person("ma_unv", "u@m.test", false, "unverified").await;
        let prod = create_production("verified-blocks-unv").await;
        relate_member(&user.id, &prod, "owner").await;

        let allowed = helpers::is_member_of(&prod, &user).await;
        assert!(
            !allowed,
            "unverified user blocked by flag=verified even as owner"
        );
    });
}

#[test]
fn flag_all_allows_any_member_regardless_of_admin_or_verification() {
    common::setup_test_db();
    clean();
    common::run(async {
        ensure_flag(FlagState::All).await;
        let user = create_person("ma_all", "a@all.test", false, "unverified").await;
        let prod = create_production("flag-all-prod").await;
        relate_member(&user.id, &prod, "member").await;

        let allowed = helpers::is_member_of(&prod, &user).await;
        assert!(
            allowed,
            "flag=all should allow any member regardless of admin/verification"
        );
    });
}

#[test]
fn flag_all_still_blocks_non_members() {
    // Even with the most permissive flag, you must be a member of THIS
    // production to manage it. Layer 2 (membership) is independent.
    common::setup_test_db();
    clean();
    common::run(async {
        ensure_flag(FlagState::All).await;
        let user = create_person("ma_all_nm", "anm@all.test", false, "identity").await;
        let prod = create_production("flag-all-nonmember").await;
        // No relate.

        let allowed = helpers::is_member_of(&prod, &user).await;
        assert!(
            !allowed,
            "flag=all does NOT bypass the per-production membership requirement"
        );
    });
}

// ---------------------------------------------------------------------------
// Overview dashboard stats — validates the v3.1 aggregate query patterns
// (count() GROUP ALL, array::distinct under GROUP ALL, record-link WHERE)
// against a real database.
// ---------------------------------------------------------------------------

#[test]
fn dashboard_stats_are_all_zero_for_untouched_production() {
    common::setup_test_db();
    clean();
    common::run(async {
        let prod = create_production("stats-empty").await;
        let stats = ProductionModel::manage_dashboard_stats(&prod)
            .await
            .expect("stats query ok");
        assert_eq!(stats.script_revisions, 0);
        assert_eq!(stats.script_documents, 0);
        assert!(stats.latest_script_title.is_none());
        assert_eq!(stats.scenes, 0);
        assert_eq!(stats.cast, 0);
        assert_eq!(stats.props, 0);
        assert_eq!(stats.locations, 0);
        assert_eq!(stats.shoot_days, 0);
        assert_eq!(stats.call_sheets, 0);
    });
}

#[test]
fn dashboard_stats_count_scripts_scenes_and_unique_breakdown_values() {
    common::setup_test_db();
    clean();
    common::clean_table("production_script");
    common::clean_table("scene");
    common::clean_table("breakdown_item");
    common::run(async {
        let user = create_person("stats_owner", "stats@m.test", false, "identity").await;
        let prod = create_production("stats-full").await;
        let uploader = user.record_id().expect("record id");

        // Script stage: 2 revisions of "Pilot" + 1 of "Bible" = 3 revisions, 2 docs.
        for (title, key) in [
            ("Pilot", "s/p1.pdf"),
            ("Pilot", "s/p2.pdf"),
            ("Bible", "s/b1.pdf"),
        ] {
            slatehub::models::script::ScriptModel::create(
                &prod,
                title,
                &format!("/api/media/{key}"),
                key,
                512,
                "application/pdf",
                "members",
                &uploader,
                None,
            )
            .await
            .expect("create script");
        }

        // Breakdown stage: 2 scenes; MAYA appears in both (unique cast = 2),
        // 3 prop rows with one duplicate value (unique props = 2),
        // 2 distinct scene locations.
        let mut scene_ids = Vec::new();
        for (num, loc) in [("1", "WAREHOUSE"), ("2", "ROOFTOP")] {
            let rows: Vec<R> = DB
                .query(
                    "CREATE scene CONTENT { production: $p, scene_number: $n,
                      heading: $h, location: $l, status: 'auto' } RETURN id",
                )
                .bind(("p", prod.clone()))
                .bind(("n", num.to_string()))
                .bind(("h", format!("INT. {loc} - DAY")))
                .bind(("l", loc.to_string()))
                .await
                .expect("create scene")
                .take(0)
                .expect("take scene");
            scene_ids.push(rows.into_iter().next().expect("one").id);
        }
        for (scene_idx, category, value) in [
            (0, "cast", "MAYA"),
            (0, "cast", "VIKTOR"),
            (1, "cast", "MAYA"),
            (0, "prop", "Revolver"),
            (0, "prop", "Briefcase"),
            (1, "prop", "Revolver"),
        ] {
            DB.query(
                "CREATE breakdown_item CONTENT { scene: $s, category: $c,
                  value: $v, source: 'auto' }",
            )
            .bind(("s", scene_ids[scene_idx].clone()))
            .bind(("c", category.to_string()))
            .bind(("v", value.to_string()))
            .await
            .expect("create breakdown item");
        }

        let stats = ProductionModel::manage_dashboard_stats(&prod)
            .await
            .expect("stats query ok");
        assert_eq!(stats.script_revisions, 3, "3 uploads total");
        assert_eq!(stats.script_documents, 2, "Pilot + Bible");
        assert_eq!(stats.latest_script_title.as_deref(), Some("Bible"));
        assert_eq!(stats.scenes, 2);
        assert_eq!(stats.cast, 2, "MAYA deduped across scenes");
        assert_eq!(stats.props, 2, "Revolver deduped across scenes");
        assert_eq!(stats.locations, 2, "WAREHOUSE + ROOFTOP");
        assert_eq!(stats.shoot_days, 0, "scheduling untouched");
        assert_eq!(stats.call_sheets, 0);
    });
}
