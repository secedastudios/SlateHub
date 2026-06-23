//! Unit tests for `slatehub::datastar` — SSE patch-event framing.
//! Pure functions; no test DB required.

use slatehub::datastar::{patch_elements, response};

#[test]
fn patch_event_has_required_framing() {
    let event = patch_elements("#list", "append", "<li>x</li>");
    assert!(event.starts_with("event: datastar-patch-elements\n"));
    assert!(event.contains("data: selector #list\n"));
    assert!(event.contains("data: mode append\n"));
    assert!(event.contains("data: elements <li>x</li>\n"));
    assert!(event.ends_with("\n\n"));
}

#[test]
fn empty_elements_omits_the_elements_line() {
    let event = patch_elements("#gone", "remove", "");
    assert!(!event.contains("data: elements"));
}

#[test]
fn newlines_in_fragments_are_flattened() {
    let event = patch_elements("#x", "inner", "<p>\na\n</p>");
    assert!(event.contains("data: elements <p> a </p>\n"));
}

#[test]
fn response_sets_event_stream_headers() {
    let resp = response("event: x\n\n".to_string());
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-cache");
}
