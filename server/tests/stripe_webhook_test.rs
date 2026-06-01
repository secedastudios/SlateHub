//! Unit tests for Stripe webhook signature verification + price selection.
//!
//! These deliberately do NOT need the test DB — they exercise pure logic.
//! Run them in any environment with just `cargo test --test stripe_webhook_test`.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use slatehub::services::stripe::{StripeService, pick_price};

type HmacSha256 = Hmac<Sha256>;

/// Sign a payload exactly the way Stripe does and produce the value of the
/// `Stripe-Signature` header. `extra_v1` lets us inject a second `v1=…`
/// signature to simulate Stripe's key-rotation behaviour.
fn sign(secret: &str, ts: i64, body: &[u8], extra_v1: Option<&str>) -> String {
    let signed_payload = format!("{}.{}", ts, std::str::from_utf8(body).unwrap_or(""));
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    match extra_v1 {
        Some(extra) => format!("t={},v1={},v1={}", ts, sig, extra),
        None => format!("t={},v1={}", ts, sig),
    }
}

/// Spin up a StripeService bound to a known fake secret. Requires the env
/// vars to be present; tests set them inline.
fn service_with_secret(secret: &str) -> StripeService {
    // SAFETY: tests run with --test-threads=1, so env var mutation is OK.
    unsafe {
        std::env::set_var("STRIPE_SECRET_KEY", "sk_test_fake_for_unit_test");
        std::env::set_var("STRIPE_WEBHOOK_SECRET", secret);
    }
    StripeService::from_env().expect("service should build from env")
}

#[test]
fn webhook_accepts_a_valid_signature() {
    let secret = "whsec_test_secret_for_unit";
    let stripe = service_with_secret(secret);

    let body = b"{\"id\":\"evt_test\",\"type\":\"identity.verification_session.verified\"}";
    let ts = chrono::Utc::now().timestamp();
    let sig_header = sign(secret, ts, body, None);

    assert!(
        stripe.verify_webhook(body, &sig_header).is_ok(),
        "valid signature must be accepted"
    );
}

#[test]
fn webhook_rejects_a_tampered_body() {
    let secret = "whsec_test_secret_for_unit";
    let stripe = service_with_secret(secret);

    let body = b"{\"id\":\"evt_test\"}";
    let ts = chrono::Utc::now().timestamp();
    let sig_header = sign(secret, ts, body, None);

    // Now tamper with the body and re-check with the same header.
    let tampered = b"{\"id\":\"evt_attacker\"}";
    assert!(
        stripe.verify_webhook(tampered, &sig_header).is_err(),
        "tampered body must fail verification"
    );
}

#[test]
fn webhook_rejects_wrong_secret() {
    let secret = "whsec_correct_secret";
    let stripe = service_with_secret(secret);

    let body = b"{}";
    let ts = chrono::Utc::now().timestamp();
    // Sign with the wrong secret.
    let sig_header = sign("whsec_attacker_secret", ts, body, None);

    assert!(
        stripe.verify_webhook(body, &sig_header).is_err(),
        "wrong-secret signature must fail"
    );
}

#[test]
fn webhook_rejects_stale_timestamp_replay() {
    let secret = "whsec_test_secret_for_unit";
    let stripe = service_with_secret(secret);

    let body = b"{}";
    // 10 minutes ago — outside the 5-minute tolerance window.
    let stale_ts = chrono::Utc::now().timestamp() - 600;
    let sig_header = sign(secret, stale_ts, body, None);

    assert!(
        stripe.verify_webhook(body, &sig_header).is_err(),
        "stale timestamp must be rejected (replay protection)"
    );
}

#[test]
fn webhook_accepts_one_of_rotating_signatures() {
    // During key rotation Stripe sends `t=...,v1=sigA,v1=sigB`.
    // We accept if ANY v1 matches our secret.
    let secret = "whsec_current_secret";
    let stripe = service_with_secret(secret);

    let body = b"{}";
    let ts = chrono::Utc::now().timestamp();
    // First v1 is from the OLD secret (won't match), second is the correct one.
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{}.{}", ts, std::str::from_utf8(body).unwrap()).as_bytes());
    let good_sig = hex::encode(mac.finalize().into_bytes());

    let sig_header = format!(
        "t={},v1=deadbeef00000000000000000000000000000000000000000000000000000000,v1={}",
        ts, good_sig
    );

    assert!(
        stripe.verify_webhook(body, &sig_header).is_ok(),
        "at least one matching v1 should be sufficient (key rotation)"
    );
}

#[test]
fn webhook_rejects_missing_signature_components() {
    let secret = "whsec_test";
    let stripe = service_with_secret(secret);

    let body = b"{}";

    // Missing v1 entirely
    let ts = chrono::Utc::now().timestamp();
    assert!(
        stripe.verify_webhook(body, &format!("t={}", ts)).is_err(),
        "missing v1 must be rejected"
    );

    // Missing t entirely
    assert!(
        stripe.verify_webhook(body, "v1=abcdef").is_err(),
        "missing t must be rejected"
    );

    // Empty header
    assert!(
        stripe.verify_webhook(body, "").is_err(),
        "empty header must be rejected"
    );
}

// ---------------------------------------------------------------------------
// Price-selection / locale tests — also pure, fast.
// ---------------------------------------------------------------------------

#[test]
fn price_picker_default_is_usd() {
    assert_eq!(pick_price(None).currency, "usd");
    assert_eq!(pick_price(Some("en-US,en;q=0.9")).currency, "usd");
}

#[test]
fn price_picker_eurozone_languages_map_to_eur() {
    for header in [
        "de-DE,de;q=0.9",
        "fr-FR,fr;q=0.9",
        "it-IT",
        "es",
        "nl-BE",
        "pt-PT",
    ] {
        assert_eq!(
            pick_price(Some(header)).currency,
            "eur",
            "header {} should map to EUR",
            header
        );
    }
}

#[test]
fn price_picker_country_tagged_currencies() {
    assert_eq!(pick_price(Some("en-GB,en;q=0.9")).currency, "gbp");
    assert_eq!(pick_price(Some("en-CA")).currency, "cad");
    assert_eq!(pick_price(Some("fr-CA")).currency, "cad");
    assert_eq!(pick_price(Some("en-AU")).currency, "aud");
    assert_eq!(pick_price(Some("de-CH")).currency, "chf");
    assert_eq!(pick_price(Some("ja,en;q=0.9")).currency, "jpy");
}

#[test]
fn price_picker_falls_back_to_usd_for_unknown_locale() {
    assert_eq!(pick_price(Some("xx-XX")).currency, "usd");
    assert_eq!(pick_price(Some("")).currency, "usd");
}
