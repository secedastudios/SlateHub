//! Unit tests for the provider-selection precedence and Postmark address
//! formatting in `services::email`. Both are pure functions (no env reads, no
//! network), so these run fast and deterministically without manipulating
//! process-wide environment variables.

use slatehub::services::email::{EmailError, ProviderKind, format_address, select_provider_kind};

// ---------------------------------------------------------------------------
// select_provider_kind — auto-detection (no explicit EMAIL_PROVIDER)
// ---------------------------------------------------------------------------

#[test]
fn auto_prefers_postmark_when_both_present() {
    assert_eq!(
        select_provider_kind(None, true, true).unwrap(),
        ProviderKind::Postmark
    );
}

#[test]
fn auto_uses_postmark_when_only_postmark() {
    assert_eq!(
        select_provider_kind(None, true, false).unwrap(),
        ProviderKind::Postmark
    );
}

#[test]
fn auto_falls_back_to_mailjet_when_only_mailjet() {
    assert_eq!(
        select_provider_kind(None, false, true).unwrap(),
        ProviderKind::Mailjet
    );
}

#[test]
fn auto_errors_when_nothing_configured() {
    assert!(matches!(
        select_provider_kind(None, false, false),
        Err(EmailError::ConfigError(_))
    ));
}

// ---------------------------------------------------------------------------
// select_provider_kind — explicit EMAIL_PROVIDER override
// ---------------------------------------------------------------------------

#[test]
fn explicit_postmark_is_honored() {
    assert_eq!(
        select_provider_kind(Some("postmark"), true, true).unwrap(),
        ProviderKind::Postmark
    );
}

#[test]
fn explicit_mailjet_overrides_postmark_preference() {
    assert_eq!(
        select_provider_kind(Some("mailjet"), true, true).unwrap(),
        ProviderKind::Mailjet
    );
}

#[test]
fn explicit_is_case_insensitive_and_trimmed() {
    assert_eq!(
        select_provider_kind(Some("  Postmark "), true, false).unwrap(),
        ProviderKind::Postmark
    );
    assert_eq!(
        select_provider_kind(Some("MAILJET"), false, true).unwrap(),
        ProviderKind::Mailjet
    );
}

#[test]
fn explicit_provider_without_its_credentials_errors() {
    assert!(matches!(
        select_provider_kind(Some("postmark"), false, true),
        Err(EmailError::ConfigError(_))
    ));
    assert!(matches!(
        select_provider_kind(Some("mailjet"), true, false),
        Err(EmailError::ConfigError(_))
    ));
}

#[test]
fn unknown_explicit_provider_errors() {
    assert!(matches!(
        select_provider_kind(Some("sendgrid"), true, true),
        Err(EmailError::ConfigError(_))
    ));
}

#[test]
fn blank_explicit_falls_back_to_auto_detect() {
    // An empty/whitespace EMAIL_PROVIDER behaves as if unset.
    assert_eq!(
        select_provider_kind(Some("   "), true, true).unwrap(),
        ProviderKind::Postmark
    );
}

// ---------------------------------------------------------------------------
// format_address — Postmark "Name <email>" rendering
// ---------------------------------------------------------------------------

#[test]
fn format_address_includes_display_name() {
    assert_eq!(
        format_address("a@b.com", Some("Jane Doe")),
        "Jane Doe <a@b.com>"
    );
}

#[test]
fn format_address_bare_when_no_name() {
    assert_eq!(format_address("a@b.com", None), "a@b.com");
}

#[test]
fn format_address_bare_when_name_blank() {
    assert_eq!(format_address("a@b.com", Some("   ")), "a@b.com");
}
