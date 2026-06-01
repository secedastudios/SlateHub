//! Tests for the DB-level lookup that drives `resume_verification`.
//!
//! The full handler makes Stripe API calls which can't be exercised without
//! either a real Stripe sandbox or a mocking layer (we have neither in this
//! repo). What we CAN test cleanly:
//!
//!   1. The "latest paid payment for this person" query returns the right
//!      row across various payment-state scenarios.
//!   2. `Person::find_by_record_id` round-trips correctly — handlers rely
//!      on it to load the user before deciding whether to resume.
//!
//! Anything Stripe-side is a separate end-to-end concern; this test pins
//! down the parts that we own.

mod common;

use slatehub::db::DB;
use slatehub::models::person::Person;
use slatehub::record_id_ext::RecordIdExt;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(serde::Deserialize, SurrealValue)]
struct R {
    id: RecordId,
}

async fn create_person(username: &str) -> RecordId {
    let rows: Vec<R> = DB
        .query(
            "CREATE person CONTENT {
                username: $username,
                email: $email,
                password: 'hashed',
                name: $username,
                verification_status: 'unverified',
                profile: { name: $username, skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
            } RETURN id",
        )
        .bind(("username", username.to_string()))
        .bind(("email", format!("{}@resume.test", username)))
        .await
        .expect("create")
        .take(0)
        .expect("take");
    rows.into_iter().next().expect("one").id
}

async fn create_payment(
    person: &RecordId,
    checkout_id: &str,
    identity_id: Option<&str>,
    status: &str,
    seconds_ago: i64,
) -> RecordId {
    let sql = format!(
        "CREATE verification_payment CONTENT {{
            person: $pid,
            stripe_checkout_session_id: $sid,
            stripe_identity_session_id: $isid,
            amount_minor: 1000,
            currency: 'usd',
            status: $status,
            created_at: time::now() - {}s
        }} RETURN id",
        seconds_ago
    );
    let rows: Vec<R> = DB
        .query(&sql)
        .bind(("pid", person.clone()))
        .bind(("sid", checkout_id.to_string()))
        .bind(("isid", identity_id.map(String::from)))
        .bind(("status", status.to_string()))
        .await
        .expect("create payment")
        .take(0)
        .expect("take payment");
    rows.into_iter().next().expect("one").id
}

#[derive(Debug, serde::Deserialize, SurrealValue, PartialEq)]
struct LookupRow {
    id: RecordId,
    stripe_identity_session_id: Option<String>,
    // v3 ORDER BY must appear in SELECT list — pulled in but unused.
    #[allow(dead_code)]
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn latest_paid_lookup(person: &RecordId) -> Option<LookupRow> {
    let mut response = DB
        .query(
            "SELECT id, stripe_identity_session_id, created_at FROM verification_payment \
             WHERE person = $pid AND status = 'paid' \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(("pid", person.clone()))
        .await
        .expect("query");
    response.take(0).expect("take")
}

fn clean() {
    common::clean_table("verification_payment");
    common::clean_table("person");
}

#[test]
fn lookup_returns_none_when_no_paid_row_exists() {
    common::setup_test_db();
    clean();

    common::run(async {
        let alice = create_person("alice_resume").await;
        // Only a pending row — not paid.
        create_payment(&alice, "cs_test_pending", None, "pending", 0).await;

        assert!(latest_paid_lookup(&alice).await.is_none());
    });
}

#[test]
fn lookup_skips_verified_and_refunded_returns_only_paid() {
    common::setup_test_db();
    clean();

    common::run(async {
        let alice = create_person("alice_paid").await;
        create_payment(&alice, "cs_verified", Some("vs_done"), "verified", 200).await;
        create_payment(&alice, "cs_refunded", Some("vs_canceled"), "refunded", 100).await;
        let paid = create_payment(&alice, "cs_paid", Some("vs_active"), "paid", 50).await;

        let row = latest_paid_lookup(&alice).await.expect("found");
        assert_eq!(
            row.id, paid,
            "should return only the row with status='paid'"
        );
        assert_eq!(row.stripe_identity_session_id.as_deref(), Some("vs_active"));
    });
}

#[test]
fn lookup_returns_most_recent_paid_when_multiple_exist() {
    common::setup_test_db();
    clean();

    common::run(async {
        let alice = create_person("alice_multi").await;
        // Two paid rows — order by created_at DESC, newest wins.
        create_payment(&alice, "cs_old", Some("vs_old"), "paid", 300).await;
        let new = create_payment(&alice, "cs_new", Some("vs_new"), "paid", 30).await;

        let row = latest_paid_lookup(&alice).await.expect("found");
        assert_eq!(row.id, new);
        assert_eq!(row.stripe_identity_session_id.as_deref(), Some("vs_new"));
    });
}

#[test]
fn lookup_finds_paid_with_no_identity_session_yet() {
    // The transient-failure case: payment captured but Identity session
    // creation failed. resume_verification should be able to find the row
    // and create a new Identity session against it.
    common::setup_test_db();
    clean();

    common::run(async {
        let alice = create_person("alice_orphan").await;
        let payment = create_payment(&alice, "cs_orphan", None, "paid", 0).await;

        let row = latest_paid_lookup(&alice).await.expect("found");
        assert_eq!(row.id, payment);
        assert!(
            row.stripe_identity_session_id.is_none(),
            "transient-failure case: no Identity session yet"
        );
    });
}

#[test]
fn lookup_ignores_other_persons_payments() {
    common::setup_test_db();
    clean();

    common::run(async {
        let alice = create_person("alice_priv").await;
        let bob = create_person("bob_priv").await;

        create_payment(&bob, "cs_bob", Some("vs_bob"), "paid", 0).await;

        assert!(
            latest_paid_lookup(&alice).await.is_none(),
            "alice must not see bob's payment"
        );
    });
}

#[test]
fn find_by_record_id_round_trips() {
    common::setup_test_db();
    clean();

    common::run(async {
        let alice = create_person("alice_findrid").await;

        let found = Person::find_by_record_id(&alice)
            .await
            .expect("query ok")
            .expect("found");
        assert_eq!(found.id.to_raw_string(), alice.to_raw_string());
        assert_eq!(found.username, "alice_findrid");
    });
}

#[test]
fn find_by_record_id_returns_none_for_missing_person() {
    common::setup_test_db();
    clean();

    common::run(async {
        let phantom = RecordId::new("person", "definitely_not_a_real_key");
        let found = Person::find_by_record_id(&phantom).await.expect("query ok");
        assert!(found.is_none());
    });
}
