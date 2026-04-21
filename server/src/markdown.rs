use ammonia::Builder;
use pulldown_cmark::{Options, Parser, html};

/// Render markdown to sanitized HTML.
///
/// Uses pulldown-cmark for parsing and ammonia for XSS sanitization.
/// Safe to use with Askama's `|safe` filter.
pub fn render(input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(input, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    Builder::default().clean(&html_output).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        let result = render("**bold** and *italic*");
        assert!(result.contains("<strong>bold</strong>"));
        assert!(result.contains("<em>italic</em>"));
    }

    #[test]
    fn test_line_breaks() {
        let result = render("line one\n\nline two");
        assert!(result.contains("<p>line one</p>"));
        assert!(result.contains("<p>line two</p>"));
    }

    #[test]
    fn test_xss_script_stripped() {
        let result = render("<script>alert('xss')</script>");
        assert!(!result.contains("<script>"));
        assert!(!result.contains("alert"));
    }

    #[test]
    fn test_xss_event_handler_stripped() {
        let result = render("<div onmouseover=\"alert('xss')\">hover</div>");
        assert!(!result.contains("onmouseover"));
        assert!(!result.contains("alert"));
    }

    #[test]
    fn test_links_preserved() {
        let result = render("[example](https://example.com)");
        assert!(result.contains("<a"));
        assert!(result.contains("https://example.com"));
    }

    #[test]
    fn test_lists() {
        let result = render("- item one\n- item two");
        assert!(result.contains("<li>"));
        assert!(result.contains("item one"));
    }
}
