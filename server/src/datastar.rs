//! Server-Sent-Events helpers for [Datastar](https://data-star.dev) hypermedia patches.
//!
//! Slatehub's interactive lists (infinite scroll, live search, like buttons,
//! message streams …) respond to `@get`/`@post` Datastar actions with a
//! `text/event-stream` body containing `datastar-patch-elements` events. The
//! browser-side Datastar runtime applies each event's `elements` fragment to
//! the DOM at `selector` using `mode` (`append`, `inner`, `outer`, …).
//!
//! Every SSE-speaking route used to carry private copies of these two
//! helpers; this module is the single shared implementation. Fragments
//! interpolated into [`patch_elements`] must already be escaped — see
//! [`crate::html::escape_html`].

use axum::http::header;
use axum::response::{IntoResponse, Response};

/// Render one `datastar-patch-elements` SSE event.
///
/// `selector` is a CSS selector for the patch target, `mode` a Datastar
/// patch mode (`append`, `inner`, `outer`, `remove`, …), and `elements` the
/// HTML fragment to apply (pass an empty string for modes like `remove`
/// that take no markup). Newlines inside `elements` are flattened to spaces
/// because the SSE framing is line-oriented.
///
/// Concatenate multiple events into one body and hand the result to
/// [`response`].
pub fn patch_elements(selector: &str, mode: &str, elements: &str) -> String {
    let mut event = format!(
        "event: datastar-patch-elements\ndata: selector {}\ndata: mode {}\n",
        selector, mode
    );
    if !elements.is_empty() {
        event += &format!("data: elements {}\n", elements.replace('\n', " "));
    }
    event += "\n";
    event
}

/// Wrap a pre-rendered SSE body in a `text/event-stream` response.
///
/// Sets `Cache-Control: no-cache` so proxies never replay stale patches.
pub fn response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}
