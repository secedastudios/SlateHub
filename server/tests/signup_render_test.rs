//! Render guard for the signup form.
//!
//! Regression: a signup that errors (e.g. duplicate email) re-renders the form,
//! and that re-render MUST carry fresh anti-bot tokens. If `form_token` /
//! `pow_challenge` come through empty, the user's resubmit fails the form-token
//! check with a 422 (see `routes::auth::fresh_signup_template`). These tests
//! pin that the template actually emits the token fields, and that an error
//! re-render keeps the entered email.

use askama::Template;
use slatehub::templates::{BaseContext, SignupTemplate};

#[test]
fn renders_antibot_token_fields() {
    let mut t = SignupTemplate::new(BaseContext::new());
    t.form_token = "TESTFORMTOKEN".to_string();
    t.pow_challenge = "TESTCHALLENGE".to_string();
    let html = t.render().expect("signup template renders");

    assert!(
        html.contains(r#"name="form_token" value="TESTFORMTOKEN""#),
        "form_token hidden field must carry the value the handler sets"
    );
    assert!(
        html.contains(r#"name="pow_challenge" value="TESTCHALLENGE""#),
        "pow_challenge hidden field must carry the value the handler sets"
    );
}

#[test]
fn empty_token_renders_empty_value_the_failure_mode() {
    // Documents the bug: default (unset) tokens render empty, which is what
    // blocked retries after a validation error before the fix.
    let html = SignupTemplate::new(BaseContext::new())
        .render()
        .expect("renders");
    assert!(html.contains(r#"name="form_token" value="""#));
}

#[test]
fn error_rerender_keeps_email_and_shows_error() {
    let mut t = SignupTemplate::new(BaseContext::new());
    t.form_token = "x".to_string();
    t.pow_challenge = "y".to_string();
    t.error = Some("Email already exists".to_string());
    t.prefill_email = Some("taken@example.com".to_string());
    let html = t.render().expect("renders");

    assert!(html.contains("Email already exists"), "error must show");
    assert!(
        html.contains("taken@example.com"),
        "entered email must be preserved so the user need not retype"
    );
}
