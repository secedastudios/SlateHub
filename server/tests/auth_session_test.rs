//! Tests for "Remember me" sessions: `session_duration` picks 12 hours by
//! default and 30 days when remembered, the JWT `exp`/`remember` claims match
//! what was requested, tokens minted before the `remember` claim existed still
//! decode, and `should_refresh_session` only slides remembered sessions once
//! the token is a day old. The same duration sizes the cookie Max-Age in the
//! login handler and middleware refresh, so these constants are the contract.

use slatehub::auth::{
    Claims, JwtConfig, SESSION_REFRESH_AFTER_SECS, create_jwt, create_jwt_with_duration,
    create_session_jwt, decode_jwt, session_duration, should_refresh_session,
};

/// Tests that mint real tokens need the signing secret. Same value every time,
/// so concurrent test threads setting it are benign.
fn ensure_secret() {
    unsafe { std::env::set_var("JWT_SECRET", "test-secret-for-auth-session-tests") }
}

#[test]
fn standard_session_is_12_hours_by_default() {
    // JWT_DURATION unset in the test environment -> default applies.
    assert_eq!(session_duration(false), 43_200);
    assert_eq!(JwtConfig::token_duration(), 43_200);
}

#[test]
fn remembered_session_is_30_days_by_default() {
    // JWT_REMEMBER_DURATION unset in the test environment -> default applies.
    assert_eq!(session_duration(true), 2_592_000);
    assert_eq!(JwtConfig::remember_duration(), 2_592_000);
}

#[test]
fn jwt_exp_matches_requested_duration() {
    ensure_secret();
    let token = create_jwt_with_duration("person:test", "tester", "t@example.com", 12_345)
        .expect("token creation");
    let claims = decode_jwt(&token).expect("token decodes");
    assert_eq!(claims.exp - claims.iat, 12_345);
    assert_eq!(claims.sub, "person:test");
    assert_eq!(claims.username, "tester");
}

#[test]
fn default_create_jwt_uses_standard_duration() {
    ensure_secret();
    let token = create_jwt("person:test", "tester", "t@example.com").expect("token creation");
    let claims = decode_jwt(&token).expect("token decodes");
    assert_eq!(claims.exp - claims.iat, JwtConfig::token_duration());
}

#[test]
fn remembered_jwt_lives_30_days() {
    ensure_secret();
    let token = create_jwt_with_duration(
        "person:test",
        "tester",
        "t@example.com",
        session_duration(true),
    )
    .expect("token creation");
    let claims = decode_jwt(&token).expect("token decodes");
    assert_eq!(claims.exp - claims.iat, 2_592_000);
}

#[test]
fn session_jwt_carries_the_remember_claim() {
    ensure_secret();
    let remembered =
        create_session_jwt("person:test", "tester", "t@example.com", true).expect("token creation");
    let claims = decode_jwt(&remembered).expect("token decodes");
    assert!(claims.remember);
    assert_eq!(claims.exp - claims.iat, 2_592_000);

    let standard = create_session_jwt("person:test", "tester", "t@example.com", false)
        .expect("token creation");
    let claims = decode_jwt(&standard).expect("token decodes");
    assert!(!claims.remember);
    assert_eq!(claims.exp - claims.iat, 43_200);
}

#[test]
fn plain_create_jwt_is_not_remembered() {
    ensure_secret();
    let token = create_jwt("person:test", "tester", "t@example.com").expect("token creation");
    let claims = decode_jwt(&token).expect("token decodes");
    assert!(!claims.remember);
}

#[test]
fn tokens_minted_before_the_remember_claim_still_decode() {
    // Tokens issued by older builds have no `remember` field; they must keep
    // decoding (serde default -> false) so existing logins survive the deploy.
    ensure_secret();
    #[derive(serde::Serialize)]
    struct LegacyClaims {
        sub: String,
        username: String,
        email: String,
        iat: u64,
        exp: u64,
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let legacy = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &LegacyClaims {
            sub: "person:test".to_string(),
            username: "tester".to_string(),
            email: "t@example.com".to_string(),
            iat: now,
            exp: now + 1_000,
        },
        &jsonwebtoken::EncodingKey::from_secret("test-secret-for-auth-session-tests".as_bytes()),
    )
    .expect("legacy token encodes");

    let claims = decode_jwt(&legacy).expect("legacy token decodes");
    assert!(!claims.remember, "missing claim must default to false");
}

fn claims(remember: bool, iat: u64) -> Claims {
    Claims {
        sub: "person:test".to_string(),
        username: "tester".to_string(),
        email: "t@example.com".to_string(),
        iat,
        exp: iat + session_duration(remember),
        remember,
    }
}

#[test]
fn refresh_policy_slides_only_remembered_day_old_sessions() {
    let now = 10_000_000;

    // Remembered + a day old -> slide.
    assert!(should_refresh_session(
        &claims(true, now - SESSION_REFRESH_AFTER_SECS),
        now
    ));
    assert!(should_refresh_session(&claims(true, now - 200_000), now));

    // Remembered but fresh -> leave alone (throttle).
    assert!(!should_refresh_session(&claims(true, now - 100), now));

    // Standard 12h session -> never slides, however old.
    assert!(!should_refresh_session(&claims(false, now - 200_000), now));

    // Clock skew (iat in the future) must not underflow or refresh.
    assert!(!should_refresh_session(&claims(true, now + 500), now));
}
