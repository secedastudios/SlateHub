//! Render guard for the email-verification page.
//!
//! The "resend verification email" control MUST submit the verify form to the
//! POST-only `/resend-verification` route (via `formaction` + the `form`
//! attribute), never be a bare `<a href>` GET link. A GET hits the POST-only
//! route and 405s, and the Facebook/Instagram in-app browsers where ad traffic
//! lands turn a bare 405 into a file-download prompt. See
//! `routes::auth::resend_verification` (registered as POST only).

use askama::Template;
use slatehub::templates::{BaseContext, EmailVerificationTemplate};

fn render() -> String {
    EmailVerificationTemplate::new(BaseContext::new())
        .render()
        .expect("verify-email template renders")
}

#[test]
fn resend_submits_the_form_via_post_not_a_get_link() {
    let html = render();
    assert!(
        html.contains("formaction=\"/resend-verification\""),
        "resend must POST to /resend-verification via formaction"
    );
    assert!(
        html.contains("form=\"verify-form\""),
        "resend button must be associated with the verify form"
    );
    assert!(
        html.contains("formnovalidate"),
        "resend must skip the (required) code-field validation"
    );
}

#[test]
fn resend_is_not_a_get_anchor() {
    // Regression: a GET link to the POST-only route 405s and triggers the
    // mobile in-app-browser download bug.
    let html = render();
    assert!(
        !html.contains("<a href=\"/resend-verification\""),
        "resend must not be a GET anchor to the POST-only route"
    );
}

#[test]
fn verify_form_has_id_for_external_button() {
    // The resend button lives outside <form>, so the form needs an id for the
    // button's `form="verify-form"` association to work.
    let html = render();
    assert!(
        html.contains("id=\"verify-form\""),
        "the verify form needs id=\"verify-form\""
    );
}
