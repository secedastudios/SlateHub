# Pico CSS Migration Guide for SlateHub

## Overview

SlateHub has been migrated to use [Pico CSS](https://picocss.com/), a minimal CSS framework for semantic HTML. This guide explains how to work with Pico CSS while maintaining clean, semantic HTML markup.

## Key Principles

### 1. Semantic HTML First
Pico CSS is designed to style semantic HTML elements directly. Write proper HTML first, then apply minimal classes only when needed.

### 2. Dark Theme Default
SlateHub defaults to dark theme (`data-theme="dark"`). The theme toggle switches between light and dark modes, with the preference saved in localStorage.

### 3. Minimal Custom CSS
Most styling comes from Pico CSS. Custom styles in `slatehub-pico.css` only provide:
- SlateHub-specific overrides
- Theme toggle functionality
- Custom components not in Pico
- Utility classes for accessibility

## File Structure

```
/static/css/
├── slatehub-pico.css    # Custom overrides and extensions
└── slatehub.css          # (deprecated - to be removed)
```

## Theme System

### HTML Setup
```html
<html lang="en" data-theme="dark">
  <head>
    <meta name="color-scheme" content="light dark">
    <!-- Pico CSS from CDN -->
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css">
    <!-- Custom overrides -->
    <link rel="stylesheet" href="/static/css/slatehub-pico.css">
  </head>
</html>
```

### Theme Toggle Implementation
```javascript
const toggle = () => {
    const currentTheme = document.documentElement.getAttribute("data-theme");
    const newTheme = currentTheme === "light" ? "dark" : "light";
    document.documentElement.setAttribute("data-theme", newTheme);
    localStorage.setItem("theme", newTheme);
};
```

## Component Patterns

### Navigation

#### Before (Custom CSS)
```html
<nav class="main-navigation">
  <div class="nav-brand">
    <a href="/" class="brand-link">SlateHub</a>
  </div>
  <ul class="nav-menu">
    <li class="nav-item">
      <a href="/" class="nav-link active">Home</a>
    </li>
  </ul>
</nav>
```

#### After (Pico CSS)
```html
<nav class="container">
  <ul>
    <li><a href="/" class="contrast"><strong>SlateHub</strong></a></li>
  </ul>
  <ul>
    <li><a href="/" class="contrast">Home</a></li>
  </ul>
</nav>
```

### Forms

#### Semantic Form Structure
```html
<form>
  <fieldset>
    <legend>Section Title</legend>
    
    <label for="field-id">
      Field Label
      <input type="text" id="field-id" name="field_name" required>
    </label>
    
    <label for="select-id">
      Select Field
      <select id="select-id" name="select_name">
        <option value="">Choose...</option>
        <option value="1">Option 1</option>
      </select>
    </label>
  </fieldset>
  
  <button type="submit">Submit</button>
</form>
```

### Cards/Articles

#### Semantic Card Pattern
```html
<article>
  <header>
    <hgroup>
      <h3>Card Title</h3>
      <p>Card subtitle or meta information</p>
    </hgroup>
  </header>
  <p>Card content goes here...</p>
  <footer>
    <button>Action</button>
  </footer>
</article>
```

### Buttons

#### Button Variants
```html
<!-- Primary button (default) -->
<button>Primary</button>

<!-- Secondary button -->
<button class="secondary">Secondary</button>

<!-- Contrast button (high visibility) -->
<button class="contrast">Contrast</button>

<!-- Outline variants -->
<button class="outline">Outline</button>
<button class="secondary outline">Secondary Outline</button>
<button class="contrast outline">Contrast Outline</button>

<!-- Link as button -->
<a href="#" role="button">Link Button</a>

<!-- Loading state -->
<button aria-busy="true">Loading...</button>

<!-- Disabled state -->
<button disabled>Disabled</button>
```

### Tables

#### Accessible Table Structure
```html
<figure>
  <table>
    <thead>
      <tr>
        <th scope="col">Column 1</th>
        <th scope="col">Column 2</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <th scope="row">Row Header</th>
        <td>Data</td>
      </tr>
    </tbody>
  </table>
  <figcaption>Table description</figcaption>
</figure>
```

### Modals/Dialogs

#### Native Dialog Element
```html
<button onclick="document.getElementById('modal-id').showModal()">
  Open Modal
</button>

<dialog id="modal-id">
  <article>
    <header>
      <button aria-label="Close" rel="prev" 
              onclick="this.closest('dialog').close()">×</button>
      <h3>Modal Title</h3>
    </header>
    <p>Modal content...</p>
    <footer>
      <button class="secondary" 
              onclick="this.closest('dialog').close()">Cancel</button>
      <button>Confirm</button>
    </footer>
  </article>
</dialog>
```

### Grid Layouts

#### Responsive Grid
```html
<!-- Auto-responsive grid -->
<div class="grid">
  <div>Column 1</div>
  <div>Column 2</div>
  <div>Column 3</div>
</div>

<!-- Grid with sidebar -->
<div class="grid">
  <article>Main content (takes more space)</article>
  <aside>Sidebar content</aside>
</div>
```

## Custom Utility Classes

### Accessibility
```css
/* Screen reader only content */
.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

/* Skip link for keyboard navigation */
.skip-link {
  position: absolute;
  top: -40px;
  left: 0;
}
.skip-link:focus {
  top: 0;
}
```

### Loading States
```css
.loading {
  opacity: 0.6;
  pointer-events: none;
}
```

### Status Badges
```html
<span class="status-badge active">Active</span>
<span class="status-badge inactive">Inactive</span>
<span class="status-badge pending">Pending</span>
```

## Migration Checklist

### HTML Structure
- [ ] Remove unnecessary wrapper divs
- [ ] Use semantic HTML5 elements (`<article>`, `<section>`, `<aside>`, `<nav>`)
- [ ] Add proper ARIA labels where needed
- [ ] Use `<hgroup>` for grouped headings
- [ ] Implement `<dialog>` for modals

### CSS Classes
- [ ] Remove custom utility classes replaced by Pico
- [ ] Replace `.primary-action` with default button styling
- [ ] Use `.contrast` for active/highlighted navigation
- [ ] Replace custom form styling with Pico defaults
- [ ] Use `.outline` for secondary button styles

### JavaScript
- [ ] Update theme toggle to use Pico's data-theme attribute
- [ ] Replace custom modal implementations with native dialog
- [ ] Remove unnecessary JavaScript for dropdowns (use `<details>`)

### Forms
- [ ] Wrap related fields in `<fieldset>` with `<legend>`
- [ ] Place labels above inputs (Pico's default)
- [ ] Remove custom form validation styling
- [ ] Use native HTML5 validation attributes

### Tables
- [ ] Add `scope` attributes to `<th>` elements
- [ ] Wrap tables in `<figure>` with `<figcaption>`
- [ ] Remove custom responsive table CSS

## Best Practices

### 1. Container Usage
Use `.container` class sparingly, primarily for:
- Top-level navigation
- Main content wrapper
- Footer content

### 2. Button Hierarchy
- Default (no class): Primary actions
- `.secondary`: Less important actions
- `.contrast`: High-visibility or active states
- `.outline`: Tertiary actions

### 3. Color System
Pico automatically handles colors based on theme. Avoid hardcoding colors; use CSS variables when needed:
```css
var(--pico-primary)
var(--pico-background-color)
var(--pico-color)
```

### 4. Spacing
Pico provides consistent spacing. Avoid custom margins/padding unless necessary.

### 5. Responsive Design
Pico is mobile-first and responsive by default. Test on mobile devices and only add custom media queries when needed.

## Common Pitfalls to Avoid

1. **Over-classing**: Don't add classes when semantic HTML will suffice
2. **Custom grids**: Use Pico's `.grid` class instead of custom flexbox/grid
3. **Color overrides**: Work with the theme system, not against it
4. **Custom buttons**: Use Pico's button variants instead of creating new ones
5. **Wrapper divs**: Remove unnecessary wrapper elements

## Testing

### Accessibility Testing
1. Test keyboard navigation (Tab, Enter, Escape)
2. Verify screen reader compatibility
3. Check color contrast in both themes
4. Ensure focus states are visible

### Theme Testing
1. Toggle between light and dark themes
2. Verify localStorage persistence
3. Check all components in both themes
4. Test theme-specific custom styles

### Responsive Testing
1. Test on mobile devices (320px+)
2. Verify tablet layouts (768px+)
3. Check desktop views (1024px+)
4. Test grid breakpoints

## Resources

- [Pico CSS Documentation](https://picocss.com/docs)
- [Pico CSS Examples](https://picocss.com/examples)
- [MDN Web Docs - Semantic HTML](https://developer.mozilla.org/en-US/docs/Glossary/Semantics)
- [Component Examples](/templates/example_components.html)

## Support

For questions or issues related to the Pico CSS migration:
1. Check this guide first
2. Review the example components page
3. Consult the Pico CSS documentation
4. Ask in the development channel

---

*Last updated: Migration to Pico CSS v2.x*