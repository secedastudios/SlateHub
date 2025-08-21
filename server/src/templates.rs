use chrono::Datelike;
use serde::Serialize;
use std::sync::LazyLock;
use tera::{Context, Tera};
use tracing::{debug, error, info};

// Global Tera instance
pub static TEMPLATES: LazyLock<Tera> = LazyLock::new(|| {
    let template_dir = "templates/**/*";
    match Tera::new(template_dir) {
        Ok(mut tera) => {
            // Register custom filters if needed
            tera.autoescape_on(vec![".html", ".htm", ".xml"]);
            info!("Templates loaded from: {}", template_dir);
            debug!(
                "Loaded templates: {:?}",
                tera.get_template_names().collect::<Vec<_>>()
            );
            tera
        }
        Err(e) => {
            error!("Failed to parse templates: {}", e);
            panic!("Template parsing error: {}", e);
        }
    }
});

/// Render a template with the given context
pub fn render<T: Serialize>(template_name: &str, data: &T) -> Result<String, tera::Error> {
    let context = Context::from_serialize(data)?;
    render_with_context(template_name, &context)
}

/// Render a template with a Tera context
pub fn render_with_context(template_name: &str, context: &Context) -> Result<String, tera::Error> {
    debug!("Rendering template: {}", template_name);
    TEMPLATES.render(template_name, context)
}

/// Create a base context with common data
pub fn base_context() -> Context {
    let mut context = Context::new();

    // Add common data that all templates might need
    context.insert("app_name", "SlateHub");
    context.insert("year", &chrono::Utc::now().year());

    // Add version info
    context.insert("version", env!("CARGO_PKG_VERSION"));

    context
}

/// Helper to create a context with base values and custom data
pub fn context_with<T: Serialize>(data: T) -> Result<Context, tera::Error> {
    let mut context = base_context();

    // Merge the custom data into the base context
    if let Ok(value) = tera::to_value(data) {
        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                context.insert(key, val);
            }
        }
    }

    Ok(context)
}

/// Initialize templates (called during startup to verify all templates load)
pub fn init() -> Result<(), String> {
    // Force lazy static initialization
    let _ = &*TEMPLATES;

    // Verify required templates exist
    let required_templates = vec!["_layout.html", "index.html"];

    for template in required_templates {
        if !TEMPLATES.get_template_names().any(|name| name == template) {
            return Err(format!("Required template '{}' not found", template));
        }
    }

    info!("Template system initialized successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_context() {
        let context = base_context();
        assert_eq!(
            context.get("app_name").unwrap().as_str().unwrap(),
            "SlateHub"
        );
    }
}
