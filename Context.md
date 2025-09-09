# SlateHub Project Context

## Project Overview
SlateHub is a free, open-source SaaS platform for the TV, film, and content industries. It combines professional networking (like LinkedIn) with project management capabilities (like GitHub), specifically tailored for creative professionals.

## CRITICAL ARCHITECTURAL PRINCIPLES

### 1. STRICT SEPARATION OF CONCERNS
**THIS IS NON-NEGOTIABLE**: HTML, CSS, and JavaScript must be completely separated.

#### HTML (Templates)
- **MUST** contain ONLY semantic markup
- **MUST** use semantic HTML5 elements (`<article>`, `<section>`, `<nav>`, `<header>`, `<footer>`, etc.)
- **MUST** use data attributes ONLY for JavaScript functionality (e.g., `data-tab="networking"`)
- **MUST NOT** contain ANY styling classes (no `class="button primary"`, no `class="error-message"`)
- **MUST NOT** contain inline styles
- **MUST NOT** reference visual appearance in any way

#### CSS (Stylesheets)
- **MUST** use ONLY semantic selectors
- **MUST** target elements by their semantic meaning, not by classes
- **MUST** use attribute selectors for state (e.g., `[aria-expanded="true"]`)
- **MUST** use pseudo-classes for interaction states (`:hover`, `:focus`, `:active`)
- **MUST** live in separate CSS files under `/static/css/`

#### JavaScript
- **MUST** be vanilla JavaScript (no frameworks)
- **MUST** use semantic selectors to find elements
- **MUST** manipulate state through data attributes and ARIA attributes
- **MUST NOT** add or remove CSS classes for styling

### 2. SERVER-SIDE RENDERING ONLY
- **NO** client-side frameworks (React, Vue, Angular, etc.)
- **NO** client-side state management libraries
- **NO** build tools for JavaScript (no webpack, no bundlers)
- **NO** datastar.js or similar reactive libraries
- **NO** Server-Sent Events (SSE) or WebSockets for UI updates
- All dynamic content must be rendered server-side using Tera templates

### 3. SEMANTIC HTML EXAMPLES

#### ❌ WRONG (Never do this):
```html
<div class="error-message">Invalid username or password</div>
<button class="button primary large">Submit</button>
<div class="card featured">Content</div>
```

#### ✅ CORRECT (Always do this):
```html
<div role="alert" aria-live="polite">Invalid username or password</div>
<button type="submit">Submit</button>
<article data-featured="true">Content</article>
```

### 4. CSS SELECTOR EXAMPLES

#### ❌ WRONG (Never do this):
```css
.error-message { color: red; }
.button.primary { background: blue; }
.card.featured { border: 2px solid gold; }
```

#### ✅ CORRECT (Always do this):
```css
[role="alert"] { color: var(--color-error); }
button[type="submit"] { background: var(--color-primary); }
article[data-featured="true"] { border: 2px solid var(--color-accent); }
```

## Technology Stack

### Backend
- **Language**: Rust
- **Web Framework**: Axum
- **Template Engine**: Tera
- **Database**: SurrealDB
- **Authentication**: SurrealDB built-in ACCESS system
- **File Storage**: MinIO (S3-compatible)

### Frontend
- **HTML**: Semantic HTML5 only
- **CSS**: Pico CSS framework (provides semantic styling)
- **JavaScript**: Vanilla JavaScript only
- **No Build Tools**: Direct file serving

## Project Structure
```
slatehub/
├── server/                 # Rust backend
│   ├── src/
│   │   ├── routes/        # HTTP route handlers
│   │   ├── models/        # Data models
│   │   ├── db.rs          # Database connection
│   │   └── templates.rs   # Template rendering
│   ├── templates/         # Tera HTML templates
│   └── static/            # Static assets
│       ├── css/          # Stylesheets
│       └── js/           # JavaScript files
├── db/                    # Database files
│   └── schema.surql      # SurrealDB schema
└── docker-compose.yml    # Services configuration
```

## Database Configuration
- **Namespace**: `slatehub` (from DB_NAMESPACE env var)
- **Database**: `main` (from DB_NAME env var)
- **Access Scope**: `user` (for authentication)

## Development Guidelines

### When Adding New Features
1. Start with semantic HTML structure
2. Style using existing semantic selectors in CSS
3. Add JavaScript behavior last, if needed
4. Never add classes for styling purposes
5. Use data attributes for JavaScript hooks
6. Use ARIA attributes for accessibility and state

### Form Handling
- Use traditional HTML form submission
- Handle forms with POST requests server-side
- Return full HTML pages with results
- Show errors using semantic markup with ARIA roles

### Template Guidelines
- Use Tera template inheritance (`{% extends "_layout.html" %}`)
- Pass all dynamic data from server-side
- Never generate HTML in JavaScript
- Use semantic HTML elements for structure

### CSS Organization
```
/static/css/
├── slatehub-pico.css    # Overrides for Pico CSS
├── components.css        # Component-specific styles
└── pages.css            # Page-specific styles
```

All CSS must target semantic elements:
- Form elements by type: `input[type="email"]`
- Buttons by purpose: `button[type="submit"]`
- Sections by role: `[role="navigation"]`
- Content by data attributes: `[data-status="active"]`

### Error Handling
- Display errors using semantic HTML with proper ARIA roles
- Never use error-specific CSS classes
- Use `role="alert"` for error messages
- Use `aria-invalid="true"` for invalid form fields

## Common Patterns

### Navigation State
```html
<!-- HTML -->
<nav>
  <a href="/projects" aria-current="page">Projects</a>
  <a href="/people">People</a>
</nav>
```
```css
/* CSS */
nav a[aria-current="page"] {
  font-weight: bold;
  color: var(--color-primary);
}
```

### Form Validation
```html
<!-- HTML -->
<input type="email" 
       name="email" 
       required 
       aria-invalid="true"
       aria-describedby="email-error">
<div id="email-error" role="alert">Invalid email address</div>
```
```css
/* CSS */
input[aria-invalid="true"] {
  border-color: var(--color-error);
}
[role="alert"] {
  color: var(--color-error);
}
```

### Content States
```html
<!-- HTML -->
<article data-status="published" data-featured="true">
  <h2>Article Title</h2>
</article>
```
```css
/* CSS */
article[data-status="published"] {
  opacity: 1;
}
article[data-featured="true"] {
  border: 2px solid var(--color-accent);
}
```

## Testing Checklist
Before committing any HTML/template changes:
- [ ] No class attributes used for styling
- [ ] All styling is in separate CSS files
- [ ] HTML uses semantic elements
- [ ] Forms work without JavaScript
- [ ] ARIA attributes are used for accessibility
- [ ] Data attributes are used for JS hooks only
- [ ] No inline styles
- [ ] No style-related class names

## AI Tool Instructions
When asked to modify or create templates:
1. NEVER add CSS classes for styling
2. ALWAYS use semantic HTML elements
3. ALWAYS keep styles in separate CSS files
4. NEVER use client-side frameworks
5. ALWAYS handle forms server-side
6. NEVER use style-descriptive names (like "error-message", "primary-button")
7. ALWAYS use data attributes for JavaScript functionality
8. ALWAYS use ARIA attributes for accessibility and state

## Red Flags to Avoid
If you see any of these in code, it needs to be fixed:
- `class="error"`, `class="success"`, `class="warning"`
- `class="button primary"`, `class="btn-large"`
- `class="card"`, `class="modal"`, `class="dropdown"`
- `style="..."` (inline styles)
- `className` (React/JSX syntax)
- `v-if`, `v-for`, `@click` (Vue syntax)  
- `*ngIf`, `*ngFor` (Angular syntax)
- `data-on-click`, `data-signals` (Datastar syntax)
- Any CSS framework classes (Bootstrap, Tailwind, etc.)

Remember: The goal is complete separation of structure, presentation, and behavior. HTML defines WHAT it is, CSS defines HOW it looks, and JavaScript defines WHAT it does.