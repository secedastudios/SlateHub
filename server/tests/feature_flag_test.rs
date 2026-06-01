//! Integration test for the feature_flag service.
//!
//! Verifies the access matrix for the four states (off / admin_only /
//! verified / all) across the three user types (anonymous, plain user,
//! admin) plus a verified-but-not-admin variant.

mod common;

use slatehub::db::DB;
use slatehub::models::person::SessionUser;
use slatehub::services::feature_flag::{self, FlagState};
use surrealdb::types::{RecordId, SurrealValue};

const TEST_FLAG: &str = "test_flag_for_unit_test";

async fn ensure_test_flag() {
    // Use a non-registry key — that means set_state would reject it. We
    // bypass by writing directly. This keeps the test isolated from
    // whatever real flags are in FLAG_REGISTRY.
    DB.query(
        "UPSERT feature_flag CONTENT { key: $key, name: 'Test Flag', description: NONE, state: 'off' } \
         WHERE key = $key",
    )
    .bind(("key", TEST_FLAG.to_string()))
    .await
    .ok();
    // UPSERT semantics in SurrealDB 3 require WHERE on the target; simpler
    // approach: DELETE then CREATE.
    DB.query("DELETE feature_flag WHERE key = $key")
        .bind(("key", TEST_FLAG.to_string()))
        .await
        .ok();
    DB.query(
        "CREATE feature_flag SET key = $key, name = 'Test Flag', description = NONE, state = 'off'",
    )
    .bind(("key", TEST_FLAG.to_string()))
    .await
    .expect("create test flag");
}

async fn set_test_flag(state: &str) {
    DB.query("UPDATE feature_flag SET state = $state WHERE key = $key")
        .bind(("key", TEST_FLAG.to_string()))
        .bind(("state", state.to_string()))
        .await
        .expect("update flag state");
}

async fn create_user(
    username: &str,
    email: &str,
    is_admin: bool,
    verification_status: &str,
) -> SessionUser {
    #[derive(serde::Deserialize, SurrealValue)]
    struct R {
        id: RecordId,
    }
    let rows: Vec<R> = DB
        .query(
            "CREATE person CONTENT {
                username: $username,
                email: $email,
                password: 'hashed',
                name: $username,
                is_admin: $is_admin,
                verification_status: $vs,
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
    use slatehub::record_id_ext::RecordIdExt;
    SessionUser {
        id: id.to_raw_string(),
        username: username.to_string(),
        email: email.to_string(),
        name: username.to_string(),
    }
}

fn clean() {
    common::clean_table("feature_flag");
    common::clean_table("person");
}

#[test]
fn test_feature_flag_access_matrix() {
    common::setup_test_db();
    clean();

    common::run(async {
        ensure_test_flag().await;

        let admin = create_user("ff_admin", "a@ff.test", true, "identity").await;
        let verified_non_admin = create_user("ff_ver", "v@ff.test", false, "identity").await;
        let unverified = create_user("ff_unv", "u@ff.test", false, "unverified").await;

        // -- state: off --
        set_test_flag("off").await;
        assert!(
            !feature_flag::allows(TEST_FLAG, None).await,
            "off → anon denied"
        );
        assert!(
            !feature_flag::allows(TEST_FLAG, Some(&admin)).await,
            "off → admin denied"
        );
        assert!(
            !feature_flag::allows(TEST_FLAG, Some(&verified_non_admin)).await,
            "off → verified denied"
        );
        assert!(
            !feature_flag::allows(TEST_FLAG, Some(&unverified)).await,
            "off → unverified denied"
        );

        // -- state: admin_only --
        set_test_flag("admin_only").await;
        assert!(
            !feature_flag::allows(TEST_FLAG, None).await,
            "admin_only → anon denied"
        );
        assert!(
            feature_flag::allows(TEST_FLAG, Some(&admin)).await,
            "admin_only → admin allowed"
        );
        assert!(
            !feature_flag::allows(TEST_FLAG, Some(&verified_non_admin)).await,
            "admin_only → verified-non-admin denied"
        );
        assert!(
            !feature_flag::allows(TEST_FLAG, Some(&unverified)).await,
            "admin_only → unverified denied"
        );

        // -- state: verified (means verification_status == 'identity') --
        set_test_flag("verified").await;
        assert!(
            !feature_flag::allows(TEST_FLAG, None).await,
            "verified → anon denied"
        );
        assert!(
            feature_flag::allows(TEST_FLAG, Some(&admin)).await,
            "verified → admin (who is also identity) allowed"
        );
        assert!(
            feature_flag::allows(TEST_FLAG, Some(&verified_non_admin)).await,
            "verified → verified-non-admin allowed"
        );
        assert!(
            !feature_flag::allows(TEST_FLAG, Some(&unverified)).await,
            "verified → unverified denied"
        );

        // -- state: all --
        set_test_flag("all").await;
        assert!(
            feature_flag::allows(TEST_FLAG, None).await,
            "all → anon allowed"
        );
        assert!(
            feature_flag::allows(TEST_FLAG, Some(&admin)).await,
            "all → admin allowed"
        );
        assert!(
            feature_flag::allows(TEST_FLAG, Some(&verified_non_admin)).await,
            "all → verified allowed"
        );
        assert!(
            feature_flag::allows(TEST_FLAG, Some(&unverified)).await,
            "all → unverified allowed"
        );
    });
}

#[test]
fn test_unknown_flag_defaults_to_off() {
    common::setup_test_db();
    clean();

    common::run(async {
        // No row exists for "nonexistent_flag" → get_state returns Off → allows
        // returns false for every user.
        assert_eq!(
            feature_flag::get_state("nonexistent_flag").await,
            FlagState::Off
        );
        assert!(!feature_flag::allows("nonexistent_flag", None).await);
    });
}
