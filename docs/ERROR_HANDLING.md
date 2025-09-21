# Error Handling in SlateHub

## Overview

SlateHub uses a sophisticated error handling system that automatically renders appropriate responses based on the client type:
- **HTML error pages** for browser requests (matching the site's design)
- **JSON responses** for API requests

This ensures a consistent user experience while maintaining API compatibility.

## Key Features

- ✅ Automatic content negotiation (HTML vs JSON)
- ✅ Beautiful, branded error pages for browsers
- ✅ Consistent JSON error format for APIs
- ✅ Request ID tracking with ULIDs
- ✅ CSS-only design following SlateHub guidelines
- ✅ Dark/light theme support
- ✅ Mobile responsive

## How It Works

### Content Negotiation

The system checks the `Accept` header to determine the response format:

```rust
// Browser request (Accept: text/html)
GET /nonexistent-page
Accept: text/html
→ Returns styled HTML error page

// API request (Accept: application/json)
GET /api/nonexistent-endpoint
Accept: application/json
→ Returns JSON error response
```

### Error Types

The `Error` enum in `src/error.rs` defines all possible error types:

| Error Type | HTTP Status | Description |
|------------|-------------|-------------|
| `NotFound` | 404 | Resource not found |
| `Unauthorized` | 401 | Authentication required |
| `Forbidden` | 403 | Access denied |
| `BadRequest` | 400 | Invalid request parameters |
| `Validation` | 422 | Validation errors |
| `Conflict` | 409 | Resource conflict |
| `Internal` | 500 | Server error |
| `Database` | 500 | Database operation failed |
| `Template` | 500 | Template rendering failed |
| `ExternalService` | 502 | External service error |

## Usage in Routes

### Basic Usage

```rust
use crate::error::Error;

async fn my_route() -> Result<Html<String>, Error> {
    // This will automatically render an HTML 404 page or JSON error
    return Err(Error::NotFound);
}
```

### With Custom Messages

```rust
async fn create_item(data: Json<ItemData>) -> Result<Json<Item>, Error> {
    if data.name.is_empty() {
        return Err(Error::BadRequest("Name is required"));
    }
    
    // Process item...
}
```

### With Request Context

For richer error pages with request details:

```rust
use crate::middleware::ErrorWithContext;

async fn protected_route(
    headers: HeaderMap,
    req: Request,
) -> Response {
    let request_id = req.request_id().map(|id| id.to_string());
    let path = req.uri().path().to_string();
    
    if !authorized {
        let error = Error::Unauthorized;
        return error.with_context(&headers, Some(path), request_id);
    }
    
    // Handle request...
}
```

## Error Page Structure

HTML error pages follow the SlateHub CSS-only design system:

```html
<section data-component="error-page" data-type="404">
    <header data-role="error-header">
        <h1>404</h1>
        <h2>Page Not Found</h2>
    </header>
    
    <div data-role="error-body">
        <p>The page you're looking for doesn't exist.</p>
        
        <nav data-role="error-actions">
            <a href="/" data-type="primary">Go to Homepage</a>
            <a href="/login" data-type="secondary">Sign In</a>
        </nav>
    </div>
</section>
```

## JSON Error Format

API errors return consistent JSON structure:

```json
{
    "error": "Resource not found",
    "status": 404,
    "request_id": "01K5GRSBB5F1J7J598NQMJVAZ7",
    "timestamp": "2024-01-15T10:30:00Z"
}
```

## Styling Error Pages

Error pages are styled via `/static/css/pages/errors.css`:

### CSS Variables Used
- `--primary`: Primary color for buttons
- `--card-background-color`: Background for detail boxes
- `--muted-color`: Secondary text color
- `--error-color`: Error text color (validation)

### Responsive Design
- Mobile-first approach
- Stacks navigation buttons on small screens
- Adjusts typography for readability

### Animations
- Fade-in animation on load
- Gradient pulse on error codes
- Hover effects on buttons

### Theme Support
```css
/* Automatically respects user's theme preference */
[data-theme="dark"] [data-component="error-page"] {
    /* Dark theme styles */
}
```

## Testing Error Pages

### Manual Testing

You can test error pages using the test endpoint:

```bash
# Test 404 error page (browser)
curl -H "Accept: text/html" http://localhost:3000/api/test-error/404

# Test 404 JSON response (API)
curl -H "Accept: application/json" http://localhost:3000/api/test-error/404

# Test other error codes
curl -H "Accept: text/html" http://localhost:3000/api/test-error/401
curl -H "Accept: text/html" http://localhost:3000/api/test-error/500
```

### Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_status_codes() {
        assert_eq!(Error::NotFound.status(), StatusCode::NOT_FOUND);
        assert_eq!(Error::Unauthorized.status(), StatusCode::UNAUTHORIZED);
    }
}
```

## Middleware Integration

The error handling integrates with request middleware:

1. **Request ID Middleware**: Adds ULID to each request
2. **Error Response Middleware**: Intercepts error responses
3. **Logging Middleware**: Logs errors with request context

## Best Practices

### Do's ✅

- Use specific error types (`NotFound`, `Unauthorized`) rather than generic `Internal`
- Include helpful error messages for `BadRequest` and `Validation` errors
- Let the system handle content negotiation automatically
- Use `with_context()` when you need request details in error pages

### Don'ts ❌

- Don't return raw status codes - use the `Error` enum
- Don't render error templates manually - use the error system
- Don't forget to handle database errors with proper error conversion
- Don't expose sensitive information in error messages

## Customization

### Adding New Error Types

1. Add variant to `Error` enum in `src/error.rs`
2. Map to appropriate HTTP status code
3. Add custom message handling if needed

### Customizing Error Page Design

Edit `/static/css/pages/errors.css` following the CSS-only design principles:
- No CSS classes in HTML
- Use data attributes for styling hooks
- Maintain semantic HTML structure

## Performance Considerations

- Error pages are rendered on-demand (not cached)
- Minimal CSS and no JavaScript for fast loading
- ULID request IDs enable efficient log correlation
- Graceful fallback if template rendering fails

## Security

- Error messages sanitized to prevent XSS
- Internal error details logged but not exposed to users
- Request IDs help track suspicious patterns
- Rate limiting errors (429) include retry information

## Troubleshooting

### Error Page Not Displaying

1. Check `Accept` header - must contain `text/html`
2. Verify CSS file is accessible at `/static/css/pages/errors.css`
3. Check browser console for CSS loading errors

### Wrong Error Type

1. Verify error is created with correct variant
2. Check middleware ordering - error handler should be early in chain
3. Ensure route returns `Result<T, Error>` type

### Styling Issues

1. Verify theme is set correctly in localStorage
2. Check CSS specificity - data attributes should work
3. Test in different browsers for compatibility

## Related Documentation

- [HTML & CSS Guidelines](./HTML_CSS_GUIDELINES.md) - CSS-only design principles
- [ULID Request IDs](../server/docs/ULID_REQUEST_IDS.md) - Request ID implementation
- [Middleware](../server/src/middleware/README.md) - Middleware chain details