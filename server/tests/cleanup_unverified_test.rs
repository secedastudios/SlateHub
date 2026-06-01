//! Regression test for `Person::cleanup_unverified`.
//!
//! The bug we're guarding against: the daily cleanup job was deleting
//! unverified accounts older than N days *without* checking whether they'd
//! started the paid identity-verification flow. A real paying customer got
//! garbage-collected mid-Stripe-Identity-session.
//!
//! Cleanup must skip anyone who has any `verification_payment` row (they've
//! put money on the line) or any `verification_request` row (they've asked
//! for manual review).

mod common;

use slatehub::db::DB;
use slatehub::models::person::Person;
use slatehub::record_id_ext::RecordIdExt;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(serde::Deserialize, SurrealValue)]
struct R {
    id: RecordId,
}

async fn create_unverified_old(username: &str, days_old: i64) -> RecordId {
    let sql = format!(
        "CREATE person CONTENT {{
            username: $username,
            email: $email,
            password: 'hashed',
            name: $username,
            verification_status: 'unverified',
            created_at: time::now() - {}d,
            profile: {{ name: $username, skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }}
        }} RETURN id",
        days_old
    );
    let rows: Vec<R> = DB
        .query(&sql)
        .bind(("username", username.to_string()))
        .bind(("email", format!("{}@cleanup.test", username)))
        .await
        .expect("create unverified")
        .take(0)
        .expect("take unverified");
    rows.into_iter().next().expect("one").id
}

async fn add_payment(person: &RecordId, status: &str) {
    DB.query(
        "CREATE verification_payment SET person = $pid, stripe_checkout_session_id = $sid, \
         amount_minor = 1000, currency = 'usd', status = $status",
    )
    .bind(("pid", person.clone()))
    .bind(("sid", format!("cs_test_{}", person.to_raw_string())))
    .bind(("status", status.to_string()))
    .await
    .expect("create payment");
}

async fn add_manual_request(person: &RecordId) {
    DB.query("CREATE verification_request SET person = $pid, status = 'pending'")
        .bind(("pid", person.clone()))
        .await
        .expect("create request");
}

async fn person_exists(rid: &RecordId) -> bool {
    let mut response = DB
        .query("SELECT count() AS c FROM person WHERE id = $id GROUP ALL")
        .bind(("id", rid.clone()))
        .await
        .expect("count person");
    #[derive(serde::Deserialize, SurrealValue)]
    struct C {
        c: i64,
    }
    let row: Option<C> = response.take(0).unwrap_or(None);
    row.map(|r| r.c > 0).unwrap_or(false)
}

fn clean() {
    common::clean_table("verification_payment");
    common::clean_table("verification_request");
    common::clean_table("notification");
    common::clean_table("person");
}

#[test]
fn test_cleanup_unverified_protects_payers_and_requesters() {
    common::setup_test_db();
    clean();

    common::run(async {
        // Three users, all unverified, all 10 days old:
        //   alice   — has a paid verification_payment row → MUST survive
        //   bob     — has a refunded verification_payment row → MUST survive
        //              (refunds are still evidence they were a real user)
        //   carol   — has a manual verification_request → MUST survive
        //   dave    — no payment, no request → SHOULD be deleted (real spam case)
        let alice = create_unverified_old("alice_cleanup", 10).await;
        let bob = create_unverified_old("bob_cleanup", 10).await;
        let carol = create_unverified_old("carol_cleanup", 10).await;
        let dave = create_unverified_old("dave_cleanup", 10).await;

        add_payment(&alice, "paid").await;
        add_payment(&bob, "refunded").await;
        add_manual_request(&carol).await;

        // Sanity: all four exist pre-cleanup.
        assert!(person_exists(&alice).await);
        assert!(person_exists(&bob).await);
        assert!(person_exists(&carol).await);
        assert!(person_exists(&dave).await);

        // Run the cleanup with a 5-day threshold (all four are 10 days old).
        Person::cleanup_unverified(5).await;

        // Post-conditions
        assert!(
            person_exists(&alice).await,
            "alice paid for verification — should NOT be deleted"
        );
        assert!(
            person_exists(&bob).await,
            "bob has a refunded payment — should NOT be deleted (real user)"
        );
        assert!(
            person_exists(&carol).await,
            "carol requested manual verification — should NOT be deleted"
        );
        assert!(
            !person_exists(&dave).await,
            "dave is unverified spam — SHOULD be deleted"
        );
    });
}

#[test]
fn test_cleanup_unverified_respects_threshold_age() {
    common::setup_test_db();
    clean();

    common::run(async {
        // Recent: 2 days old. Old: 10 days old. With threshold = 5d, only
        // the old one is eligible.
        let recent = create_unverified_old("recent_user", 2).await;
        let old = create_unverified_old("old_user", 10).await;

        Person::cleanup_unverified(5).await;

        assert!(person_exists(&recent).await, "recent user should survive");
        assert!(!person_exists(&old).await, "old user should be deleted");
    });
}
