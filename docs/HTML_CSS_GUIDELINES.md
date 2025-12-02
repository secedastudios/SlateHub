# HTML & CSS Design Guidelines for SlateHub

## Overview

SlateHub follows a **strict separation of concerns** architecture where HTML provides semantic structure, CSS handles all visual presentation, and JavaScript manages behavior. This document serves as a comprehensive guide for designers and developers to understand the current structure and maintain consistency when redesigning the site.

**Key Principle**: All styling is done through CSS using semantic selectors - no CSS classes are used for presentation.

## Architecture Principles

### 1. Complete Separation of Concerns
- **HTML**: Semantic structure, content, and accessibility
- **CSS**: All visual presentation, layout, and responsive design
- **JavaScript**: Behavior, interactivity, and dynamic content
- **No presentational classes**: Use semantic HTML elements and data attributes

### 2. Semantic HTML First
- Use proper HTML5 semantic elements (`main`, `section`, `article`, `nav`, `header`, `footer`, `aside`)
- Choose elements based on meaning and document structure, not appearance
- Maintain proper heading hierarchy (h1 → h2 → h3)
- Use appropriate form elements and ARIA attributes for accessibility

### 3. Predictable Selector Patterns
- Consistent ID naming conventions for unique elements
- Data attributes for component identification and state management
- ARIA attributes for accessibility and interactive states

## Design System Variables

### Colors
The design system uses CSS custom properties defined in `/static/css/main.css`:

**Background Colors:**
- `--color-bg-primary`: #d6d8ca (Main sage background)
- `--color-bg-secondary`: #e5e7db
- `--color-bg-tertiary`: #f2f3ed
- `--color-bg-white`: #ffffff
- `--color-bg-dark`: #2a2a2a

**Text Colors:**
- `--color-text-primary`: #171717 (Primary text)
- `--color-text-secondary`: #5a5a5a
- `--color-text-tertiary`: #8a8a8a
- `--color-text-light`: #ffffff
- `--color-text-muted`: #9ca39e

**Accent & Semantic Colors:**
- `--color-accent`: #eb5437 (Primary accent - orange/red)
- `--color-accent-hover`: #d74328
- `--color-success`: #4a7c59 (Green)
- `--color-warning`: #d4a574 (Amber)
- `--color-error`: #c44536 (Red)
- `--color-info`: #5b7c99 (Blue)

**Border Colors:**
- `--color-border`: #c8cab9
- `--color-border-light`: #e0e2d5
- `--color-border-dark`: #9b9d8e

### Typography
**Font Families:**
- `--font-display`: "Denton XCondensed Test", "Helvetica Neue", Arial, sans-serif
- `--font-body`: "Helvetica Now Display", "Helvetica Neue", Arial, sans-serif
- `--font-mono`: "SF Mono", Monaco, "Courier New", monospace

**Font Sizes:**
- `--text-xs`: 0.75rem (12px)
- `--text-sm`: 0.875rem (14px)
- `--text-base`: 1rem (16px)
- `--text-lg`: 1.125rem (18px)
- `--text-xl`: 1.25rem (20px)
- `--text-2xl`: 1.5rem (24px)
- `--text-3xl`: 1.875rem (30px)
- `--text-4xl`: 2.25rem (36px)
- `--text-5xl`: 3rem (48px)

### Spacing System
- `--space-xs`: 0.25rem (4px)
- `--space-sm`: 0.5rem (8px)
- `--space-md`: 1rem (16px)
- `--space-lg`: 1.5rem (24px)
- `--space-xl`: 2rem (32px)
- `--space-2xl`: 3rem (48px)
- `--space-3xl`: 4rem (64px)
- `--space-4xl`: 6rem (96px)

### Border Radius
- `--radius-sm`: 2px
- `--radius-md`: 4px
- `--radius-lg`: 8px
- `--radius-xl`: 12px
- `--radius-full`: 9999px (pill shape)

### Shadows
- `--shadow-sm`: 0 1px 2px rgba(23, 23, 23, 0.05)
- `--shadow-md`: 0 2px 4px rgba(23, 23, 23, 0.08)
- `--shadow-lg`: 0 4px 8px rgba(23, 23, 23, 0.1)
- `--shadow-xl`: 0 8px 16px rgba(23, 23, 23, 0.12)

### Transitions
- `--transition-fast`: 150ms ease-in-out
- `--transition-base`: 250ms ease-in-out
- `--transition-slow`: 350ms ease-in-out

## HTML Structure Patterns

### Page Layout
```html
<!doctype html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="color-scheme" content="light dark" />
    <title>Page Title - SlateHub</title>
    <!-- Embedded fonts and styles -->
</head>
<body data-page="[page-name]" data-user="[authenticated|anonymous]">
    <header id="site-header">
        <nav id="main-nav" aria-label="Main navigation">
            <!-- Navigation content -->
        </nav>
    </header>
    
    <main id="main-content">
        <!-- Page content sections -->
    </main>
    
    <footer id="site-footer">
        <!-- Footer content -->
    </footer>
</body>
</html>
```

### Navigation Structure
```html
<header id="site-header">
    <nav id="main-nav" aria-label="Main navigation">
        <ul id="nav-brand" data-role="nav-brand">
            <li>
                <a href="/" id="site-logo" aria-label="SlateHub Home">
                    <strong data-role="logo-text">SlateHub</strong>
                </a>
            </li>
        </ul>
        
        <ul id="nav-primary" data-role="nav-primary">
            <li>
                <a href="/people" id="link-nav-people" 
                   aria-current="page">People</a>
            </li>
            <!-- More navigation items -->
        </ul>
        
        <ul id="nav-user" data-role="nav-user">
            <!-- User menu or login/signup buttons -->
        </ul>
    </nav>
</header>
```

### Form Structure
```html
<section id="section-[form-name]" data-component="auth-form" data-type="[type]">
    <header id="[form-name]-header" data-role="form-header">
        <hgroup>
            <h1 id="heading-[form-name]">Form Title</h1>
            <p data-role="subtitle">Form description</p>
        </hgroup>
    </header>

    <form id="form-[form-name]" method="post" data-component="form">
        <fieldset id="fieldset-[section]" data-role="form-section">
            <legend>Section Title</legend>
            
            <div id="field-[field-name]" data-field="[field-name]">
                <label for="input-[field-name]">Field Label</label>
                <input type="text" id="input-[field-name]" name="[field-name]"
                       required aria-required="true" aria-invalid="false" />
            </div>
        </fieldset>
        
        <div id="[form-name]-actions" data-role="form-actions">
            <button type="submit" id="button-submit-[form-name]" data-type="primary">
                Submit
            </button>
        </div>
    </form>
</section>
```

### Card Components
```html
<article data-component="project-card" data-project-id="123" data-status="active">
    <header data-role="card-header">
        <h2 id="project-title-123">
            <a href="/projects/123">Project Title</a>
        </h2>
        <span data-role="status-badge" data-status="active">Active</span>
    </header>
    
    <div data-role="card-body">
        <p data-role="description">Project description...</p>
        
        <div data-role="metadata">
            <span data-field="owner">By John Doe</span>
            <time datetime="2024-01-15" data-field="created-date">Jan 15, 2024</time>
        </div>
        
        <div data-role="tags-list">
            <span data-role="tag" data-tag="film">film</span>
        </div>
    </div>
    
    <footer data-role="card-footer">
        <nav data-role="card-actions">
            <a href="/projects/123" role="button" data-type="primary">
                View Details
            </a>
        </nav>
    </footer>
</article>
```

## CSS Selector Patterns

### Page Context Selectors
```css
/* Target specific pages */
[data-page="home"] { }
[data-page="profile"] { }
[data-page="login"] { }

/* Target authenticated/anonymous states */
[data-user="authenticated"] [data-role="nav-user"] { }
[data-user="anonymous"] [data-role="nav-user"] { }
```

### Component Selectors
```css
/* Component containers */
[data-component="project-card"] { }
[data-component="auth-form"] { }
[data-component="user-menu"] { }

/* Component parts */
[data-component="project-card"] [data-role="card-header"] { }
[data-component="project-card"] [data-role="card-body"] { }
[data-component="auth-form"] [data-field="email"] { }
```

### ID Selectors
```css
/* Unique page elements */
#site-header { }
#main-nav { }
#main-content { }
#site-footer { }

/* Specific elements */
#heading-projects { }
#form-login { }
#button-submit-login { }
```

### State Selectors
```css
/* Element states */
[data-state="loading"] { }
[data-state="empty"] { }
[data-status="active"] { }
[data-status="pending"] { }

/* ARIA states */
[aria-expanded="true"] { }
[aria-current="page"] { }
[aria-invalid="true"] { }
```

### Type and Variant Selectors
```css
/* Button types */
button[data-type="primary"] { }
button[data-type="secondary"] { }
button[data-type="danger"] { }

/* Layout types */
[data-layout="grid"] { }
[data-layout="flex"] { }
```

## Component Catalog

### Navigation Components

**Main Navigation** (`#site-header`, `#main-nav`):
- Fixed position header with brand, primary nav, and user menu
- Responsive collapse on mobile devices
- Active page indication with `aria-current="page"`

**User Menu** (`[data-component="user-menu"]`):
- Dropdown using HTML `<details>` element
- Avatar display (image or initials)
- Menu items with semantic roles

### Form Components

**Auth Forms** (`[data-component="auth-form"]`):
- Login, signup, password reset forms
- Centered layout with background and border
- Consistent field structure with proper labeling

**Filter Forms** (`[data-component="filter-form"]`):
- Horizontal layout with flexible wrapping
- Used for search and filtering interfaces

**Field Structure**:
- Wrapped in `div[data-field]` containers
- Consistent label and input pairing
- Error messages with `div[role="alert"]`
- Help text with `small[data-role="help-text"]`

### Card Components

**Project Cards** (`[data-component="project-card"]`):
- Article-based semantic structure
- Header with title and status badge
- Body with description and metadata
- Footer with action buttons

**Person Cards** (`[data-component="person-card"]`):
- Similar structure to project cards
- Avatar integration
- Professional information display

### Button Components

**Button Types**:
- `data-type="primary"`: Main action buttons (blue)
- `data-type="secondary"`: Secondary actions (outlined)
- `data-type="danger"`: Destructive actions (red)

**Button States**:
- `:hover`, `:focus`, `:active` states defined
- `:disabled` and `[aria-disabled="true"]` support

### Status and State Components

**Status Badges** (`[data-role="status-badge"]`):
- Color-coded status indicators
- Types: active (green), pending (yellow), inactive (gray)

**Loading States** (`[data-state="loading"]`):
- CSS-only spinner animation
- Applied to containers and forms

**Empty States** (`[data-state="empty"]`):
- Centered content with helpful messaging
- Action buttons to resolve empty state

### Alert Components

**Alert Types** (`[data-component="alert"]`):
- `data-type="success"`: Green success alerts
- `data-type="warning"`: Amber warning alerts  
- `data-type="error"`: Red error alerts
- `data-type="info"`: Blue informational alerts

### Avatar Components

**Avatar Sizes**:
- Small (32px): Navigation and inline use
- Medium (48px): Card components
- Large (120px): Profile headers

**Avatar Types**:
- Image avatars with fallback to initials
- Initials with gradient background
- Status indicators for online presence

## Layout Patterns

### Container System
```css
.container, [data-container] {
    width: 100%;
    max-width: 1200px;
    margin: 0 auto;
    padding: 0 var(--spacing-md);
}
```

### Grid Layouts
```css
[data-layout="grid"] {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: var(--spacing-lg);
}
```

### Flex Layouts
```css
[data-layout="flex"] {
    display: flex;
    gap: var(--spacing-md);
    align-items: center;
}
```

## Responsive Design

### Breakpoints
- Mobile: `max-width: 768px`
- Tablet: `768px - 1024px`
- Desktop: `min-width: 1024px`

### Mobile-First Approach
Base styles are mobile-optimized, with progressive enhancement:

```css
/* Mobile first (base styles) */
[data-layout="grid"] {
    display: block;
}

/* Tablet and up */
@media (min-width: 768px) {
    [data-layout="grid"] {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
    }
}

/* Desktop and up */  
@media (min-width: 1024px) {
    [data-layout="grid"] {
        grid-template-columns: repeat(3, 1fr);
    }
}
```

### Navigation Responsive Behavior
- Mobile: Collapsed navigation with user menu adjustments
- Tablet/Desktop: Full horizontal navigation layout

## Dark Mode Support

Dark mode is implemented via `[data-theme="dark"]` attribute:

```css
:root {
    /* Light theme variables */
}

[data-theme="dark"] {
    /* Dark theme overrides */
    --color-text: #f3f4f6;
    --color-bg: #111827;
    --color-border: #374151;
}
```

Theme switching is handled via JavaScript with localStorage persistence.

## Accessibility Patterns

### Focus Management
```css
*:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: 2px;
}
```

### Skip Links
```css
#skip-to-main {
    position: absolute;
    top: -40px;
    left: 0;
    transform: translateY(-100%);
}

#skip-to-main:focus {
    transform: translateY(0);
}
```

### ARIA Integration
- `aria-current="page"` for active navigation
- `aria-expanded` for dropdown states
- `aria-invalid` for form validation
- `role="alert"` for error messages
- `role="button"` for link-buttons

### Color Contrast
All color combinations meet WCAG AA standards (4.5:1 ratio minimum).

## Animation and Motion

### Reduced Motion Support
```css
@media (prefers-reduced-motion: reduce) {
    *, *::before, *::after {
        animation-duration: 0.01ms !important;
        animation-iteration-count: 1 !important;
        transition-duration: 0.01ms !important;
    }
}
```

### Standard Transitions
- `--transition-all`: All properties, 150ms duration
- `--transition-colors`: Color properties only
- `--transition-opacity`: Opacity changes
- `--transition-transform`: Transform properties

## Print Styles

Print styles hide interactive elements and optimize for readability:

```css
@media print {
    #site-header, #site-footer, 
    [data-role="nav-user"], 
    button, [role="button"] {
        display: none;
    }
    
    body {
        background: white;
        color: black;
    }
}
```

## CSS File Organization

### Structure
```
/static/css/
├── main.css                   # Core styles, layout, and design system variables
├── components/
│   ├── forms.css             # Form styling
│   ├── avatar.css            # Avatar components  
│   └── image-upload.css      # Image upload widget
├── pages/
│   ├── errors.css            # Error page styles
│   ├── profile.css           # Profile page styles
│   └── public-profile.css    # Public profile styles
└── legal.css                  # Legal pages styling
```

### Import Order
1. Variables and design tokens
2. Reset and base styles  
3. Layout and grid systems
4. Component styles
5. Page-specific styles
6. Utility classes (minimal)

## Designer Guidelines

### Making Changes
1. **Colors**: Modify CSS custom properties in `main.css`
2. **Typography**: Update font variables and heading styles in `main.css`
3. **Spacing**: Adjust spacing variables in `main.css` for consistent rhythm
4. **Components**: Target component data attributes, not classes
5. **Layout**: Use CSS Grid and Flexbox via data attributes
6. **States**: Leverage data-state and ARIA attributes for styling

### Common Tasks

**Changing Button Styles**:
```css
button[data-type="primary"] {
    background: var(--color-primary);
    border: none;
    border-radius: var(--border-radius);
    color: white;
    /* etc */
}
```

**Modifying Card Layout**:
```css
[data-component="project-card"] {
    padding: var(--card-padding);
    background: var(--card-bg);
    border: 1px solid var(--color-border);
    border-radius: var(--card-border-radius);
    /* etc */
}
```

**Responsive Adjustments**:
```css
@media (max-width: 768px) {
    [data-component="project-card"] {
        padding: var(--spacing-md);
    }
}
```

### Testing Considerations
- Test both light and dark themes
- Verify responsive behavior at all breakpoints
- Check keyboard navigation and focus states
- Validate color contrast ratios
- Test with reduced motion preferences

## Best Practices

### Do ✅
- Use semantic HTML elements first
- Target elements with data attributes and IDs
- Maintain consistent spacing using design system variables
- Test accessibility features
- Follow mobile-first responsive design
- Use CSS custom properties for theming
- Implement proper focus management

### Don't ❌
- Add CSS classes for presentation
- Use inline styles
- Override semantic HTML with styling
- Hardcode color values instead of variables
- Break the component data attribute patterns  
- Ignore responsive behavior
- Skip accessibility testing

## Resources
- **Main Stylesheet & Variables**: `/static/css/main.css`
- **Component Examples**: `/static/css/components/`
- **Page Styles**: `/static/css/pages/`
- **HTML Templates**: `/server/templates/`

This guide ensures consistency and maintainability while providing complete creative freedom for visual design through CSS.