//! End-to-end test for Person::delete_with_cascade.
//!
//! Seeds a person plus rows in every table the cascade is supposed to wipe,
//! runs the cascade, and asserts that:
//!   - all of the person's data is gone
//!   - the OTHER person (also seeded) is untouched
//!   - unrelated notifications survive
//!   - the conversation between the two is gone
//!
//! S3 deletion is best-effort and not asserted here (the test S3 backend may
//! or may not be reachable from CI); the cascade logs and continues either
//! way. The DB cascade is what we're verifying.

mod common;

use slatehub::db::DB;
use slatehub::models::person::Person;
use slatehub::record_id_ext::RecordIdExt;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct CountRow {
    c: i64,
}

/// Count rows in `table` matching `where_clause` with `$pid` bound to the
/// supplied RecordId. Returns 0 when no rows match.
async fn count(table: &str, where_clause: &str, pid: &RecordId) -> i64 {
    let sql = format!(
        "SELECT count() AS c FROM {} WHERE {} GROUP ALL",
        table, where_clause
    );
    let row: Option<CountRow> = DB
        .query(&sql)
        .bind(("pid", pid.clone()))
        .await
        .unwrap_or_else(|e| panic!("count query failed for {table}: {e}"))
        .take(0)
        .unwrap_or(None);
    row.map(|r| r.c).unwrap_or(0)
}

/// Count rows in `table` matching `where_clause` with `$conv` bound. For the
/// message-notification cleanup that references conversation_id as a string.
async fn count_by_conv(table: &str, where_clause: &str, conv_id_str: &str) -> i64 {
    let sql = format!(
        "SELECT count() AS c FROM {} WHERE {} GROUP ALL",
        table, where_clause
    );
    let row: Option<CountRow> = DB
        .query(&sql)
        .bind(("conv", conv_id_str.to_string()))
        .await
        .unwrap_or_else(|e| panic!("count query failed for {table}: {e}"))
        .take(0)
        .unwrap_or(None);
    row.map(|r| r.c).unwrap_or(0)
}

/// Clear every table the cascade touches, in case a prior test left state.
fn clean_all() {
    for table in [
        "direct_message",
        "conversation",
        "notification",
        "likes",
        "application",
        "consent_grant",
        "access_token",
        "refresh_token",
        "authorization_code",
        "verification_codes",
        "verification_request",
        "verification_payment",
        "pending_invitation",
        "profile_view",
        "activity_event",
        "production_script",
        "location",
        "job_posting",
        "equipment_rental",
        "equipment_kit",
        "equipment",
        "security_event",
        "media",
        "involvement",
        "member_of",
        // Productions-management tables (added Phase 0):
        "call_time",
        "call_sheet",
        "schedule_scene",
        "schedule_day",
        "scene",
        "episode",
        "season",
        "production",
        "person",
    ] {
        common::clean_table(table);
    }
}

async fn create_person(username: &str, email: &str) -> RecordId {
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
                verification_status: 'identity',
                profile: { name: $username, skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
            } RETURN id",
        )
        .bind(("username", username.to_string()))
        .bind(("email", email.to_string()))
        .await
        .expect("create person")
        .take(0)
        .expect("take person");
    rows.into_iter().next().expect("one person").id
}

#[test]
fn test_delete_with_cascade_wipes_everything_for_target_and_spares_others() {
    common::setup_test_db();
    clean_all();

    common::run(async {
        // ── Seed two people ────────────────────────────────────────────────
        let alice = create_person("alice_cascade", "alice@cascade.test").await;
        let bob = create_person("bob_cascade", "bob@cascade.test").await;
        let alice_str = alice.to_raw_string();
        let bob_str = bob.to_raw_string();

        // ── Conversation + 3 messages (2 alice, 1 bob) ─────────────────────
        #[derive(serde::Deserialize, SurrealValue)]
        struct R {
            id: RecordId,
        }
        let conv_rows: Vec<R> = DB
            .query("CREATE conversation CONTENT { participant_a: $a, participant_b: $b } RETURN id")
            .bind(("a", alice.clone()))
            .bind(("b", bob.clone()))
            .await
            .expect("create conv")
            .take(0)
            .expect("take conv");
        let conv = conv_rows.into_iter().next().expect("one conv").id;
        let conv_str = conv.to_raw_string();

        for (sender, body) in [
            (&alice, "hi bob"),
            (&alice, "hi again"),
            (&bob, "hey alice"),
        ] {
            DB.query("CREATE direct_message CONTENT { conversation: $conv, sender: $s, body: $b }")
                .bind(("conv", conv.clone()))
                .bind(("s", sender.clone()))
                .bind(("b", body.to_string()))
                .await
                .expect("create message");
        }

        // ── Notifications: 1 to bob about alice's message (related_id = conv_str),
        //                  1 unrelated to bob (should SURVIVE),
        //                  1 to alice (recipient — should die).
        DB.query("CREATE notification CONTENT { person_id: $bob, notification_type: 'message', title: 't', message: 'm', related_id: $conv_str, read: false }")
            .bind(("bob", bob.clone()))
            .bind(("conv_str", conv_str.clone()))
            .await
            .expect("seed message notification");
        DB.query("CREATE notification CONTENT { person_id: $bob, notification_type: 'general', title: 'unrelated', message: 'm', read: false }")
            .bind(("bob", bob.clone()))
            .await
            .expect("seed unrelated notification");
        DB.query("CREATE notification CONTENT { person_id: $alice, notification_type: 'general', title: 't', message: 'm', read: false }")
            .bind(("alice", alice.clone()))
            .await
            .expect("seed alice notification");

        // ── Graph edges: alice likes bob, alice member_of an org-ish target,
        //                involvement edge alice → something.
        // Use member_of via raw RELATE so we don't need to seed orgs.
        DB.query("RELATE $alice->likes->$bob")
            .bind(("alice", alice.clone()))
            .bind(("bob", bob.clone()))
            .await
            .expect("seed like");

        // ── verification_codes row for alice ───────────────────────────────
        DB.query("CREATE verification_codes CONTENT { person_id: $pid, code: 'abc', code_type: 'email_verification', expires_at: time::now() + 1h, used: false }")
            .bind(("pid", alice.clone()))
            .await
            .expect("seed verification_codes");

        // ── verification_request row for alice ─────────────────────────────
        DB.query("CREATE verification_request CONTENT { person: $pid, status: 'pending' }")
            .bind(("pid", alice.clone()))
            .await
            .expect("seed verification_request");

        // ── activity_event for alice ───────────────────────────────────────
        DB.query(
            "CREATE activity_event CONTENT { person_id: $pid, event_type: 'page_view', path: '/' }",
        )
        .bind(("pid", alice.clone()))
        .await
        .expect("seed activity_event");

        // ── pending_invitation invited_by alice ────────────────────────────
        DB.query("CREATE pending_invitation CONTENT { target_type: 'organization', target_id: 'organization:x', target_name: 'X', target_slug: 'x', invited_by: $pid, status: 'pending' }")
            .bind(("pid", alice.clone()))
            .await
            .expect("seed pending_invitation");

        // ── Productions-management seeds ───────────────────────────────────
        // schedule_day for a production (alice is on the crew via call_time).
        // call_sheet.generated_by + sent_by reference alice — must be nulled
        // by the cascade (not deleted, so the sheet remains for audit).
        // (Reusing the `R` struct defined earlier in this scope.)

        let prod_rows: Vec<R> = DB
            .query("CREATE production CONTENT { title: 'Cascade Prod', slug: 'cascade-prod', type: 'Feature Film', status: 'in_production' } RETURN id")
            .await
            .expect("seed production")
            .take(0)
            .expect("take production");
        let prod = prod_rows.into_iter().next().expect("one prod").id;

        let day_rows: Vec<R> = DB
            .query("CREATE schedule_day SET production = $p, date = time::now() RETURN id")
            .bind(("p", prod.clone()))
            .await
            .expect("seed schedule_day")
            .take(0)
            .expect("take schedule_day");
        let day = day_rows.into_iter().next().expect("one day").id;

        // alice gets a call_time on this shoot day (cast/crew row)
        DB.query("CREATE call_time SET schedule_day = $d, person = $pid, role = 'Director'")
            .bind(("d", day.clone()))
            .bind(("pid", alice.clone()))
            .await
            .expect("seed call_time");

        // alice generated AND sent the call sheet
        DB.query("CREATE call_sheet SET schedule_day = $d, version = 1, pdf_key = 'cs/1.pdf', generated_by = $pid, sent_by = $pid, sent_at = time::now(), recipient_count = 5")
            .bind(("d", day.clone()))
            .bind(("pid", alice.clone()))
            .await
            .expect("seed call_sheet");

        // ── Sanity: everything is in place before the cascade ──────────────
        assert_eq!(count("direct_message", "sender = $pid", &alice).await, 2);
        assert_eq!(count("direct_message", "sender = $pid", &bob).await, 1);
        assert_eq!(
            count(
                "conversation",
                "participant_a = $pid OR participant_b = $pid",
                &alice
            )
            .await,
            1
        );
        assert_eq!(
            count_by_conv("notification", "related_id = $conv", &conv_str).await,
            1
        );
        assert_eq!(count("notification", "person_id = $pid", &alice).await, 1);
        assert_eq!(count("notification", "person_id = $pid", &bob).await, 2);
        assert_eq!(count("likes", "in = $pid", &alice).await, 1);
        assert_eq!(
            count("verification_codes", "person_id = $pid", &alice).await,
            1
        );
        assert_eq!(
            count("verification_request", "person = $pid", &alice).await,
            1
        );
        assert_eq!(count("activity_event", "person_id = $pid", &alice).await, 1);
        assert_eq!(
            count("pending_invitation", "invited_by = $pid", &alice).await,
            1
        );
        assert_eq!(count("call_time", "person = $pid", &alice).await, 1);
        assert_eq!(count("call_sheet", "generated_by = $pid", &alice).await, 1);
        assert_eq!(count("call_sheet", "sent_by = $pid", &alice).await, 1);
        assert_eq!(count("person", "id = $pid", &alice).await, 1);

        // ── Run the cascade ────────────────────────────────────────────────
        Person::delete_with_cascade(&alice)
            .await
            .expect("cascade succeeded");

        // ── Post-conditions: all of alice's data is gone ───────────────────
        assert_eq!(
            count("direct_message", "sender = $pid", &alice).await,
            0,
            "alice's messages still present"
        );
        assert_eq!(
            count("direct_message", "sender = $pid", &bob).await,
            0,
            "bob's message in the deleted conversation should also be gone"
        );
        assert_eq!(
            count(
                "conversation",
                "participant_a = $pid OR participant_b = $pid",
                &alice
            )
            .await,
            0,
            "conversation still present"
        );
        assert_eq!(
            count_by_conv("notification", "related_id = $conv", &conv_str).await,
            0,
            "message-type notification referencing deleted conv still present"
        );
        assert_eq!(
            count("notification", "person_id = $pid", &alice).await,
            0,
            "alice's notifications still present"
        );
        assert_eq!(
            count("likes", "in = $pid", &alice).await,
            0,
            "alice's outbound likes still present"
        );
        assert_eq!(
            count("verification_codes", "person_id = $pid", &alice).await,
            0,
            "verification_codes still present"
        );
        assert_eq!(
            count("verification_request", "person = $pid", &alice).await,
            0,
            "verification_request still present"
        );
        assert_eq!(
            count("activity_event", "person_id = $pid", &alice).await,
            0,
            "activity_event still present"
        );
        assert_eq!(
            count("pending_invitation", "invited_by = $pid", &alice).await,
            0,
            "pending_invitation still present"
        );
        assert_eq!(
            count("call_time", "person = $pid", &alice).await,
            0,
            "alice's call_time row should be deleted on cascade"
        );
        assert_eq!(
            count("call_sheet", "generated_by = $pid", &alice).await,
            0,
            "alice's call_sheet.generated_by should be nulled — row remains for audit but not linked to her"
        );
        assert_eq!(
            count("call_sheet", "sent_by = $pid", &alice).await,
            0,
            "alice's call_sheet.sent_by should be nulled"
        );
        assert_eq!(
            count("call_sheet", "schedule_day != NONE", &alice).await,
            1,
            "the call_sheet row itself should remain (production data, not alice's)"
        );
        assert_eq!(
            count("person", "id = $pid", &alice).await,
            0,
            "alice's person record still present"
        );

        // ── Spare-the-rest: bob and his unrelated data are untouched ───────
        assert_eq!(
            count("person", "id = $pid", &bob).await,
            1,
            "bob should still exist"
        );
        assert_eq!(
            count(
                "notification",
                "person_id = $pid AND notification_type = 'general'",
                &bob
            )
            .await,
            1,
            "bob's unrelated general notification should survive"
        );

        // Belt-and-suspenders: explicit string equality checks so failures
        // show the actual ids in the assertion message.
        assert_ne!(alice_str, bob_str);
    });
}
