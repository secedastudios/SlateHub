//! Integration tests for the SQL contracts inside the Stripe webhook
//! handlers. We don't dispatch through the HTTP layer (would need signed
//! bodies + a full router); instead we seed `verification_payment` rows,
//! run the *same* UPDATE statements the handlers run, and assert the
//! state transitions are correct. If the handler SQL changes, the test
//! must follow.
//!
//! Covers:
//!   - `identity.verification_session.processing` — only flips `paid` → `processing`
//!   - `charge.refunded` — matches by `payment_intent`, captures `refund_id`,
//!     skips already-refunded rows

mod common;

use slatehub::db::DB;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(serde::Deserialize, SurrealValue)]
struct R {
    id: RecordId,
}

async fn seed_person(username: &str) -> RecordId {
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
        .bind(("email", format!("{}@wh.test", username)))
        .await
        .expect("create person")
        .take(0)
        .expect("take person");
    rows.into_iter().next().expect("one").id
}

async fn seed_payment(
    person: &RecordId,
    checkout_id: &str,
    payment_intent: Option<&str>,
    identity_session: Option<&str>,
    status: &str,
) -> RecordId {
    let rows: Vec<R> = DB
        .query(
            "CREATE verification_payment CONTENT {
                person: $pid,
                stripe_checkout_session_id: $sid,
                stripe_payment_intent_id: $pi,
                stripe_identity_session_id: $isid,
                amount_minor: 1000,
                currency: 'usd',
                status: $status
            } RETURN id",
        )
        .bind(("pid", person.clone()))
        .bind(("sid", checkout_id.to_string()))
        .bind(("pi", payment_intent.map(String::from)))
        .bind(("isid", identity_session.map(String::from)))
        .bind(("status", status.to_string()))
        .await
        .expect("create payment")
        .take(0)
        .expect("take payment");
    rows.into_iter().next().expect("one").id
}

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct StatusRow {
    status: String,
    refund_id: Option<String>,
}

async fn fetch_status(row_id: &RecordId) -> StatusRow {
    let mut response = DB
        .query("SELECT status, refund_id FROM $id")
        .bind(("id", row_id.clone()))
        .await
        .expect("select status");
    response
        .take::<Option<StatusRow>>(0)
        .expect("take status")
        .expect("row exists")
}

fn clean() {
    common::clean_table("verification_payment");
    common::clean_table("person");
}

// ---------------------------------------------------------------------------
// identity.verification_session.processing — flips paid → processing
// ---------------------------------------------------------------------------

#[test]
fn processing_handler_flips_paid_to_processing() {
    common::setup_test_db();
    clean();

    common::run(async {
        let person = seed_person("alice_proc").await;
        let row = seed_payment(
            &person,
            "cs_alice",
            Some("pi_alice"),
            Some("vs_alice"),
            "paid",
        )
        .await;

        // Same SQL as on_identity_processing()
        DB.query(
            "UPDATE verification_payment SET status = 'processing', updated_at = time::now() \
             WHERE stripe_identity_session_id = $isid AND status IN ['paid']",
        )
        .bind(("isid", "vs_alice".to_string()))
        .await
        .expect("processing update");

        let after = fetch_status(&row).await;
        assert_eq!(after.status, "processing");
    });
}

#[test]
fn processing_handler_does_not_demote_verified_row() {
    // A late-arriving `processing` event after a row was already `verified`
    // must be a no-op — never lose the verified status.
    common::setup_test_db();
    clean();

    common::run(async {
        let person = seed_person("bob_proc").await;
        let row = seed_payment(
            &person,
            "cs_bob",
            Some("pi_bob"),
            Some("vs_bob"),
            "verified",
        )
        .await;

        DB.query(
            "UPDATE verification_payment SET status = 'processing', updated_at = time::now() \
             WHERE stripe_identity_session_id = $isid AND status IN ['paid']",
        )
        .bind(("isid", "vs_bob".to_string()))
        .await
        .expect("processing update (no-op)");

        let after = fetch_status(&row).await;
        assert_eq!(
            after.status, "verified",
            "verified must NOT be demoted to processing"
        );
    });
}

#[test]
fn processing_handler_ignores_unknown_identity_session() {
    // Unknown session id → no-op, no error.
    common::setup_test_db();
    clean();

    common::run(async {
        // No matching row exists; UPDATE should affect 0 rows and not error.
        let result = DB
            .query(
                "UPDATE verification_payment SET status = 'processing', updated_at = time::now() \
                 WHERE stripe_identity_session_id = $isid AND status IN ['paid']",
            )
            .bind(("isid", "vs_does_not_exist".to_string()))
            .await;
        assert!(result.is_ok(), "no-op update must not error");
    });
}

// ---------------------------------------------------------------------------
// charge.refunded — matches by payment_intent, captures refund_id, idempotent
// ---------------------------------------------------------------------------

#[test]
fn charge_refunded_handler_marks_row_refunded() {
    common::setup_test_db();
    clean();

    common::run(async {
        let person = seed_person("carol_ref").await;
        let row = seed_payment(
            &person,
            "cs_carol",
            Some("pi_carol"),
            Some("vs_carol"),
            "paid",
        )
        .await;

        // Same SQL as on_charge_refunded()
        DB.query(
            "UPDATE verification_payment SET status = 'refunded', refund_id = $rid, updated_at = time::now() \
             WHERE stripe_payment_intent_id = $pi AND status != 'refunded'",
        )
        .bind(("pi", "pi_carol".to_string()))
        .bind(("rid", Some("re_carol_first".to_string())))
        .await
        .expect("refund update");

        let after = fetch_status(&row).await;
        assert_eq!(after.status, "refunded");
        assert_eq!(after.refund_id.as_deref(), Some("re_carol_first"));
    });
}

#[test]
fn charge_refunded_handler_is_idempotent_on_duplicate_event() {
    // Stripe retries webhooks. A second charge.refunded for the same charge
    // must NOT overwrite the first refund_id we recorded.
    common::setup_test_db();
    clean();

    common::run(async {
        let person = seed_person("dave_ref").await;
        let row = seed_payment(&person, "cs_dave", Some("pi_dave"), None, "paid").await;

        // First delivery — flips to refunded with refund_id "re_first".
        DB.query(
            "UPDATE verification_payment SET status = 'refunded', refund_id = $rid, updated_at = time::now() \
             WHERE stripe_payment_intent_id = $pi AND status != 'refunded'",
        )
        .bind(("pi", "pi_dave".to_string()))
        .bind(("rid", Some("re_first".to_string())))
        .await
        .expect("first refund");

        // Second delivery — would try to set refund_id "re_second" but
        // the `status != 'refunded'` guard now blocks the row.
        DB.query(
            "UPDATE verification_payment SET status = 'refunded', refund_id = $rid, updated_at = time::now() \
             WHERE stripe_payment_intent_id = $pi AND status != 'refunded'",
        )
        .bind(("pi", "pi_dave".to_string()))
        .bind(("rid", Some("re_second".to_string())))
        .await
        .expect("second refund (no-op)");

        let after = fetch_status(&row).await;
        assert_eq!(after.status, "refunded");
        assert_eq!(
            after.refund_id.as_deref(),
            Some("re_first"),
            "second event must not overwrite the first refund_id"
        );
    });
}

#[test]
fn charge_refunded_handler_ignores_unknown_payment_intent() {
    // Some dashboard refund on a charge we don't have a row for — no-op.
    common::setup_test_db();
    clean();

    common::run(async {
        let result = DB
            .query(
                "UPDATE verification_payment SET status = 'refunded', refund_id = $rid, updated_at = time::now() \
                 WHERE stripe_payment_intent_id = $pi AND status != 'refunded'",
            )
            .bind(("pi", "pi_does_not_exist".to_string()))
            .bind(("rid", Some("re_xxx".to_string())))
            .await;
        assert!(result.is_ok(), "no-op update must not error");
    });
}

// ---------------------------------------------------------------------------
// Schema: 'processing' is now an accepted value for verification_payment.status
// ---------------------------------------------------------------------------

#[test]
fn schema_accepts_processing_status() {
    common::setup_test_db();
    clean();

    common::run(async {
        let person = seed_person("schema_proc").await;
        // Directly create with status='processing' — schema ASSERT must allow it.
        let result = DB
            .query(
                "CREATE verification_payment SET person = $pid, \
                 stripe_checkout_session_id = 'cs_schema', \
                 amount_minor = 1000, currency = 'usd', status = 'processing'",
            )
            .bind(("pid", person.clone()))
            .await;
        assert!(result.is_ok(), "schema must accept status='processing'");
    });
}
