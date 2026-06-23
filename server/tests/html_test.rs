//! Unit tests for `slatehub::html` — escaping helpers for hand-built
//! fragments. Pure functions; no test DB required.

use slatehub::html::{escape_attr, escape_html};

#[test]
fn escape_html_neutralizes_tags() {
    // ammonia::clean_text entity-encodes `/` too (&#47;), which is harmless
    // in text nodes and part of its hardened output.
    assert_eq!(
        escape_html("<script>x</script>"),
        "&lt;script&gt;x&lt;&#47;script&gt;"
    );
}

#[test]
fn escape_attr_neutralizes_quote_breakout() {
    assert_eq!(escape_attr(r#"a"b&c<d>"#), "a&quot;b&amp;c&lt;d&gt;");
}
