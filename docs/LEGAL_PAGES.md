# Legal Pages Documentation

## Overview

This document describes the Terms of Service and Privacy Policy pages added to SlateHub.

## Implementation Details

### Pages Added

1. **Terms of Service** - Available at `/terms`
2. **Privacy Policy** - Available at `/privacy`

### Files Created/Modified

#### Templates
- `/server/templates/terms.html` - Terms of Service page template
- `/server/templates/privacy.html` - Privacy Policy page template

#### Styles
- `/server/static/css/legal.css` - Styling for legal document pages

#### Code Changes
- `/server/src/templates.rs` - Added `TermsTemplate` and `PrivacyTemplate` structs
- `/server/src/routes/pages.rs` - Added route handlers for `/terms` and `/privacy`

## Key Features

### Company Information
- Company: **Seceda GmbH**
- Based in: **Germany**
- Compliant with: **GDPR and German data protection laws**
- Age requirement: **13 years and older** (with parental consent provisions for users under 18)

### Privacy Commitments
- **No third-party marketing** - Data is never shared with marketers or advertisers
- **Full data deletion** - Users can completely delete all their data at any time
- **Transparent data use** - Clear explanation of what data is collected and how it's used
- **GDPR compliance** - Full compliance with European data protection regulations

### Terms of Service Highlights
- Clear and simple language
- User responsibilities clearly outlined
- **Age requirement: 13+ (parental consent may be required for users under 18)**
- Account termination process explained
- Data ownership clarified (users retain ownership of their content)
- Governing law (Germany) specified

## Technical Implementation

### Template Structure
Both templates follow the SlateHub conventions:
- Extend `_layout.html` base template
- Use semantic HTML with proper heading hierarchy
- Include data attributes for styling hooks
- No inline styles or styling classes

### CSS Architecture
The `legal.css` file follows SlateHub's CSS guidelines:
- Uses semantic selectors only
- Targets elements via data attributes and IDs
- Responsive design with mobile-first approach
- Dark mode support
- Print styles included
- Accessibility features (high contrast support, focus management)

### Routing
Simple GET routes added to the pages router:
```rust
.route("/terms", get(terms))
.route("/privacy", get(privacy))
```

### Template Context
Both pages use the standard `BaseContext` with optional user authentication:
- Displays user info in header if authenticated
- Shows appropriate navigation based on auth state
- Consistent with other SlateHub pages

## Usage

### Accessing the Pages
After rebuilding and restarting the server:
1. Terms of Service: `http://localhost:3000/terms`
2. Privacy Policy: `http://localhost:3000/privacy`

### Linking to Legal Pages
These pages should be linked from:
- Footer (already has links prepared)
- Sign-up page (users should agree to terms)
- Account settings (for data deletion requests)

## Maintenance

### Updating Content
To update the legal content:
1. Edit the respective template file (`terms.html` or `privacy.html`)
2. Update the "Last updated" date in the template
3. Rebuild the server
4. Consider notifying users of significant changes

### Styling Changes
All styling is contained in `/server/static/css/legal.css`. The file includes:
- Component styles for legal documents
- Responsive breakpoints
- Dark mode variations
- Print styles
- Accessibility enhancements

## Compliance Notes

### GDPR Requirements Met
- ✅ Clear privacy policy
- ✅ Data portability mentioned
- ✅ Right to deletion prominently featured
- ✅ Data Protection Officer contact provided
- ✅ Supervisory authority information included
- ✅ No third-party marketing commitment
- ✅ Age-appropriate privacy provisions (13+ with parental consent considerations)
- ✅ Enhanced privacy protections for users under 18

### German Law Compliance
- ✅ German governing law specified
- ✅ Company information clearly stated
- ✅ Contact information provided
- ✅ Data stored in EU mentioned

## Future Enhancements

Consider adding:
1. Cookie consent banner (if analytics are added)
2. Data download functionality in user settings
3. Automated data deletion after account closure
4. Version history for terms changes
5. Email notifications for policy updates
6. Multi-language support (German translation)

## Testing Checklist

- [ ] Pages load correctly when not authenticated
- [ ] Pages load correctly when authenticated
- [ ] Links in footer point to correct pages
- [ ] Mobile responsive design works
- [ ] Dark mode displays correctly
- [ ] Print view is readable
- [ ] All internal links work
- [ ] External links open in new tabs
- [ ] Accessibility: keyboard navigation works
- [ ] Accessibility: screen reader compatible