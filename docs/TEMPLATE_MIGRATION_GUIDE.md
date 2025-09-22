# Template Migration Guide

## Overview

This guide provides step-by-step instructions for migrating existing templates to the new standardized HTML structure. All templates must follow the semantic HTML patterns and consistent naming conventions outlined in the HTML & CSS Guidelines.

## Migration Checklist

### Core Templates
- [x] `/templates/login.html` - ✅ Complete
- [x] `/templates/signup.html` - ✅ Complete  
- [x] `/templates/index.html` - ✅ Complete
- [x] `/templates/partials/footer.html` - ✅ Complete
- [ ] `/templates/partials/header.html` - In Progress
- [ ] `/templates/_layout.html` - Needs Review
- [ ] `/templates/profile.html`
- [ ] `/templates/profile_edit.html`
- [ ] `/templates/projects.html`
- [ ] `/templates/people.html`
- [ ] `/templates/about.html`

### Equipment Templates
- [ ] `/templates/equipment/list.html`
- [ ] `/templates/equipment/detail.html`
- [ ] `/templates/equipment/form.html`
- [ ] `/templates/equipment/checkout.html`
- [ ] `/templates/equipment/checkin.html`
- [ ] `/templates/equipment/kit_detail.html`
- [ ] `/templates/equipment/kit_form.html`
- [ ] `/templates/equipment/rental_history.html`

### Organization Templates
- [ ] `/templates/organizations/list.html`
- [ ] `/templates/organizations/profile.html`
- [ ] `/templates/organizations/edit.html`
- [ ] `/templates/organizations/new.html`
- [ ] `/templates/organizations/my-orgs.html`

### Error Templates
- [ ] `/templates/errors/401.html`
- [ ] `/templates/errors/403.html`
- [ ] `/templates/errors/404.html`
- [ ] `/templates/errors/500.html`
- [ ] `/templates/errors/generic.html`

### Other Templates
- [ ] `/templates/persons/public_profile.html`
- [ ] `/templates/partials/card.html`
- [ ] `/templates/partials/project-card.html`
- [ ] `/templates/partials/scripts.html`
- [ ] `/templates/partials/styles.html`
- [ ] `/templates/macros.html`
- [ ] `/templates/example_components.html`

## Migration Rules

### 1. Remove ALL CSS Classes

❌ **Before:**
```html
<div class="card">
<button class="btn btn-primary">
<section class="hero-section">
```

✅ **After:**
```html
<article data-component="card">
<button data-type="primary">
<section id="section-hero" data-section="hero">
```

### 2. Add Semantic IDs

Every unique element needs an ID following the pattern: `[context]-[element]-[purpose]`

❌ **Before:**
```html
<h1>Welcome</h1>
<form method="post">
<input type="email" name="email">
```

✅ **After:**
```html
<h1 id="heading-welcome">Welcome</h1>
<form id="form-contact" method="post">
<input type="email" id="input-email" name="email">
```

### 3. Add Data Attributes

#### Components
```html
<!-- Major reusable components -->
<article data-component="project-card">
<section data-component="auth-form">
<div data-component="search-bar">
```

#### Sections
```html
<!-- Page sections -->
<section id="section-overview" data-section="overview">
<div id="section-filters" data-section="filters">
```

#### Roles
```html
<!-- Semantic roles within components -->
<header data-role="page-header">
<div data-role="card-body">
<nav data-role="card-actions">
```

#### Fields
```html
<!-- Form fields -->
<div id="field-email" data-field="email">
<div id="field-password" data-field="password">
```

#### States
```html
<!-- Element states -->
<form data-state="submitting">
<section data-state="loading">
<div data-state="empty">
```

#### Types
```html
<!-- Variations -->
<button data-type="primary">
<article data-type="featured">
<div data-type="warning">
```

### 4. Fix Form Structure

❌ **Before:**
```html
<form class="login-form">
    <div class="form-group">
        <label>Email</label>
        <input type="email" name="email" class="form-control">
        <span class="error">Invalid email</span>
    </div>
    <button class="btn btn-submit">Login</button>
</form>
```

✅ **After:**
```html
<form id="form-login" method="post" action="/login" data-component="form">
    <fieldset id="fieldset-credentials" data-role="form-section">
        <legend hidden>Login Credentials</legend>
        
        <div id="field-email" data-field="email">
            <label for="input-email">Email</label>
            <input 
                type="email" 
                id="input-email" 
                name="email"
                aria-required="true"
                aria-describedby="help-email error-email"
                aria-invalid="false"
            >
            <small id="help-email" data-role="help-text">
                Enter your registered email
            </small>
            <div id="error-email" role="alert" data-role="error-message" hidden>
                Invalid email
            </div>
        </div>
    </fieldset>
    
    <div id="login-actions" data-role="form-actions">
        <button type="submit" id="button-submit-login" data-type="primary">
            Login
        </button>
    </div>
</form>
```

### 5. Fix Card Structure

❌ **Before:**
```html
<div class="card project-card">
    <div class="card-header">
        <h3 class="card-title">{{ project.name }}</h3>
        <span class="badge">Active</span>
    </div>
    <div class="card-body">
        <p>{{ project.description }}</p>
    </div>
    <div class="card-footer">
        <a href="/projects/{{ project.id }}" class="btn">View</a>
    </div>
</div>
```

✅ **After:**
```html
<article 
    id="project-{{ project.id }}"
    data-component="project-card"
    data-status="active"
    data-project-id="{{ project.id }}"
>
    <header data-role="card-header">
        <h3 id="project-title-{{ project.id }}">
            <a href="/projects/{{ project.id }}">{{ project.name }}</a>
        </h3>
        <span data-role="status-badge" data-status="active">
            Active
        </span>
    </header>
    
    <div data-role="card-body">
        <p data-role="description">{{ project.description }}</p>
    </div>
    
    <footer data-role="card-footer">
        <nav data-role="card-actions">
            <a href="/projects/{{ project.id }}" role="button" data-type="primary">
                View Details
            </a>
        </nav>
    </footer>
</article>
```

### 6. Fix Navigation Structure

❌ **Before:**
```html
<nav class="navbar">
    <ul class="nav-list">
        <li class="nav-item active">
            <a href="/" class="nav-link">Home</a>
        </li>
    </ul>
</nav>
```

✅ **After:**
```html
<nav id="main-nav" aria-label="Main navigation">
    <ul data-role="nav-primary">
        <li>
            <a href="/" id="link-home" aria-current="page">Home</a>
        </li>
    </ul>
</nav>
```

### 7. Add ARIA Attributes

Always include appropriate ARIA attributes for accessibility:

```html
<!-- Navigation -->
<nav aria-label="Main navigation">
<nav aria-labelledby="heading-section">

<!-- Current page -->
<a href="/current" aria-current="page">

<!-- Form validation -->
<input aria-invalid="true" aria-describedby="error-field">
<div id="error-field" role="alert">

<!-- Live regions -->
<div role="alert" aria-live="polite">

<!-- Expandable content -->
<button aria-expanded="false" aria-controls="content-id">
<div id="content-id" hidden>

<!-- Required fields -->
<input aria-required="true">
```

## Template-Specific Migration Instructions

### Profile Template (`profile.html`)

1. Change main container:
```html
<!-- From -->
<div class="profile-container">

<!-- To -->
<section id="profile-main" data-component="profile" data-user-id="{{ profile.id }}">
```

2. Update sections:
```html
<!-- From -->
<div class="about-section">

<!-- To -->
<section id="section-about" data-section="about" aria-labelledby="heading-about">
    <h2 id="heading-about">About</h2>
```

3. Fix skills list:
```html
<!-- From -->
<ul class="skills-list">

<!-- To -->
<ul id="profile-skills-list" data-role="skills-list">
    {% for skill in skills %}
    <li data-skill="{{ skill }}">{{ skill }}</li>
    {% endfor %}
</ul>
```

### Projects Template (`projects.html`)

1. Update main container:
```html
<section id="projects-main" data-component="projects-page">
```

2. Fix search bar:
```html
<div id="projects-search" data-component="search-bar">
    <form id="form-search-projects" method="get" action="/projects">
        <div id="field-search" data-field="search">
            <label for="input-search" hidden>Search projects</label>
            <input type="search" id="input-search" name="q">
            <button type="submit" id="button-search" data-type="primary">
                Search
            </button>
        </div>
    </form>
</div>
```

3. Fix project grid:
```html
<section id="section-projects-list" data-section="projects-list" data-layout="grid">
    <div data-role="content-container">
        {% for project in projects %}
        <!-- Project card here -->
        {% endfor %}
    </div>
</section>
```

### Equipment Templates

1. List template structure:
```html
<section id="equipment-main" data-component="equipment-page">
    <header id="equipment-header" data-role="page-header">
        <h1 id="heading-equipment">Equipment Management</h1>
    </header>
    
    <nav id="equipment-controls" data-role="page-controls">
        <!-- Control buttons -->
    </nav>
    
    <section id="section-equipment-list" data-section="equipment-list">
        <!-- Equipment cards -->
    </section>
</section>
```

2. Equipment card:
```html
<article 
    id="equipment-{{ item.id }}"
    data-component="equipment-card"
    data-status="{% if item.available %}available{% else %}unavailable{% endif %}"
    data-equipment-id="{{ item.id }}"
>
    <!-- Card content -->
</article>
```

### Error Templates

1. Error page structure:
```html
<section id="error-{{ code }}" data-component="error-page" data-error-code="{{ code }}">
    <header id="error-header" data-role="error-header">
        <h1 id="heading-error">{{ code }} - {{ message }}</h1>
    </header>
    
    <div data-role="error-content">
        <p data-role="error-description">{{ description }}</p>
    </div>
    
    <nav id="error-actions" data-role="error-actions">
        <a href="/" id="link-home" role="button" data-type="primary">
            Go Home
        </a>
    </nav>
</section>
```

## Common Patterns Reference

### Empty States
```html
<div id="empty-state-[context]" data-role="empty-state" data-state="empty">
    <h3 id="heading-empty-[context]">No items found</h3>
    <p data-role="empty-message">Description of empty state</p>
    <nav data-role="empty-state-actions">
        <a href="#" role="button" data-type="primary">Call to Action</a>
    </nav>
</div>
```

### Loading States
```html
<section data-state="loading" aria-busy="true" aria-label="Loading content">
    <!-- Content that's loading -->
</section>
```

### Alerts/Messages
```html
<div 
    id="alert-[context]-[type]"
    role="alert"
    aria-live="polite"
    data-component="alert"
    data-type="[success|error|warning|info]"
>
    Message content
</div>
```

### Pagination
```html
<nav id="pagination-[context]" aria-label="Pagination" data-component="pagination">
    <ul data-role="pagination-list">
        <li><a href="?page=1" aria-current="page">1</a></li>
        <li><a href="?page=2">2</a></li>
        <li><a href="?page=3">3</a></li>
    </ul>
</nav>
```

### Tabs
```html
<div id="[context]-tabs" data-component="tabs">
    <nav role="tablist" aria-label="[Context] tabs">
        <button role="tab" id="tab-[name]" aria-selected="true" aria-controls="panel-[name]">
            Tab 1
        </button>
    </nav>
    <div role="tabpanel" id="panel-[name]" aria-labelledby="tab-[name]">
        Panel content
    </div>
</div>
```

## Testing After Migration

### 1. Validate HTML Structure
- [ ] No CSS classes remain
- [ ] All IDs follow naming convention
- [ ] Data attributes properly applied
- [ ] ARIA attributes present

### 2. Test Functionality
- [ ] Forms submit correctly
- [ ] Links work
- [ ] JavaScript interactions function
- [ ] No console errors

### 3. Check Accessibility
- [ ] Keyboard navigation works
- [ ] Screen reader compatible
- [ ] Focus states visible
- [ ] ARIA attributes correct

### 4. Verify Styling
- [ ] CSS selectors match new structure
- [ ] Themes apply correctly
- [ ] Responsive design works
- [ ] Print styles function

## Migration Script

For bulk updates, use these search/replace patterns:

```regex
# Remove class attributes
Find: class="[^"]*"
Replace: (remove)

# Convert div to semantic elements
Find: <div class="card">
Replace: <article data-component="card">

# Add IDs to headings
Find: <h([1-6])>([^<]+)</h([1-6])>
Replace: <h$1 id="heading-[context]">$2</h$3>

# Fix buttons
Find: <button class="btn btn-([^"]+)">
Replace: <button data-type="$1">
```

## Troubleshooting

### CSS Not Applying
- Verify selectors match new structure
- Check specificity isn't too high
- Ensure CSS files are loaded

### JavaScript Broken
- Update selectors in JavaScript to use IDs/data attributes
- Replace class-based queries with attribute queries
- Test event listeners still attached

### Accessibility Issues
- Add missing ARIA attributes
- Ensure proper heading hierarchy
- Include focus management

## Resources

- [HTML & CSS Guidelines](/docs/HTML_CSS_GUIDELINES.md)
- [CSS Variables](/static/css/base/variables.css)
- [Example Components](/templates/example_components.html)
- [MDN ARIA Guide](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA)

---

*Document version: 2.0 - Last updated: 2024*