# SlateHub Design System

## Overview

SlateHub uses a clean, minimalist design system focused on clarity, professionalism, and usability for the TV, film, and content industries. The design emphasizes typography, whitespace, and subtle interactions.

## Design Principles

1. **Minimalist Aesthetic** - Clean interfaces with generous whitespace
2. **Typography First** - Strong typographic hierarchy using Denton XCondensed for headlines
3. **Semantic Structure** - HTML defines meaning, CSS provides all styling
4. **Accessibility** - WCAG compliant with keyboard navigation and screen reader support
5. **Responsive** - Mobile-first design that scales elegantly

## Color Palette

### Primary Colors
- **Background Primary**: `#D6D8CA` - Soft sage green (main background)
- **Background Secondary**: `#E5E7DB` - Lighter sage
- **Background Tertiary**: `#F2F3ED` - Very light sage
- **Background White**: `#FFFFFF` - Pure white for cards/panels
- **Background Dark**: `#2A2A2A` - Dark gray

### Text Colors
- **Text Primary**: `#171717` - Near black for main content
- **Text Secondary**: `#5A5A5A` - Medium gray for secondary text
- **Text Tertiary**: `#8A8A8A` - Light gray for hints/placeholders
- **Text Light**: `#FFFFFF` - White text on dark backgrounds
- **Text Muted**: `#9CA39E` - Muted gray-green

### Accent Colors
- **Accent Primary**: `#EB5437` - Vibrant red-orange (CTAs, links)
- **Accent Hover**: `#D74328` - Darker red-orange for hover states
- **Accent Light**: `#F6917F` - Light coral
- **Accent Background**: `#FEF4F2` - Very light peach

### Semantic Colors
- **Success**: `#4A7C59` / Background: `#E8F3EC`
- **Warning**: `#D4A574` / Background: `#FCF5ED`
- **Error**: `#C44536` / Background: `#FCEBE9`
- **Info**: `#5B7C99` / Background: `#EEF3F7`

### Border Colors
- **Border Default**: `#C8CAB9` - Soft gray-green
- **Border Light**: `#E0E2D5` - Very light border
- **Border Dark**: `#9B9D8E` - Darker border for emphasis

## Typography

### Font Families
- **Display Font**: Denton XCondensed Test, 400 weight
  - Used for all headlines (h1-h6)
  - Logo and branding
  - Always uppercase with letter-spacing
  - Embedded font files served from `/static/fonts/`
  
- **Body Font**: Helvetica Now Display
  - Used for all body text, forms, and UI elements
  - Clean and highly readable
  - Embedded font files served from `/static/fonts/`

- **Monospace Font**: SF Mono, Monaco, Courier New
  - Used for code blocks and technical content

### Type Scale
```css
--text-xs: 0.75rem;    /* 12px */
--text-sm: 0.875rem;   /* 14px */
--text-base: 1rem;     /* 16px */
--text-lg: 1.125rem;   /* 18px */
--text-xl: 1.25rem;    /* 20px */
--text-2xl: 1.5rem;    /* 24px */
--text-3xl: 1.875rem;  /* 30px */
--text-4xl: 2.25rem;   /* 36px */
--text-5xl: 3rem;      /* 48px */
```

### Heading Styles
- All headings use Denton XCondensed Test
- Text transform: UPPERCASE
- Letter spacing: 0.04em
- Font weight: 400 (regular)

## Spacing System

```css
--space-xs: 0.25rem;   /* 4px */
--space-sm: 0.5rem;    /* 8px */
--space-md: 1rem;      /* 16px */
--space-lg: 1.5rem;    /* 24px */
--space-xl: 2rem;      /* 32px */
--space-2xl: 3rem;     /* 48px */
--space-3xl: 4rem;     /* 64px */
--space-4xl: 6rem;     /* 96px */
```

## Components

### Buttons

#### Primary Button
- Background: `#EB5437` (accent color)
- Text: White
- Hover: Darker accent color
- Used for main CTAs

#### Secondary Button
- Background: `#E5E7DB` (secondary background)
- Text: Primary text color
- Border: Default border color
- Used for secondary actions

#### Danger Button
- Background: `#C44536` (error color)
- Text: White
- Used for destructive actions

### Cards
- Background: White
- Border radius: 8px (--radius-lg)
- Subtle shadow
- Hover: Elevated shadow with slight translate

### Forms
- Clean, minimal inputs with subtle borders
- Focus state: Accent color border with soft shadow
- Labels: Uppercase, small text with letter spacing
- Error states: Red border with error message below

### Navigation
- Sticky header with white background
- Logo: Denton XCondensed Test, uppercase
- Nav items: Uppercase with letter spacing
- Active state: Accent color underline
- Mobile: Collapsible menu

### Footer
- Minimal design with essential links
- Small, elegant typography
- Centered on mobile

## Layout

### Containers
- Max width: 1280px
- Narrow container: 768px
- Wide container: 1440px
- Consistent padding: 24px on mobile, 32px on desktop

### Grid System
- CSS Grid for layouts
- Auto-fill with minmax for responsive cards
- Gap: 24px (--space-lg)

## Shadows

```css
--shadow-sm: 0 1px 2px rgba(23, 23, 23, 0.05);
--shadow-md: 0 2px 4px rgba(23, 23, 23, 0.08);
--shadow-lg: 0 4px 8px rgba(23, 23, 23, 0.1);
--shadow-xl: 0 8px 16px rgba(23, 23, 23, 0.12);
```

## Border Radii

```css
--radius-sm: 2px;
--radius-md: 4px;
--radius-lg: 8px;
--radius-xl: 12px;
--radius-full: 9999px;
```

## Transitions

```css
--transition-fast: 150ms ease-in-out;
--transition-base: 250ms ease-in-out;
--transition-slow: 350ms ease-in-out;
```

## Responsive Breakpoints

- Mobile: < 768px
- Tablet: 768px - 1024px
- Desktop: > 1024px
- Wide: > 1440px

### Mobile Adjustments
- Smaller font sizes
- Reduced spacing
- Stacked navigation
- Full-width buttons
- Single column layouts

## Accessibility Features

### Focus States
- Visible focus outline: 2px solid accent color
- Outline offset: 2px
- Consistent across all interactive elements

### Skip Links
- "Skip to main content" link for keyboard navigation
- Becomes visible on focus

### Motion Preferences
- Respects `prefers-reduced-motion`
- Disables animations when requested

### High Contrast Mode
- Increased border widths
- Underlined links
- Enhanced visual separation

### Screen Reader Support
- Semantic HTML structure
- Proper ARIA labels
- `.sr-only` class for screen reader only content

## Implementation

### CSS Architecture
- Single main.css file with organized sections
- No CSS classes for styling
- Semantic selectors using:
  - Element types
  - IDs for unique elements
  - Data attributes for components
  - ARIA attributes for states

### File Structure
```
/static/
├── css/
│   ├── main.css           # Complete design system
│   ├── legal.css          # Legal pages extension
│   └── components/        # Component-specific styles
│       └── avatar.css
└── fonts/
    ├── Denton XCondensed Test/  # Display font family
    │   └── *.otf                 # Various weights
    └── helveticanowtext-*.ttf    # Body font family
```

### Usage Example

```html
<!-- Button -->
<button data-type="primary">Save Changes</button>

<!-- Card -->
<article data-component="card">
    <header data-role="card-header">
        <h3>Card Title</h3>
    </header>
    <div data-role="card-body">
        <p>Card content</p>
    </div>
</article>

<!-- Form Field -->
<div data-field="email">
    <label for="input-email">Email Address</label>
    <input type="email" id="input-email" required>
    <small data-role="help-text">We'll never share your email</small>
</div>
```

## Maintenance

### Adding New Colors
1. Add to CSS custom properties in `:root`
2. Create semantic variants if needed
3. Ensure adequate contrast ratios

### Adding New Components
1. Follow existing naming patterns
2. Use data attributes for identification
3. Ensure responsive behavior
4. Test accessibility

### Performance Considerations
- CSS is minified in production
- Custom properties for consistent design tokens
- Minimal CSS file size (~25KB uncompressed)
- No external dependencies
- Embedded fonts with `font-display: swap` for optimal loading

## Browser Support

- Chrome/Edge: Last 2 versions
- Firefox: Last 2 versions  
- Safari: Last 2 versions
- Mobile browsers: iOS Safari 14+, Chrome Mobile

## Version History

- **v1.1.0** - Light mode only with embedded fonts
  - Embedded Denton XCondensed Test fonts
  - Embedded Helvetica Now Display fonts
  - Removed dark mode for cleaner codebase
  - Optimized font loading

- **v1.0.0** - Initial design system release
  - Core color palette
  - Typography system
  - Basic components
  - Responsive grid
  - Accessibility features