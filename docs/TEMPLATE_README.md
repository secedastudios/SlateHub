# SlateHub Templates Documentation

This directory contains all Tera templates for the SlateHub web application. We use [Tera](https://tera.netlify.app/), a powerful template engine for Rust inspired by Jinja2 and Django templates.

## Directory Structure

```
templates/
├── _layout.html        # Base layout template (inherited by all pages)
├── index.html          # Homepage template
├── projects.html       # Projects listing page
├── people.html         # People/professionals listing page
├── about.html          # About page
├── macros.html         # Reusable macro functions
└── partials/           # Reusable template components
    └── card.html       # Example card component
```

## Template Inheritance

Tera uses template inheritance to avoid duplication. All page templates extend from `_layout.html`:

```html
{% extends "_layout.html" %}

{% block title %}Page Title{% endblock %}

{% block content %}
    <!-- Page content here -->
{% endblock %}
```

### Available Blocks in _layout.html

- `title` - Page title (appears in browser tab)
- `description` - Meta description for SEO
- `head` - Additional head elements (CSS, meta tags)
- `content` - Main page content
- `scripts` - Additional JavaScript at the end of body

## Using Macros

Macros are reusable template functions. Import and use them like this:

```html
{% import "macros.html" as macros %}

<!-- Using a button macro -->
{{ macros::button(text="Click Me", type="primary", href="/action") }}

<!-- Using a card macro -->
{{ macros::project_card(project=project_data) }}
```

### Available Macros

- `button` - Renders a button or link styled as button
- `input` - Form input with label and error handling
- `alert` - Alert/notification messages
- `avatar` - User avatar with optional status indicator
- `nav_item` - Navigation menu item
- `pagination` - Pagination controls
- `loading` - Loading state indicator
- `empty_state` - Empty state message
- `project_card` - Project display card
- `person_card` - Person/professional display card
- `breadcrumb` - Breadcrumb navigation
- `stat_card` - Statistics display card
- `modal` - Modal dialog
- `tag_list` - List of tags/labels
- `rating` - Star rating display
- `search_bar` - Search input form
- `social_links` - Social media links

## Using Partials (Includes)

Partials are template snippets that can be included in other templates:

```html
<!-- Include a partial -->
{% include "partials/card.html" %}

<!-- Include with context variables -->
{% set card_title = "My Card" %}
{% set card_content = "Card content here" %}
{% include "partials/card.html" %}
```

## Template Context Variables

All templates have access to these base context variables (set in `src/templates.rs`):

- `app_name` - Application name ("SlateHub")
- `version` - Application version
- `year` - Current year
- `active_page` - Currently active page for navigation highlighting
- `user` - Current user data (if authenticated)

## Best Practices

### 1. Template Organization

- Prefix layout templates with underscore (e.g., `_layout.html`)
- Keep page templates at the root level
- Group reusable components in `partials/`
- Put all macros in `macros.html` or separate macro files

### 2. Avoid Hardcoding HTML in Routes

❌ **Don't do this:**
```rust
async fn page() -> Html<String> {
    let html = r#"<!DOCTYPE html><html>...</html>"#;
    Ok(Html(html.to_string()))
}
```

✅ **Do this instead:**
```rust
async fn page() -> Result<Html<String>, Error> {
    let mut context = templates::base_context();
    context.insert("active_page", "page_name");
    
    let html = templates::render_with_context("page.html", &context)?;
    Ok(Html(html))
}
```

### 3. Component Reusability

When you find yourself repeating HTML structures:

1. **For simple includes:** Create a partial
2. **For parameterized components:** Create a macro
3. **For complex components:** Consider a macro with structured data

### 4. Styling Best Practices

- Use scoped styles within templates when needed
- Leverage CSS custom properties (CSS variables) for theming
- Keep component styles with their templates
- Global styles should be in `/static/css/slatehub.css`

### 5. JavaScript Integration

We use [Datastar](https://datastar.fly.dev/) for reactive UI components:

```html
<!-- Reactive store -->
<div data-store="{count: 0}">
    <button data-on-click="$count++">Count: <span data-text="$count">0</span></button>
</div>

<!-- Conditional rendering -->
<div data-show="$isVisible">
    This is conditionally visible
</div>

<!-- Event handling -->
<button data-on-click="$handleClick">Click me</button>
```

### 6. Error Handling

Templates can fail to render. Always handle template errors in routes:

```rust
let html = templates::render_with_context("template.html", &context)
    .map_err(|e| {
        error!("Failed to render template: {}", e);
        Error::template(e.to_string())
    })?;
```

### 7. Performance Tips

- Use `{% include %}` sparingly - prefer macros for better performance
- Minimize template nesting depth
- Cache compiled templates in production (handled by `LazyLock<Tera>`)
- Avoid complex logic in templates - prepare data in Rust

### 8. Security

- Always escape user input (Tera does this by default)
- Use `| safe` filter only for trusted content
- Never put user input directly into JavaScript code
- Use data attributes for passing data to JavaScript

## Template Filters

Tera provides many built-in filters:

```html
{{ value | upper }}              <!-- Uppercase -->
{{ text | truncate(length=50) }} <!-- Truncate text -->
{{ html | safe }}                <!-- Mark as safe HTML -->
{{ date | date(format="%Y-%m-%d") }} <!-- Format date -->
{{ list | first }}               <!-- Get first item -->
{{ number | round }}             <!-- Round number -->
```

## Debugging Templates

1. **Check template loading:**
   ```rust
   // In src/templates.rs
   debug!("Loaded templates: {:?}", TEMPLATES.get_template_names().collect::<Vec<_>>());
   ```

2. **Debug context variables:**
   ```html
   <!-- In template -->
   <pre>{{ __tera_context | json_encode(pretty=true) | safe }}</pre>
   ```

3. **Check for syntax errors:**
   Templates are validated at startup. Check logs for parsing errors.

## Adding New Pages

1. Create a new template file (e.g., `new_page.html`)
2. Extend the base layout:
   ```html
   {% extends "_layout.html" %}
   {% block title %}New Page - {{ app_name }}{% endblock %}
   {% block content %}
       <!-- Your content -->
   {% endblock %}
   ```

3. Add a route handler:
   ```rust
   async fn new_page() -> Result<Html<String>, Error> {
       let mut context = templates::base_context();
       context.insert("active_page", "new_page");
       
       let html = templates::render_with_context("new_page.html", &context)?;
       Ok(Html(html))
   }
   ```

4. Register the route in `routes/mod.rs`:
   ```rust
   .route("/new-page", get(new_page))
   ```

## Resources

- [Tera Documentation](https://tera.netlify.app/docs/)
- [Tera Template Syntax](https://tera.netlify.app/docs/#templates)
- [Datastar Documentation](https://datastar.fly.dev/)
- [Semantic CSS Patterns](https://semantic-ui.com/)