use slatehub::templates::{BaseContext, IndexTemplate};

#[test]
fn test_base_context() {
    let context = BaseContext::new();
    assert_eq!(context.app_name, "SlateHub");
    assert!(context.user.is_none());
}

#[test]
fn test_base_context_with_page() {
    let context = BaseContext::new().with_page("home");
    assert_eq!(context.active_page, "home");
}

#[test]
fn test_template_creation() {
    let base = BaseContext::new().with_page("index");
    let template = IndexTemplate::new(base);
    assert_eq!(template.active_page, "index");
    assert_eq!(template.app_name, "SlateHub");
}
