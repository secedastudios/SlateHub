# CSS Selector Quick Reference

## Page Selectors
```css
/* Body page context */
[data-page="home"]           /* Home page */
[data-page="projects"]        /* Projects page */
[data-page="people"]          /* People page */
[data-page="profile"]         /* Profile page */
[data-page="login"]           /* Login page */
[data-page="signup"]          /* Signup page */

/* User state on body */
[data-user="authenticated"]   /* Logged in users */
[data-user="anonymous"]       /* Logged out users */
```

## Major Layout IDs
```css
/* Primary layout elements */
#site-header                  /* Main header */
#main-nav                     /* Main navigation */
#site-logo                    /* Logo/brand link */
#main-content                 /* Main content area */
#site-footer                  /* Main footer */

/* Navigation sections */
[data-role="nav-primary"]     /* Primary nav links */
[data-role="nav-user"]        /* User account nav */
[data-role="nav-brand"]       /* Brand/logo area */
```

## Component Selectors
```css
/* Card components */
[data-component="project-card"]
[data-component="person-card"]
[data-component="content-card"]
[data-component="auth-form"]
[data-component="modal"]
[data-component="search-bar"]
[data-component="user-menu"]
[data-component="theme-toggle"]

/* Card parts */
[data-role="card-header"]
[data-role="card-body"]
[data-role="card-footer"]
[data-role="card-actions"]
```

## Form Selectors
```css
/* Form containers */
#form-login                   /* Login form */
#form-signup                  /* Signup form */
#form-search-projects         /* Project search */
#form-search-people           /* People search */

/* Form structure */
[data-component="form"]
[data-role="form-section"]
[data-role="form-header"]
[data-role="form-actions"]
[data-role="form-footer"]

/* Form fields pattern */
#input-email                  /* Email input */
#input-password               /* Password input */
#input-username               /* Username input */
#error-email                  /* Email error message */
#help-email                   /* Email help text */

/* Field containers */
[data-field="email"]
[data-field="password"]
[data-field="username"]
```

## Button Types
```css
/* Button variations */
button[data-type="primary"]
button[data-type="secondary"]
button[data-type="danger"]
a[role="button"][data-type="primary"]
a[role="button"][data-type="secondary"]

/* Button purposes */
button[type="submit"]
button[type="button"]
button[type="reset"]
button[data-action="close"]
button[data-action="cancel"]
button[data-action="message"]
```

## State Selectors
```css
/* Loading/processing states */
[data-state="loading"]
[data-state="submitting"]
[data-state="processing"]

/* Content states */
[data-state="empty"]
[data-state="error"]
[data-state="success"]
[data-state="ready"]

/* ARIA states */
[aria-current="page"]         /* Current page in nav */
[aria-expanded="true"]        /* Expanded sections */
[aria-invalid="true"]         /* Invalid form fields */
[aria-selected="true"]        /* Selected tabs/items */
[aria-pressed="true"]         /* Pressed buttons */
[aria-checked="true"]         /* Checked items */
```

## Status & Types
```css
/* Status indicators */
[data-status="active"]
[data-status="pending"]
[data-status="completed"]
[data-status="on-hold"]
[data-status="published"]
[data-status="draft"]

/* Content types */
[data-type="featured"]
[data-type="blog-post"]
[data-type="project"]
[data-type="profile"]
```

## Layout Modifiers
```css
/* Layout variations */
[data-layout="grid"]
[data-layout="list"]
[data-layout="compact"]
[data-layout="expanded"]

/* Sections */
[data-section="controls"]
[data-section="projects-list"]
[data-section="people-list"]
[data-section="about"]
[data-section="experience"]
[data-section="education"]
```

## Common Patterns
```css
/* Empty states */
[data-role="empty-state"]
[data-role="empty-state-actions"]

/* Metadata */
[data-role="metadata"]
[data-role="date-range"]
[data-role="tags-list"]
[data-role="skills-list"]

/* Navigation */
[data-role="pagination"]
[data-role="breadcrumb"]
[data-role="dropdown-menu"]

/* Content elements */
[data-role="avatar"]
[data-role="badge"]
[data-role="status-badge"]
[data-role="headline"]
[data-role="description"]
[data-role="bio"]
```

## Specific Page Elements

### Projects Page
```css
#projects-main
#projects-header
#heading-projects
#section-controls
#section-projects-list
#empty-state-projects
```

### People Page
```css
#people-main
#people-header
#heading-people
#specialty-filters
#section-people-list
#empty-state-people
```

### Profile Page
```css
#profile-main
#profile-header
#profile-name
#profile-headline
#profile-bio
#section-about
#section-experience
#section-education
#section-skills
```

## Theme & Dark Mode
```css
/* Theme targeting */
[data-theme="dark"]
[data-theme="light"]

/* Dark mode overrides */
[data-theme="dark"] [data-component="card"]
[data-theme="dark"] button[data-type="primary"]
```

## Media Queries
```css
/* Common breakpoints */
@media (max-width: 768px)    /* Mobile */
@media (max-width: 1024px)   /* Tablet */
@media (min-width: 1200px)   /* Desktop */

/* Preferences */
@media (prefers-reduced-motion: reduce)
@media (prefers-contrast: high)
@media (prefers-color-scheme: dark)
@media print
```

## Pseudo-Classes
```css
/* Interactive states */
:hover
:focus
:focus-visible
:active
:visited

/* Form states */
:checked
:disabled
:required
:valid
:invalid
:placeholder-shown

/* Structural */
:first-child
:last-child
:nth-child(n)
:empty
:not()
```

## Combining Selectors
```css
/* Nested components */
[data-component="project-card"] h2
[data-component="project-card"] [data-role="metadata"]

/* Multiple attributes */
[data-component="project-card"][data-status="active"]
button[type="submit"][data-state="loading"]

/* Page-specific components */
[data-page="projects"] [data-component="project-card"]

/* State combinations */
[data-state="loading"] button[type="submit"]
form[data-state="submitting"] input
```

## Tips

1. **Start broad, then narrow**
   ```css
   [data-page="projects"] { }  /* Page-wide styles */
   [data-page="projects"] [data-component="project-card"] { }  /* Specific */
   ```

2. **Use CSS variables for consistency**
   ```css
   :root {
     --primary-color: #007bff;
     --card-padding: 1.5rem;
   }
   ```

3. **Target by meaning, not position**
   ```css
   /* Good */
   [data-role="card-header"]
   
   /* Avoid */
   article > header:first-child
   ```

4. **Leverage ARIA for states**
   ```css
   [aria-expanded="true"]  /* Instead of custom classes */
   [aria-current="page"]   /* For active states */
   ```
