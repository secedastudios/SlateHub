//! Unit tests for `SessionUser::record_id()` — the canonical accessor that
//! gives new code a proper `RecordId` without ad-hoc string parsing.
//!
//! These don't need a DB — pure logic.

use slatehub::models::person::SessionUser;
use slatehub::record_id_ext::RecordIdExt;
use surrealdb::types::RecordId;

fn make_session(id: &str) -> SessionUser {
    SessionUser {
        id: id.to_string(),
        username: "test".to_string(),
        email: "t@t.test".to_string(),
        name: "Test".to_string(),
    }
}

#[test]
fn record_id_parses_prefixed_form() {
    let user = make_session("person:abc123");
    let rid = user.record_id().expect("should parse");
    assert_eq!(rid.to_raw_string(), "person:abc123");
}

#[test]
fn record_id_handles_legacy_bare_key() {
    // Some legacy code paths may have stored a bare key. The helper should
    // still produce a valid person RecordId so callers don't have to care.
    let user = make_session("legacy_bare_key");
    let rid = user.record_id().expect("should construct");
    assert_eq!(rid.to_raw_string(), "person:legacy_bare_key");
}

#[test]
fn record_id_handles_ulid_form() {
    // SurrealDB usually generates ULID-like ids — make sure those round-trip.
    let ulid = "01k0q3a8z2v7m9n4r8x6y3p1c5";
    let user = make_session(&format!("person:{}", ulid));
    let rid = user.record_id().expect("should parse");
    assert_eq!(rid.to_raw_string(), format!("person:{}", ulid));
}

#[test]
fn record_id_can_be_used_as_bind_param() {
    // Compile-time check: the returned RecordId is the right type to feed
    // into a `.bind(("pid", rid))` call. We don't actually run the query;
    // we just need this to type-check.
    let user = make_session("person:bind_check");
    let rid: RecordId = user.record_id().expect("parse");
    let _ = (rid,); // consume so the binding isn't optimized away
}
