# HTML & CSS Guidelines for SlateHub

## Overview

SlateHub uses a **CSS-only design system** where HTML structure is fixed and semantic, while all visual design is controlled through CSS. This allows designers to create any visual design without modifying HTML templates.

## Core Principles

### 1. Complete Separation of Concerns
- **HTML**: Defines structure and semantics only
- **CSS**: Controls all visual presentation
- **JavaScript**: Handles behavior and interactions
- **No CSS classes** for styling purposes

### 2. Semantic HTML First
- Use proper HTML5 semantic elements
- Choose elements based on meaning, not appearance
- Maintain proper document outline and hierarchy

### 3. Predictable Selectors
- Every styleable element has a predictable selector
- Use IDs for unique elements
- Use data attributes for components and variations
- Use ARIA attributes for states

## HTML Structure Guidelines

### Semantic Element Usage

#### ✅ Use `<article>` for:
- Blog posts
- News items  
- Project cards
- User comments
- Any self-contained, redistributable content

#### ✅ Use `<section>` for:
- Thematic groupings of content
- Page regions with headings
- Form wrappers
- Content groups

#### ✅ Use `<div>` for:
- Generic containers
- Layout wrappers
- When no semantic element is appropriate

#### ❌ Never use `<article>` for:
- Forms
- Navigation
- Footers
- UI components
- Page sections

## Naming Conventions

### ID Naming Pattern
```
[context]-[element]-[purpose]
```

Examples:
- `#main-nav` - Main navigation
- `#form-login` - Login form
- `#input-email` - Email input field
- `#section-projects` - Projects section
- `#heading-about` - About section heading

### Data Attributes

#### `data-component` - Identifies reusable components
```html
<article data-component="project-card">
<section data-component="auth-form">
<nav data-component="breadcrumb">
```

#### `data-page` - Page context (on body element)
```html
<body data-page="profile">
<body data-page="projects">
```

#### `data-section` - Section context
```html
<section data-section="controls">
<div data-section="user-content">
```

#### `data-state` - Element states
```html
<form data-state="submitting">
<div data-state="loading">
<section data-state="empty">
```

#### `data-type` - Variations
```html
<button data-type="primary">
<article data-type="featured">
<section data-type="login">
```

#### `data-role` - Semantic purposes
```html
<div data-role="card-header">
<nav data-role="pagination">
<div data-role="empty-state">
```

## HTML Templates

### Page Structure
```html
<body data-page="[page-name]" data-user="[authenticated|anonymous]">
    <header id="site-header">
        <nav id="main-nav" aria-label="Main navigation">
            <ul data-role="nav-primary">...</ul>
            <ul data-role="nav-user">...</ul>
        </nav>
    </header>
    
    <main id="main-content">
        <!-- Page content -->
    </main>
    
    <footer id="site-footer">
        <section data-role="footer-main">
            <div data-role="footer-brand">...</div>
            <div data-role="footer-links">...</div>
        </section>
    </footer>
</body>
```

### Form Structure
```html
<section data-component="auth-form" data-type="login">
    <header data-role="form-header">
        <h2 id="heading-login">Title</h2>
    </header>
    
    <form id="form-login" method="post">
        <fieldset data-role="form-section">
            <div data-field="email">
                <label for="input-email">Email</label>
                <input id="input-email" name="email">
                <div id="error-email" role="alert">Error</div>
            </div>
        </fieldset>
        
        <div data-role="form-actions">
            <button type="submit" data-type="primary">Submit</button>
        </div>
    </form>
</section>
```

### Content Card
```html
<article data-component="project-card" data-status="active">
    <header data-role="card-header">
        <h2 id="project-title-123">Title</h2>
        <span data-role="status-badge">Active</span>
    </header>
    
    <div data-role="card-body">
        <p data-role="description">Description</p>
        <div data-role="metadata">
            <time datetime="2024-01-01">Date</time>
        </div>
    </div>
    
    <footer data-role="card-footer">
        <nav data-role="card-actions">
            <a href="#" role="button" data-type="primary">Action</a>
        </nav>
    </footer>
</article>
```

## CSS Selector Patterns

### Basic Selectors
```css
/* Page context */
[data-page="profile"] { }

/* Component targeting */
[data-component="project-card"] { }

/* State-based styling */
[data-state="loading"] { }
[aria-expanded="true"] { }
[aria-current="page"] { }

/* Type variations */
[data-type="primary"] { }
[data-status="active"] { }

/* Nested selectors */
[data-component="project-card"] [data-role="card-header"] { }
```

### Layout Control
```css
/* Grid layouts */
[data-layout="grid"] {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
}

/* Responsive containers */
@media (max-width: 768px) {
    [data-layout="grid"] {
        grid-template-columns: 1fr;
    }
}
```

### Theme Support
```css
/* Light/Dark theme */
[data-theme="dark"] [data-component="card"] {
    background: var(--dark-bg);
}

/* User preference */
[data-user="authenticated"] [data-role="nav-user"] {
    display: flex;
}
```

## Benefits for Designers

### 1. Complete Visual Control
- Style any element without touching HTML
- Create multiple themes/skins
- Implement any design system

### 2. Predictable Structure
- Consistent naming patterns
- Stable selectors that won't change
- Clear component boundaries

### 3. Flexibility
- Override Pico CSS defaults
- Add custom animations
- Create responsive designs
- Support multiple themes

### 4. No Build Process
- Edit CSS files directly
- See changes immediately
- No compilation needed

## CSS File Organization

```
/static/css/
├── base/
│   ├── reset.css         # CSS reset
│   ├── variables.css     # CSS custom properties
│   └── typography.css    # Base typography
├── layout/
│   ├── header.css       # Site header
│   ├── footer.css       # Site footer
│   └── navigation.css   # Navigation components
├── components/
│   ├── cards.css        # Content cards
│   ├── forms.css        # Form styling
│   ├── buttons.css      # Button variations
│   └── modals.css       # Modal dialogs
├── pages/
│   ├── profile.css      # Profile page
│   ├── projects.css     # Projects page
│   └── people.css       # People page
└── themes/
    ├── light.css        # Light theme
    └── dark.css         # Dark theme
```

## CSS-Only Interactions

### Available Without JavaScript
- `:hover` - Hover effects
- `:focus` / `:focus-visible` - Focus styles
- `:active` - Active states
- `:checked` - Checkbox/radio states
- `:empty` - Empty containers
- `:target` - Anchor targeting
- `:valid` / `:invalid` - Form validation
- `[open]` - Details/summary state

### Example: Interactive Card
```css
[data-component="project-card"] {
    transition: transform 0.2s, box-shadow 0.2s;
}

[data-component="project-card"]:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0,0,0,0.1);
}

[data-component="project-card"]:active {
    transform: translateY(0);
}
```

## Accessibility Considerations

### Required Patterns
- Use `aria-label` for navigation regions
- Use `aria-current="page"` for active navigation
- Use `aria-invalid="true"` for form errors
- Use `role="alert"` for error messages
- Use `aria-describedby` for form help text

### Focus Management
```css
/* Visible focus indicators */
:focus-visible {
    outline: 2px solid var(--primary-color);
    outline-offset: 2px;
}

/* Skip to main content */
#skip-to-main:focus {
    position: absolute;
    top: 0;
    left: 0;
}
```

## Testing Your CSS

### Checklist
- [ ] Works in light and dark themes
- [ ] Responsive on mobile devices
- [ ] Keyboard navigation visible
- [ ] High contrast mode compatible
- [ ] Print styles included
- [ ] Reduced motion respected

### Browser Testing
```css
/* Progressive enhancement */
@supports (display: grid) {
    [data-layout="grid"] {
        display: grid;
    }
}

/* Fallbacks */
[data-layout="grid"] {
    display: flex;
    flex-wrap: wrap;
}
```

## Common Patterns

### Status Indicators
```css
[data-status="active"] { color: green; }
[data-status="pending"] { color: orange; }
[data-status="inactive"] { color: gray; }
```

### Loading States
```css
[data-state="loading"] {
    opacity: 0.6;
    pointer-events: none;
    position: relative;
}

[data-state="loading"]::after {
    content: "";
    position: absolute;
    /* Spinner styles */
}
```

### Empty States
```css
[data-state="empty"] {
    text-align: center;
    padding: 4rem 2rem;
}
```

## Migration Guide

### From Class-Based CSS
```css
/* Old (class-based) */
.card { }
.card-header { }
.btn-primary { }

/* New (semantic) */
[data-component="card"] { }
[data-component="card"] [data-role="card-header"] { }
button[data-type="primary"] { }
```

### Finding Elements
```css
/* Instead of guessing classes, use predictable patterns */

/* Components are always data-component */
[data-component="..."] { }

/* Page sections use IDs */
#section-projects { }

/* Form fields follow pattern */
#input-[fieldname] { }
#error-[fieldname] { }
```

## Best Practices

### Do's ✅
- Use CSS custom properties for theming
- Leverage CSS Grid and Flexbox for layouts
- Use semantic selectors
- Include print styles
- Test with keyboard navigation
- Support reduced motion preferences

### Don'ts ❌
- Don't modify HTML templates
- Don't add CSS classes to HTML
- Don't use inline styles
- Don't rely on element order
- Don't use overly specific selectors
- Don't forget accessibility

## Resources

- [MDN Web Docs - CSS](https://developer.mozilla.org/en-US/docs/Web/CSS)
- [MDN Web Docs - ARIA](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA)
- [Pico CSS Documentation](https://picocss.com/docs/)
- [CSS Custom Properties](https://developer.mozilla.org/en-US/docs/Web/CSS/Using_CSS_custom_properties)

## Examples

See the following files for reference implementations:
- `/static/css/pages/projects.css` - Projects page styling
- `/static/css/pages/people.css` - People page styling
- `/static/css/pages/profile.css` - Profile page styling

Each demonstrates how to create sophisticated designs using only semantic selectors and data attributes.