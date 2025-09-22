# HTML & CSS Guidelines for SlateHub

## Overview

SlateHub uses a **strict separation of concerns** architecture where HTML defines semantic structure, CSS controls all visual presentation, and JavaScript handles behavior. This document provides comprehensive guidelines for maintaining this separation and creating maintainable, accessible, and designer-friendly code.

## Core Principles

### 1. Complete Separation of Concerns
- **HTML**: Semantic structure and content only
- **CSS**: All visual presentation and layout
- **JavaScript**: Behavior and interactivity only
- **NO CSS classes for styling** - Use semantic selectors instead

### 2. Semantic HTML First
- Use proper HTML5 semantic elements (`article`, `section`, `nav`, `header`, `footer`, `aside`, `main`)
- Choose elements based on meaning, not appearance
- Maintain proper document outline and hierarchy

### 3. Predictable and Consistent Selectors
- Every element has a predictable selector pattern
- IDs for unique page elements
- Data attributes for components and states
- ARIA attributes for accessibility and state

## Naming Conventions

### ID Naming Pattern: `[context]-[element]-[purpose]`

#### Page Landmarks
```html
#site-header        <!-- Main site header -->
#main-nav          <!-- Main navigation -->
#main-content      <!-- Main content area -->
#site-footer       <!-- Main site footer -->
```

#### Section Headers
```html
#section-[name]    <!-- Section container -->
#heading-[name]    <!-- Section heading -->

<!-- Examples: -->
#section-projects
#heading-projects
#section-about
#heading-about
```

#### Form Elements
```html
#form-[name]              <!-- Form container -->
#fieldset-[name]          <!-- Fieldset grouping -->
#field-[name]             <!-- Field container -->
#input-[name]             <!-- Input element -->
#select-[name]            <!-- Select element -->
#textarea-[name]          <!-- Textarea element -->
#button-[action]-[context] <!-- Button element -->
#error-[field]            <!-- Field error message -->
#help-[field]             <!-- Field help text -->
#alert-[context]-[type]   <!-- Alert messages -->

<!-- Examples: -->
#form-login
#fieldset-credentials
#field-email
#input-email
#error-email
#help-email
#button-submit-login
#alert-login-error
```

#### Navigation Elements
```html
#nav-[context]           <!-- Navigation container -->
#link-[destination]      <!-- Important links -->

<!-- Examples: -->
#nav-footer-links
#link-privacy
#link-terms
```

### Data Attributes

#### `data-component` - Major Reusable Components
```html
<article data-component="project-card">
<section data-component="auth-form">
<nav data-component="breadcrumb">
<div data-component="search-bar">
<form data-component="filter-form">
```

#### `data-page` - Page Context (on body)
```html
<body data-page="profile">
<body data-page="projects">
<body data-page="login">
<body data-page="signup">
```

#### `data-section` - Page Sections
```html
<section data-section="controls">
<section data-section="user-content">
<section data-section="overview">
<div data-section="filters">
```

#### `data-role` - Semantic Roles Within Components
```html
<!-- Common roles -->
<header data-role="page-header">
<div data-role="card-header">
<div data-role="card-body">
<footer data-role="card-footer">
<nav data-role="card-actions">
<div data-role="empty-state">
<div data-role="form-actions">
<ul data-role="link-list">
<p data-role="subtitle">
<small data-role="help-text">
```

#### `data-field` - Form Field Identifiers
```html
<div data-field="email">
<div data-field="password">
<dt data-field="location">
```

#### `data-state` - Element States
```html
<form data-state="submitting">
<section data-state="loading">
<div data-state="empty">
<article data-state="expanded">
<button data-state="active">
```

#### `data-type` - Variations
```html
<button data-type="primary">
<button data-type="secondary">
<button data-type="danger">
<article data-type="featured">
<div data-type="warning">
```

#### `data-status` - Status Indicators
```html
<article data-status="published">
<span data-status="active">
<div data-status="pending">
```

#### `data-layout` - Layout Types
```html
<section data-layout="grid">
<div data-layout="flex">
<article data-layout="card">
```

#### `data-user` - Authentication State (on body)
```html
<body data-user="authenticated">
<body data-user="anonymous">
```

## HTML Structure Templates

### Page Structure
```html
<!doctype html>
<html lang="en" data-theme="light">
<head>
    <!-- meta tags -->
</head>
<body data-page="[page-name]" data-user="[authenticated|anonymous]">
    <header id="site-header">
        <nav id="main-nav" aria-label="Main navigation">
            <ul data-role="nav-brand">
                <li><a href="/" id="site-logo">SlateHub</a></li>
            </ul>
            <ul data-role="nav-primary">
                <!-- Primary navigation items -->
            </ul>
            <ul data-role="nav-user">
                <!-- User menu items -->
            </ul>
        </nav>
    </header>

    <main id="main-content">
        <!-- Page content -->
    </main>

    <footer id="site-footer">
        <!-- Footer content -->
    </footer>
</body>
</html>
```

### Form Structure
```html
<section id="section-[name]" data-component="auth-form" data-type="[login|signup]">
    <header id="[name]-header" data-role="form-header">
        <hgroup>
            <h1 id="heading-[name]">Title</h1>
            <p data-role="subtitle">Subtitle text</p>
        </hgroup>
    </header>

    <form id="form-[name]" method="post" action="/[action]" data-state="ready">
        <fieldset id="fieldset-[group]" data-role="form-section">
            <legend>Group Title</legend>
            
            <div id="field-[name]" data-field="[name]">
                <label for="input-[name]">Label</label>
                <input 
                    id="input-[name]" 
                    name="[name]"
                    type="[type]"
                    aria-required="true"
                    aria-describedby="help-[name] error-[name]"
                    aria-invalid="false"
                />
                <small id="help-[name]" data-role="help-text">
                    Help text
                </small>
                <div id="error-[name]" role="alert" data-role="error-message" hidden>
                    Error message
                </div>
            </div>
        </fieldset>

        <div id="[name]-actions" data-role="form-actions">
            <button type="submit" id="button-submit-[name]" data-type="primary">
                Submit
            </button>
        </div>
    </form>
</section>
```

### Content Card Structure
```html
<article 
    id="[type]-[id]"
    data-component="[type]-card" 
    data-status="[status]"
    data-[type]-id="[id]"
>
    <header data-role="card-header">
        <h2 id="[type]-title-[id]">Title</h2>
        <span data-role="status-badge" data-status="[status]">
            Status
        </span>
    </header>

    <div data-role="card-body">
        <p data-role="description">Description</p>
        
        <dl data-role="metadata">
            <dt>Label</dt>
            <dd data-field="[field]">Value</dd>
        </dl>

        <ul data-role="tag-list">
            <li data-tag="[value]">Tag</li>
        </ul>
    </div>

    <footer data-role="card-footer">
        <nav data-role="card-actions">
            <a href="#" role="button" data-type="primary">Action</a>
        </nav>
    </footer>
</article>
```

### List/Grid Container
```html
<section id="section-[name]" data-section="[name]" data-layout="grid">
    <header id="[name]-header" data-role="section-header">
        <h2 id="heading-[name]">Section Title</h2>
        <p data-role="description">Description</p>
    </header>

    <!-- When content exists -->
    <div data-role="content-container" data-state="ready">
        <!-- Cards or list items -->
    </div>

    <!-- When empty -->
    <div id="empty-state-[name]" data-role="empty-state" data-state="empty">
        <h3 id="heading-empty-[name]">No items found</h3>
        <p data-role="empty-message">Helpful message</p>
        <nav data-role="empty-state-actions">
            <a href="#" role="button" data-type="primary">Call to Action</a>
        </nav>
    </div>
</section>
```

## CSS Selector Patterns

### Basic Selectors
```css
/* Page context */
[data-page="profile"] { }

/* Component targeting */
[data-component="project-card"] { }

/* ID targeting for unique elements */
#site-header { }
#main-nav { }
#heading-projects { }

/* State-based styling */
[data-state="loading"] { }
[data-state="empty"] { }
[aria-expanded="true"] { }
[aria-current="page"] { }
[aria-invalid="true"] { }

/* Type variations */
[data-type="primary"] { }
[data-type="danger"] { }

/* Status indicators */
[data-status="active"] { }
[data-status="pending"] { }

/* Layout variations */
[data-layout="grid"] { }
[data-layout="flex"] { }
```

### Nested Selectors
```css
/* Component parts */
[data-component="project-card"] [data-role="card-header"] { }
[data-component="project-card"] [data-role="card-body"] { }
[data-component="project-card"] [data-role="card-footer"] { }

/* Form fields */
[data-component="auth-form"] [data-field="email"] { }
[data-component="auth-form"] #input-email { }
[data-component="auth-form"] #error-email { }

/* Page-specific component styling */
[data-page="profile"] [data-component="project-card"] { }
```

### Contextual Selectors
```css
/* User state variations */
[data-user="authenticated"] [data-role="nav-user"] { }
[data-user="anonymous"] [data-role="nav-user"] { }

/* Theme variations */
[data-theme="dark"] [data-component="card"] { }
[data-theme="light"] [data-component="card"] { }

/* Responsive states */
@media (max-width: 768px) {
    [data-layout="grid"] { }
}
```

## CSS File Organization

```
/static/css/
├── base/
│   ├── reset.css           # Normalize/reset styles
│   ├── variables.css       # CSS custom properties
│   ├── typography.css      # Base typography
│   └── utilities.css       # Utility styles
├── layout/
│   ├── grid.css           # Grid layouts
│   ├── header.css         # Site header
│   ├── footer.css         # Site footer
│   └── navigation.css     # Navigation components
├── components/
│   ├── alerts.css         # Alert messages
│   ├── buttons.css        # Button styles
│   ├── cards.css          # Card components
│   ├── forms.css          # Form styling
│   ├── modals.css         # Modal dialogs
│   └── tables.css         # Table styling
├── pages/
│   ├── profile.css        # Profile page specific
│   ├── projects.css       # Projects page specific
│   ├── people.css         # People page specific
│   └── equipment.css      # Equipment page specific
├── themes/
│   ├── light.css          # Light theme
│   └── dark.css           # Dark theme
└── main.css               # Main import file
```

## Component Examples

### Status Badge
```html
<span 
    id="status-[context]-[id]"
    data-role="status-badge" 
    data-status="active"
>
    Active
</span>
```

```css
[data-role="status-badge"] {
    padding: 0.25rem 0.75rem;
    border-radius: 1rem;
    font-size: 0.875rem;
    font-weight: 500;
}

[data-role="status-badge"][data-status="active"] {
    background: var(--color-success-bg);
    color: var(--color-success-text);
}

[data-role="status-badge"][data-status="pending"] {
    background: var(--color-warning-bg);
    color: var(--color-warning-text);
}

[data-role="status-badge"][data-status="inactive"] {
    background: var(--color-muted-bg);
    color: var(--color-muted-text);
}
```

### Loading State
```html
<section data-state="loading">
    <!-- Content -->
</section>
```

```css
[data-state="loading"] {
    position: relative;
    opacity: 0.6;
    pointer-events: none;
}

[data-state="loading"]::after {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    /* Spinner styles */
}
```

### Empty State
```html
<div id="empty-state-projects" data-role="empty-state" data-state="empty">
    <h3 id="heading-empty">No projects found</h3>
    <p data-role="empty-message">Start by creating your first project</p>
    <nav data-role="empty-state-actions">
        <a href="/projects/new" role="button" data-type="primary">
            Create Project
        </a>
    </nav>
</div>
```

```css
[data-role="empty-state"] {
    text-align: center;
    padding: 4rem 2rem;
}

[data-role="empty-state"] h3 {
    color: var(--color-muted);
    margin-bottom: 1rem;
}

[data-role="empty-state"] [data-role="empty-message"] {
    color: var(--color-muted-text);
    margin-bottom: 2rem;
}
```

## Accessibility Patterns

### Required ARIA Attributes
```html
<!-- Navigation -->
<nav aria-label="Main navigation">
<nav aria-labelledby="heading-section">

<!-- Current page indicator -->
<a href="/page" aria-current="page">Current</a>

<!-- Form validation -->
<input aria-invalid="true" aria-describedby="error-field help-field">
<div id="error-field" role="alert">Error message</div>

<!-- Live regions -->
<div role="alert" aria-live="polite">Updated content</div>

<!-- Expandable content -->
<button aria-expanded="false" aria-controls="content-id">Toggle</button>
<div id="content-id">Content</div>

<!-- Loading states -->
<div aria-busy="true" aria-label="Loading content">...</div>
```

### Focus Management
```css
/* Visible focus indicators */
:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: 2px;
}

/* Skip navigation */
#skip-to-main {
    position: absolute;
    top: -40px;
    left: 0;
}

#skip-to-main:focus {
    top: 0;
}
```

## CSS Custom Properties

### Define in :root or [data-theme]
```css
:root {
    /* Colors */
    --color-primary: #0172ad;
    --color-secondary: #667eea;
    --color-success: #22c55e;
    --color-warning: #f59e0b;
    --color-danger: #ef4444;
    
    /* Text colors */
    --color-text: #1f2937;
    --color-text-muted: #6b7280;
    
    /* Backgrounds */
    --color-bg: #ffffff;
    --color-bg-secondary: #f9fafb;
    
    /* Spacing */
    --spacing-xs: 0.25rem;
    --spacing-sm: 0.5rem;
    --spacing-md: 1rem;
    --spacing-lg: 2rem;
    --spacing-xl: 4rem;
    
    /* Typography */
    --font-family: system-ui, -apple-system, sans-serif;
    --font-size-sm: 0.875rem;
    --font-size-base: 1rem;
    --font-size-lg: 1.125rem;
    --font-size-xl: 1.25rem;
    
    /* Borders */
    --border-radius: 0.375rem;
    --border-color: #e5e7eb;
}

[data-theme="dark"] {
    --color-text: #f3f4f6;
    --color-text-muted: #9ca3af;
    --color-bg: #1f2937;
    --color-bg-secondary: #111827;
    --border-color: #374151;
}
```

## Responsive Design Patterns

### Mobile-First Approach
```css
/* Base mobile styles */
[data-layout="grid"] {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

/* Tablet and up */
@media (min-width: 768px) {
    [data-layout="grid"] {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: 2rem;
    }
}

/* Desktop */
@media (min-width: 1024px) {
    [data-layout="grid"] {
        grid-template-columns: repeat(3, 1fr);
    }
}
```

### Container Queries (when supported)
```css
@container (min-width: 400px) {
    [data-component="card"] {
        flex-direction: row;
    }
}
```

## Print Styles

```css
@media print {
    /* Hide interactive elements */
    [data-role="form-actions"],
    [data-role="nav-user"],
    #site-footer {
        display: none;
    }
    
    /* Ensure content doesn't break across pages */
    [data-component="card"],
    section[data-section] {
        page-break-inside: avoid;
    }
    
    /* Show link URLs */
    a[href^="http"]:after {
        content: " (" attr(href) ")";
    }
}
```

## Animation Patterns

### Respect Motion Preferences
```css
/* Default animations */
[data-component="card"] {
    transition: transform 0.2s, box-shadow 0.2s;
}

[data-component="card"]:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

/* Reduced motion */
@media (prefers-reduced-motion: reduce) {
    * {
        animation-duration: 0.01ms !important;
        animation-iteration-count: 1 !important;
        transition-duration: 0.01ms !important;
    }
}
```

## Migration Guide

### Converting Class-Based CSS

#### Old (Class-Based)
```css
.card { }
.card-header { }
.btn-primary { }
.form-control { }
.alert-error { }
```

#### New (Semantic)
```css
[data-component="card"] { }
[data-component="card"] [data-role="card-header"] { }
button[data-type="primary"] { }
[data-component="form"] input { }
[data-component="alert"][data-type="error"] { }
```

### Finding Elements Without Classes

Instead of guessing class names, use predictable patterns:

1. **Components**: Look for `data-component`
2. **Unique elements**: Look for IDs following the naming pattern
3. **Form fields**: Use `#input-[fieldname]` pattern
4. **Sections**: Use `#section-[name]` pattern
5. **States**: Look for `data-state` attributes
6. **Types/Variations**: Look for `data-type` attributes

## Testing Your CSS

### Checklist
- [ ] Works with both light and dark themes
- [ ] Responsive on all device sizes
- [ ] Keyboard navigation is clearly visible
- [ ] Works with high contrast mode
- [ ] Print styles are included
- [ ] Respects prefers-reduced-motion
- [ ] No reliance on JavaScript for core styling
- [ ] Passes WCAG 2.1 AA accessibility standards

### Browser Testing Priority
1. Modern evergreen browsers (Chrome, Firefox, Safari, Edge)
2. Mobile browsers (iOS Safari, Chrome Mobile)
3. Progressive enhancement for older browsers

## Best Practices

### Do's ✅
- Use CSS custom properties for all colors and spacing
- Write mobile-first responsive styles
- Include focus states for all interactive elements
- Use semantic HTML elements
- Test with keyboard navigation
- Include print styles
- Document complex selectors with comments
- Group related styles together
- Use consistent naming patterns

### Don'ts ❌
- Don't use CSS classes for styling
- Don't use inline styles
- Don't modify HTML structure for styling purposes
- Don't use overly specific selectors (avoid > 3 levels)
- Don't forget accessibility attributes
- Don't rely on element order for styling
- Don't use `!important` except for utilities
- Don't forget to test responsive breakpoints

## Common Patterns Reference

### Form Validation States
```css
/* Valid field */
[aria-invalid="false"] {
    border-color: var(--color-success);
}

/* Invalid field */
[aria-invalid="true"] {
    border-color: var(--color-danger);
}

/* Error message */
[role="alert"][data-role="error-message"] {
    color: var(--color-danger);
    font-size: var(--font-size-sm);
    margin-top: 0.25rem;
}
```

### Navigation States
```css
/* Current page */
[aria-current="page"] {
    font-weight: 600;
    color: var(--color-primary);
}

/* Hover state */
nav a:hover {
    text-decoration: underline;
}

/* Focus state */
nav a:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: 2px;
}
```

### Button Variations
```css
button[data-type="primary"],
[role="button"][data-type="primary"] {
    background: var(--color-primary);
    color: white;
}

button[data-type="secondary"],
[role="button"][data-type="secondary"] {
    background: transparent;
    color: var(--color-primary);
    border: 1px solid var(--color-primary);
}

button[data-type="danger"],
[role="button"][data-type="danger"] {
    background: var(--color-danger);
    color: white;
}

button:disabled,
[role="button"][aria-disabled="true"] {
    opacity: 0.5;
    cursor: not-allowed;
}
```

## Resources

- [MDN Web Docs - HTML](https://developer.mozilla.org/en-US/docs/Web/HTML)
- [MDN Web Docs - CSS](https://developer.mozilla.org/en-US/docs/Web/CSS)
- [MDN Web Docs - ARIA](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA)
- [WCAG 2.1 Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [CSS Custom Properties](https://developer.mozilla.org/en-US/docs/Web/CSS/Using_CSS_custom_properties)
- [Modern CSS Solutions](https://moderncss.dev/)

## Version History

- **v2.0.0** - Complete rewrite with standardized naming conventions
- **v1.0.0** - Initial guidelines

---

*Last updated: 2024*