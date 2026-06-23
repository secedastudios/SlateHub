//! HTML-safety helpers for hand-built markup fragments.
//!
//! Most HTML in slatehub is rendered through Askama templates, which escape
//! by default. These helpers exist for the handful of places that build
//! fragments with `format!` instead — primarily the Datastar SSE patches in
//! route handlers (see [`crate::datastar`]) and `<meta>`/attribute values
//! assembled outside templates.
//!
//! Single source of truth: route files previously each carried a private
//! copy of `escape_html`; they all delegate here now.

/// Escape text for safe interpolation into HTML element content.
///
/// Delegates to [`ammonia::clean_text`], which HTML-entity-encodes all
/// markup-significant characters. Suitable for text nodes; for attribute
/// values prefer [`escape_attr`].
pub fn escape_html(s: &str) -> String {
    ammonia::clean_text(s)
}

/// Escape text for safe interpolation into a double-quoted HTML attribute.
///
/// Encodes `&`, `"`, `<`, and `>`. Use for `attr="{value}"` interpolations
/// in hand-built fragments; single-quoted attributes are not covered, so
/// always double-quote.
pub fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
