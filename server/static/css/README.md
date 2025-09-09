# SlateHub CSS Architecture

## Philosophy

SlateHub follows a **strict semantic CSS architecture** where HTML structure and CSS styling are completely separated. This means:

- **NO CSS classes for styling** - HTML should only contain semantic markup
- **NO style-related class names** - No `.button`, `.card`, `.error-message`, etc.
- **NO inline styles** - All styling lives in CSS files
- **NO JavaScript style manipulation** - JS should only modify data attributes and ARIA states

## File Organization

```
/static/css/
├── README.md           # This file
├── slatehub-pico.css  # Global overrides for Pico CSS framework
├── layout.css         # Header, footer, navigation, main structure
├── forms.css          # Form elements and validation states
├── components.css     # Reusable component patterns
└── pages/             # Page-specific styles (if needed)
    ├── projects.css
    ├── people.css
    └── about.css
```

## CSS Load Order

In `_layout.html`, CSS files are loaded in this specific order:

1. **Pico CSS** (external) - Base semantic framework
2. **slatehub-pico.css** - Our overrides to Pico
3. **layout.css** - Site structure
4. **forms.css** - Form styling
5. **components.css** - Component patterns
6. Page-specific CSS (loaded per template as needed)

## Selector Patterns

### ✅ CORRECT Selectors

```css
/* Element selectors */
nav { }
article { }
button { }

/* Attribute selectors for type */
input[type="email"] { }
button[type="submit"] { }

/* ARIA state selectors */
[aria-current="page"] { }
[aria-invalid="true"] { }
[aria-expanded="true"] { }
[role="alert"] { }

/* Data attribute selectors (for component state) */
[data-status="active"] { }
[data-featured="true"] { }
[data-theme="dark"] { }

/* Pseudo-classes for interaction */
button:hover { }
input:focus { }
a:visited { }

/* Structural pseudo-classes */
nav ul li:first-child { }
article:last-of-type { }

/* Combinators for context */
header nav a { }
main > article { }
footer section h3 { }
```

### ❌ NEVER Use These Selectors

```css
/* Class selectors */
.button { }
.error-message { }
.card { }
.primary { }
.large { }

/* ID selectors for styling (IDs are for JS/anchors only) */
#main-content { }  /* Exception: When used as anchor target */

/* Style-descriptive attributes */
[class*="error"] { }
[class*="success"] { }
```

## Common Patterns

### Form Validation

```css
/* Invalid fields */
input[aria-invalid="true"] {
    border-color: var(--pico-del-color);
}

/* Error messages */
[role="alert"] {
    color: var(--pico-del-color);
    background: var(--pico-del-background-color);
}

/* Success messages */
[role="status"] {
    color: var(--pico-ins-color);
    background: var(--pico-ins-background-color);
}
```

### Navigation States

```css
/* Current page indicator */
nav a[aria-current="page"] {
    font-weight: bold;
    color: var(--pico-primary);
}

/* Dropdown menus */
details[open] summary {
    background: var(--pico-secondary-background);
}
```

### Button Hierarchy

```css
/* Primary action (submit) */
button[type="submit"] {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
}

/* Secondary action (reset/cancel) */
button[type="reset"],
button[type="button"] {
    background: var(--pico-secondary);
}

/* Dangerous action */
button[data-action="delete"] {
    background: var(--pico-del-color);
}
```

### Loading States

```css
/* Loading button */
button[aria-busy="true"] {
    opacity: 0.7;
    cursor: wait;
}

/* Loading container */
[data-loading="true"] {
    position: relative;
    pointer-events: none;
}
```

## CSS Custom Properties (Variables)

We use Pico's CSS variables plus our own extensions:

```css
:root {
    /* Pico variables we commonly use */
    --pico-primary: /* theme primary color */
    --pico-color: /* main text color */
    --pico-background-color: /* main background */
    --pico-border-radius: /* standard radius */
    
    /* Our additions */
    --slatehub-header-height: 4rem;
    --slatehub-sidebar-width: 250px;
    --slatehub-content-max-width: 1200px;
}
```

## How to Add New Styles

### 1. Identify the Semantic Element

First, determine what the element **is**, not how it should **look**:

- Is it a navigation? Use `<nav>`
- Is it an independent piece of content? Use `<article>`
- Is it a thematic grouping? Use `<section>`
- Is it an alert? Use `role="alert"`

### 2. Find or Create the Right Selector

```css
/* For a project card that's featured */
article[data-featured="true"] {
    border: 2px solid var(--pico-primary);
}

/* For a form in submission state */
form[data-submitting="true"] {
    opacity: 0.7;
    pointer-events: none;
}
```

### 3. Never Add Classes to HTML

```html
<!-- ❌ WRONG -->
<div class="card featured">
    <h2 class="card-title">Project</h2>
</div>

<!-- ✅ CORRECT -->
<article data-featured="true">
    <h2>Project</h2>
</article>
```

## Responsive Design

Use semantic breakpoints based on content, not devices:

```css
/* When navigation needs to stack */
@media (max-width: 768px) {
    header nav {
        flex-direction: column;
    }
}

/* When cards need single column */
@media (max-width: 600px) {
    main > section {
        grid-template-columns: 1fr;
    }
}
```

## Accessibility

Always include focus states and ARIA-based styling:

```css
/* Focus states */
a:focus,
button:focus {
    outline: 2px solid var(--pico-primary);
    outline-offset: 2px;
}

/* Screen reader only */
.sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0,0,0,0);
    border: 0;
}

/* High contrast mode support */
@media (prefers-contrast: high) {
    button {
        border: 2px solid;
    }
}
```

## Debug Mode

During development, you can add this to catch violations:

```css
/* Highlight any elements with class attributes (shouldn't exist) */
[class] {
    outline: 3px solid red !important;
}

/* Highlight inline styles (shouldn't exist) */
[style] {
    outline: 3px solid orange !important;
}
```

## Migration Checklist

When converting existing HTML with classes:

1. **Identify the semantic meaning** of the element
2. **Replace div/span** with semantic HTML5 elements
3. **Remove all class attributes** used for styling
4. **Add data attributes** only for JavaScript state
5. **Add ARIA attributes** for accessibility and state
6. **Update CSS** to use semantic selectors
7. **Test** with keyboard navigation and screen readers

## Examples

### Card Component

```html
<!-- HTML (no classes!) -->
<article data-featured="true">
    <header>
        <h3>Project Title</h3>
        <time datetime="2024-01-15">January 15, 2024</time>
    </header>
    <p>Description text here...</p>
    <footer>
        <a href="/project/123" role="button">View Project</a>
    </footer>
</article>
```

```css
/* CSS (semantic selectors only) */
article[data-featured="true"] {
    border: 2px solid var(--pico-primary);
    padding: 1.5rem;
    margin-bottom: 2rem;
}

article header {
    display: flex;
    justify-content: space-between;
    margin-bottom: 1rem;
}

article footer a[role="button"] {
    display: inline-block;
    padding: 0.5rem 1rem;
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    text-decoration: none;
    border-radius: var(--pico-border-radius);
}
```

## Resources

- [MDN: Semantic HTML](https://developer.mozilla.org/en-US/docs/Glossary/Semantics#semantics_in_html)
- [ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/)
- [Pico CSS Documentation](https://picocss.com/docs/)

Remember: **The goal is complete separation of structure (HTML), presentation (CSS), and behavior (JavaScript).**