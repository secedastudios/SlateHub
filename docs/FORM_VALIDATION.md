# Form Validation and Error Handling

## Overview

This document describes the form validation system in SlateHub, including custom deserializers for handling HTML form quirks and improved error handling for validation failures.

## Problem Statement

HTML forms have several quirks that can cause issues with Rust/Serde deserialization:

1. **Empty Number Fields**: When a `<input type="number">` field is left empty, browsers send an empty string `""` rather than omitting the field from the form data.
2. **Empty Optional Fields**: Empty text fields send empty strings instead of being omitted.
3. **Poor Error Messages**: Default deserialization errors like "cannot parse integer from empty string" are not user-friendly.

## Solution

### Custom Deserializers

SlateHub provides custom deserializers in `src/serde_utils.rs` to handle these HTML form quirks gracefully.

#### Available Deserializers

1. **`deserialize_optional_i32`** - For optional integer fields
2. **`deserialize_optional_u32`** - For optional positive integer fields
3. **`deserialize_optional_i64`** - For optional large integer fields
4. **`deserialize_optional_f64`** - For optional decimal number fields
5. **`deserialize_optional_string`** - For optional text fields (treats empty as None)
6. **`deserialize_string_list`** - For comma-separated string lists
7. **`deserialize_optional_string_list`** - For optional comma-separated lists

### Usage Example

```rust
use serde::Deserialize;
use crate::serde_utils::deserialize_optional_i32;

#[derive(Deserialize)]
struct CreateLocationForm {
    name: String,                    // Required field
    address: String,                  // Required field
    
    #[serde(deserialize_with = "deserialize_optional_i32")]
    max_capacity: Option<i32>,       // Optional numeric field
    
    #[serde(deserialize_with = "deserialize_optional_string")]
    description: Option<String>,     // Optional text field
}
```

### How It Works

The custom deserializers:

1. Accept `Option<String>` from the form data
2. Check if the string is `None` or empty/whitespace
3. Return `None` for empty values
4. Parse and return `Some(value)` for valid values
5. Provide clear error messages for invalid values

### HTML Form Example

```html
<form method="POST" action="/locations/create">
    <!-- Required field -->
    <input type="text" name="name" required>
    
    <!-- Optional number field with custom deserializer -->
    <input type="number" name="max_capacity" min="1" placeholder="50">
    
    <!-- If left empty, deserializer returns None instead of error -->
</form>
```

## Error Handling

### User-Friendly Validation Messages

The `Error::parse_form_validation_error()` method transforms technical error messages into user-friendly ones:

#### Before
```
Failed to deserialize form body: max_capacity: cannot parse integer from empty string
```

#### After
```
Please enter a valid number for Max Capacity
```

### Error Page Rendering

Form validation errors (HTTP 422 Unprocessable Entity) now render proper error pages with:

- Clear error title: "Invalid Input"
- User-friendly message explaining what went wrong
- Consistent styling with other error pages
- Request ID for debugging

### Status Code Handling

The error handler properly handles different HTTP status codes:

- **400 Bad Request**: "Your request couldn't be understood"
- **422 Unprocessable Entity**: "The information you provided couldn't be processed"
- **404 Not Found**: "The page you're looking for doesn't exist"
- **500 Internal Server Error**: "Something went wrong on our end"

## Implementation Details

### Form Rejection Handling

```rust
// Automatic conversion from Axum's form rejection to our Error type
impl From<axum::extract::rejection::FormRejection> for Error {
    fn from(rejection: axum::extract::rejection::FormRejection) -> Self {
        let message = rejection.body_text();
        Error::parse_form_validation_error(message)
    }
}
```

### Field Name Formatting

The system automatically formats field names for display:

- `max_capacity` → "Max Capacity"
- `contact_email` → "Contact Email"  
- `postal_code` → "Postal Code"

## Best Practices

### 1. Always Use Custom Deserializers for Optional Numeric Fields

```rust
// ✅ Good
#[serde(deserialize_with = "deserialize_optional_i32")]
max_capacity: Option<i32>,

// ❌ Bad - will fail on empty string
max_capacity: Option<i32>,
```

### 2. Provide Clear Field Labels in HTML

```html
<!-- Good - clear label and help text -->
<label for="input-max-capacity">Maximum Capacity</label>
<input type="number" id="input-max-capacity" name="max_capacity">
<small>Maximum number of people (leave blank if unlimited)</small>
```

### 3. Use Form Validation on Client Side Too

While server-side validation is essential, adding client-side validation improves UX:

```html
<input 
    type="number" 
    name="max_capacity" 
    min="1" 
    max="10000"
    pattern="[0-9]*"
>
```

### 4. Handle Errors Gracefully in Routes

```rust
async fn create_location(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<CreateLocationForm>,  // Automatically uses error handling
) -> Result<Response, Error> {
    // Additional validation
    if data.name.is_empty() {
        return Err(Error::Validation("Name is required".to_string()));
    }
    
    // Process form...
}
```

## Testing

### Unit Tests

The serde_utils module includes comprehensive tests:

```bash
cargo test --lib serde_utils
```

### Manual Testing

1. Submit a form with an empty number field
2. Verify you see a user-friendly error page (not raw error text)
3. Check that optional fields accept empty values
4. Ensure required fields show appropriate errors

## Common Scenarios

### Scenario 1: Empty Number Field

**Input**: `<input type="number" name="max_capacity" value="">`

**Without Custom Deserializer**:
- Error: "cannot parse integer from empty string"
- Raw error displayed in browser

**With Custom Deserializer**:
- Parsed as `None`
- No error for optional field

### Scenario 2: Invalid Number

**Input**: `<input type="number" name="max_capacity" value="abc">`

**Result**:
- Error: "Please enter a valid number for Max Capacity"
- Proper error page displayed

### Scenario 3: Comma-Separated Lists

**Input**: `<input type="text" name="tags" value="rust, web, axum">`

**With `deserialize_string_list`**:
- Parsed as `vec!["rust", "web", "axum"]`
- Whitespace automatically trimmed
- Empty items filtered out

## Migration Guide

To update existing forms:

1. Identify all `Option<numeric>` fields in form structs
2. Add the appropriate `#[serde(deserialize_with = "...")]` attribute
3. Import the deserializer from `crate::serde_utils`
4. Test form submission with empty fields
5. Verify error messages are user-friendly

## Summary

The form validation system in SlateHub:

- ✅ Handles HTML form quirks gracefully
- ✅ Provides user-friendly error messages
- ✅ Renders proper error pages for validation failures
- ✅ Maintains type safety with Rust/Serde
- ✅ Follows simple, idiomatic Rust patterns

This ensures a better user experience while maintaining robust validation on the server side.