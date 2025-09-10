# SlateHub Project Context

## Project Overview
SlateHub is a free, open-source SaaS platform for the TV, film, and content industries. It combines professional networking (like LinkedIn) with project management capabilities (like GitHub), specifically tailored for creative professionals.

## CRITICAL ARCHITECTURAL PRINCIPLES

### 1. STRICT SEPARATION OF CONCERNS
**THIS IS NON-NEGOTIABLE**: HTML, CSS, and JavaScript must be completely separated.

#### HTML (Templates)
- **MUST** contain ONLY semantic markup
- **MUST** use semantic HTML5 elements (`<article>`, `<section>`, `<nav>`, `<header>`, `<footer>`, etc.)
- **MUST** use data attributes for component identification and state
- **MUST** use IDs for unique landmarks and form associations
- **MUST NOT** contain ANY styling classes
- **MUST NOT** contain inline styles
- **MUST NOT** reference visual appearance in any way

#### CSS (Stylesheets)
- **MUST** use ONLY semantic selectors
- **MUST** target elements by their semantic meaning, IDs, and data attributes
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

## HTML STRUCTURE & NAMING CONVENTIONS

### ID Naming Conventions

IDs should be used for:
1. **Page landmarks** - Major sections of the page
2. **Form associations** - Linking labels to inputs and error messages
3. **Navigation targets** - Anchor links and skip navigation
4. **Unique interactive elements** - Elements that need JavaScript interaction

#### ID Naming Patterns:
```
[context]-[element]-[purpose]
```

Examples:
- `#main-nav` - Main navigation
- `#user-menu` - User account menu
- `#profile-header` - Profile page header
- `#form-login` - Login form
- `#input-email` - Email input field
- `#error-email` - Email error message
- `#section-experience` - Experience section
- `#heading-skills` - Skills section heading

### Data Attribute Conventions

Data attributes provide hooks for CSS styling and JavaScript behavior without polluting the semantic structure.

#### Component Identification
Use `data-component` for major reusable components:
```html
<article data-component="profile-card" data-user-id="123">
<section data-component="media-gallery">
<nav data-component="breadcrumb">
```

#### Page/Section Context
Use `data-page` and `data-section` for page-specific styling:
```html
<body data-page="profile">
<main data-section="user-content">
<article data-section="project-details">
```

#### State Management
Use `data-state` for element states:
```html
<article data-state="published">
<button data-state="loading">
<form data-state="submitting">
<div data-state="empty">
```

#### Feature Flags
Use specific data attributes for features:
```html
<article data-featured="true">
<section data-editable="true">
<div data-collapsible="true" data-collapsed="false">
```

#### Content Type
Use `data-type` for content variations:
```html
<article data-type="blog-post">
<article data-type="project">
<button data-type="primary">
<button data-type="danger">
```

#### Role/Purpose
Use `data-role` for semantic purposes (when ARIA roles aren't appropriate):
```html
<div data-role="thumbnail">
<span data-role="badge">
<div data-role="overlay">
```

### Semantic HTML Structure

#### Proper Element Usage

**Article Element (`<article>`)**
- Use ONLY for self-contained, independently distributable content
- Good for: Blog posts, news articles, project cards, user comments
- NOT for: Forms, page sections, navigation, footers, UI components

**Section Element (`<section>`)**
- Use for thematic grouping of content with a heading
- Good for: Page regions, form wrappers, content groups
- Always include a heading (h1-h6) or aria-label

**Div Element (`<div>`)**
- Use for generic containers with no semantic meaning
- Good for: Layout wrappers, styling hooks, JavaScript targets
- Use when article/section aren't semantically appropriate

#### Page Layout Structure
```html
<body data-page="[page-name]" data-user="[authenticated|anonymous]">
    <header id="site-header">
        <nav id="main-nav" aria-label="Main navigation">
            <!-- Logo/Brand -->
            <div data-role="brand">
                <a href="/" id="site-logo">SlateHub</a>
            </div>
            
            <!-- Primary Navigation -->
            <ul data-role="nav-primary">
                <li><a href="/projects" aria-current="page">Projects</a></li>
                <li><a href="/people">People</a></li>
            </ul>
            
            <!-- User Navigation -->
            <ul data-role="nav-user">
                <li data-component="theme-toggle">
                    <button id="theme-toggle" aria-label="Toggle theme">
                        <span data-theme-icon="light">☀️</span>
                        <span data-theme-icon="dark">🌙</span>
                    </button>
                </li>
                <li data-component="user-menu">
                    <details id="user-menu">
                        <summary>User Name</summary>
                        <ul data-role="dropdown-menu">
                            <li><a href="/profile">Profile</a></li>
                            <li><a href="/logout">Logout</a></li>
                        </ul>
                    </details>
                </li>
            </ul>
        </nav>
    </header>
    
    <main id="main-content">
        <!-- Page-specific content -->
    </main>
    
    <footer id="site-footer">
        <section data-role="footer-main">
            <!-- Use divs for footer sections, not articles -->
            <div data-role="footer-brand">
                <h3>Brand Name</h3>
                <p>Description</p>
            </div>
            <div data-role="footer-links">
                <h4>Links</h4>
                <nav><!-- links --></nav>
            </div>
        </section>
    </footer>
</body>
```

#### Form Structure
```html
<!-- Forms should be wrapped in section/div, NOT article -->
<section id="section-[form-name]" data-component="auth-form" data-type="[login|signup|etc]">
    <header data-role="form-header">
        <h2 id="heading-[form]">Form Title</h2>
        <p>Form description</p>
    </header>
    
    <form id="form-[name]" data-component="form" method="post" action="/[endpoint]">
        <fieldset data-role="form-section">
            <legend>Section Title</legend>
            
            <div data-field="email">
                <label for="input-email">Email</label>
                <input 
                    type="email" 
                    id="input-email" 
                    name="email"
                    required
                    aria-invalid="false"
                    aria-describedby="help-email error-email"
                >
                <small id="help-email" data-role="help-text">
                    Enter your email address
                </small>
                <div id="error-email" role="alert" data-role="error-message" hidden>
                    <!-- Error message -->
                </div>
            </div>
        </fieldset>
        
        <div data-role="form-actions">
            <button type="submit" data-type="primary">Submit</button>
            <button type="button" data-type="secondary">Cancel</button>
        </div>
    </form>
</section>
```

#### Content Card Structure
```html
<!-- Use article ONLY for self-contained, redistributable content -->
<article data-component="content-card" data-type="[project|post|news]" data-id="[id]">
    <header data-role="card-header">
        <h3 id="card-title-[id]">Title</h3>
        <div data-role="metadata">
            <time datetime="2024-01-01">January 1, 2024</time>
            <span data-role="author">Author Name</span>
        </div>
    </header>
    
    <div data-role="card-body">
        <!-- Content -->
    </div>
    
    <footer data-role="card-footer">
        <nav data-role="card-actions">
            <button type="button" data-action="like">Like</button>
            <button type="button" data-action="share">Share</button>
        </nav>
    </footer>
</article>

<!-- For non-article content items like experience entries, use div -->
<div data-component="experience-item" data-id="[id]">
    <header data-role="item-header">
        <h3 id="item-title-[id]">Role Title</h3>
        <time>2024</time>
    </header>
    <div data-role="item-body">
        <!-- Description -->
    </div>
</div>
```

#### Modal/Dialog Structure
```html
<dialog id="modal-[name]" data-component="modal" aria-labelledby="modal-title-[name]">
    <div data-role="modal-content">
        <header data-role="modal-header">
            <h2 id="modal-title-[name]">Modal Title</h2>
            <button type="button" data-action="close" aria-label="Close modal">×</button>
        </header>
        
        <div data-role="modal-body">
            <!-- Content -->
        </div>
        
        <footer data-role="modal-footer">
            <button type="button" data-type="primary">Confirm</button>
            <button type="button" data-type="secondary" data-action="cancel">Cancel</button>
        </footer>
    </div>
</dialog>
```

#### List/Grid Structure
```html
<section data-component="content-list" data-layout="[grid|list]">
    <header data-role="section-header">
        <h2 id="heading-[section]">Section Title</h2>
        <nav data-role="filters">
            <button type="button" data-filter="all" aria-pressed="true">All</button>
            <button type="button" data-filter="recent" aria-pressed="false">Recent</button>
        </nav>
    </header>
    
    <div data-role="content-container" data-state="[loading|empty|error|ready]">
        <!-- When empty -->
        <div data-role="empty-state" data-state="empty">
            <p>No items found</p>
        </div>
        
        <!-- When has content -->
        <ul data-role="item-list">
            <li data-item="true" data-item-id="[id]">
                <!-- Item content -->
            </li>
        </ul>
    </div>
    
    <footer data-role="section-footer">
        <nav data-role="pagination" aria-label="Pagination">
            <a href="?page=1" aria-current="page">1</a>
            <a href="?page=2">2</a>
        </nav>
    </footer>
</section>
```

### CSS Selector Examples

#### Basic Element Selectors
```css
/* Page-specific styling */
[data-page="profile"] main { }
[data-page="projects"] [data-section="filters"] { }

/* Component styling */
[data-component="content-card"] { }
[data-component="content-card"][data-type="featured"] { }

/* State-based styling */
[data-state="loading"] { }
[data-state="error"] { }
form[data-state="submitting"] button[type="submit"] { }

/* Navigation states */
nav a[aria-current="page"] { }
button[aria-pressed="true"] { }
details[open] > summary { }

/* Form states */
input[aria-invalid="true"] { }
input:required:valid { }
[data-role="error-message"]:not([hidden]) { }

/* Layout variations */
[data-layout="grid"] { }
[data-layout="list"] { }

/* Responsive containers */
[data-component="content-list"][data-layout="grid"] {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
}
```

### Common UI Patterns

#### Tabs
```html
<div data-component="tabs" role="tablist" aria-label="Section Tabs">
    <button role="tab" id="tab-1" aria-selected="true" aria-controls="panel-1">
        Tab 1
    </button>
    <button role="tab" id="tab-2" aria-selected="false" aria-controls="panel-2">
        Tab 2
    </button>
</div>
<div role="tabpanel" id="panel-1" aria-labelledby="tab-1">
    <!-- Panel 1 content -->
</div>
<div role="tabpanel" id="panel-2" aria-labelledby="tab-2" hidden>
    <!-- Panel 2 content -->
</div>
```

#### Accordion
```html
<div data-component="accordion">
    <details data-accordion-item="true">
        <summary id="accordion-header-1">Section 1</summary>
        <div data-role="accordion-content">
            <!-- Content -->
        </div>
    </details>
    <details data-accordion-item="true">
        <summary id="accordion-header-2">Section 2</summary>
        <div data-role="accordion-content">
            <!-- Content -->
        </div>
    </details>
</div>
```

#### Notifications/Alerts
```html
<div data-component="notification" data-type="[success|warning|error|info]" role="alert">
    <div data-role="notification-content">
        <strong id="notification-title">Title</strong>
        <p>Message content</p>
    </div>
    <button type="button" data-action="dismiss" aria-label="Dismiss notification">×</button>
</div>
```

## Template Implementation Checklist

When creating or updating templates, ensure:

### Structure
- [ ] Every major section has a unique ID
- [ ] All form inputs have associated labels via ID
- [ ] Error messages are linked to inputs via aria-describedby
- [ ] Major components use data-component attribute
- [ ] Page context is set with data-page on body
- [ ] Sections use data-section for context

### Navigation
- [ ] Current page uses aria-current="page"
- [ ] Navigation sections have aria-label
- [ ] Skip links are provided for keyboard navigation
- [ ] Dropdown menus use details/summary or proper ARIA

### Forms
- [ ] Each form has a unique ID
- [ ] All inputs have unique IDs
- [ ] Labels are properly associated with inputs
- [ ] Error messages have role="alert"
- [ ] Required fields are marked with required attribute
- [ ] Invalid fields use aria-invalid
- [ ] Help text is linked via aria-describedby

### Interactive Elements
- [ ] Buttons specify type attribute
- [ ] Toggle buttons use aria-pressed
- [ ] Expandable sections use aria-expanded
- [ ] Loading states are indicated with data-state
- [ ] Modals use dialog element or proper ARIA

### Content
- [ ] Headings follow proper hierarchy
- [ ] Lists use appropriate ul/ol/dl elements
- [ ] Time elements use datetime attribute
- [ ] Images have alt text
- [ ] Links to external sites use rel="noopener"

## CSS Organization

```
/static/css/
├── base/
│   ├── reset.css         # CSS reset/normalize
│   ├── variables.css     # CSS custom properties
│   └── typography.css    # Base typography
├── layout/
│   ├── grid.css         # Grid system
│   ├── header.css       # Site header
│   ├── footer.css       # Site footer
│   └── navigation.css   # Navigation components
├── components/
│   ├── cards.css        # Content cards
│   ├── forms.css        # Form styling
│   ├── buttons.css      # Button variations
│   ├── modals.css       # Modal dialogs
│   └── tables.css       # Data tables
├── pages/
│   ├── profile.css      # Profile page specific
│   ├── projects.css     # Projects page specific
│   └── dashboard.css    # Dashboard specific
└── themes/
    ├── light.css        # Light theme overrides
    └── dark.css         # Dark theme overrides
```

## Design Flexibility Guidelines

### For Designers

With this structure, you can:

1. **Target any component** using data-component selectors
2. **Style different states** using data-state attributes
3. **Create variations** using data-type attributes
4. **Scope styles to pages** using data-page on body
5. **Target specific sections** using IDs and data-section
6. **Style interactive states** using ARIA attributes
7. **Create responsive layouts** using data-layout attributes

### What You Cannot Do

1. Add new HTML elements (structure is fixed)
2. Add CSS classes to HTML
3. Modify HTML attributes (except through JavaScript)
4. Change the semantic structure

### CSS-Only Interactions You Can Create

1. **Hover effects** - Using :hover pseudo-class
2. **Focus styles** - Using :focus and :focus-visible
3. **Active states** - Using :active pseudo-class
4. **Checked states** - Using :checked for inputs
5. **Empty states** - Using :empty pseudo-class
6. **Target states** - Using :target for anchors
7. **Valid/Invalid** - Using :valid/:invalid for forms
8. **Open states** - Using [open] for details elements

## Testing & Validation

Before committing any HTML/template changes:

### Structure Validation
- [ ] HTML validates with W3C validator
- [ ] All IDs are unique
- [ ] All forms have proper associations
- [ ] ARIA attributes are used correctly
- [ ] Data attributes follow naming conventions

### Accessibility
- [ ] Keyboard navigation works
- [ ] Screen reader navigation is logical
- [ ] Color contrast meets WCAG standards
- [ ] Focus indicators are visible
- [ ] Error messages are announced

### CSS Independence
- [ ] No CSS classes used for styling
- [ ] No inline styles
- [ ] All styling in external CSS files
- [ ] CSS uses only semantic selectors
- [ ] No JavaScript manipulation of styles

## AI Tool Instructions

When asked to modify or create templates:

1. **ALWAYS** follow the ID naming convention: `[context]-[element]-[purpose]`
2. **ALWAYS** use data-component for major reusable components
3. **ALWAYS** use data-state for stateful elements
4. **ALWAYS** use data-type for variations
5. **ALWAYS** use data-page on body element
6. **ALWAYS** provide proper ARIA attributes
7. **NEVER** add CSS classes for styling
8. **NEVER** use inline styles
9. **NEVER** create non-semantic wrapper divs
10. **ALWAYS** ensure forms have proper ID associations

## Red Flags to Avoid

If you see any of these in code, it needs to be fixed:

### Class-based Styling (FORBIDDEN)
- `class="error"`, `class="success"`, `class="warning"`
- `class="button primary"`, `class="btn-large"`
- `class="card"`, `class="modal"`, `class="dropdown"`
- Any Bootstrap, Tailwind, or other CSS framework classes

### Inline Styles (FORBIDDEN)
- `style="color: red"`
- `style="display: none"`
- Any style attribute

### Missing Structure (MUST FIX)
- Forms without IDs
- Inputs without IDs
- Labels without for attributes
- Missing aria-describedby for error messages
- Components without data-component
- Pages without data-page on body

### Semantic Misuse (MUST FIX)
- Using `<article>` for forms, footers, or UI components
- Using `<section>` without headings
- Missing semantic HTML5 elements where appropriate
- Using divs when semantic elements exist

### Poor Naming (MUST FIX)
- Generic IDs like `id="1"`, `id="div1"`
- Inconsistent naming patterns
- Missing data attributes for state
- No data-type for variations

Remember: The goal is to provide designers with maximum flexibility through CSS while maintaining clean, semantic, accessible HTML that never changes. Every element that might need styling should have a predictable selector through IDs, data attributes, or ARIA attributes.