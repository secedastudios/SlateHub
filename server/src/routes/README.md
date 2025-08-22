# Routes Module Structure

This module organizes all HTTP routes for the SlateHub server application using a modular architecture for better maintainability and scalability.

## Directory Structure

```
routes/
├── mod.rs      # Main router composition and middleware setup
├── api.rs      # API endpoints (/api/*)
├── auth.rs     # Authentication routes (signup, login, logout)
├── pages.rs    # Page rendering routes (/, /projects, /people, /about)
└── README.md   # This file
```

## Module Descriptions

### `mod.rs`
The main entry point that:
- Composes all sub-routers into the main application router
- Configures middleware (compression, tracing)
- Sets up static file serving

### `pages.rs`
Handles all page rendering routes:
- `/` - Home page
- `/projects` - Projects listing
- `/people` - People directory
- `/about` - About page

### `auth.rs`
Manages authentication flow:
- `GET /signup` - Signup form
- `POST /signup` - Process signup
- `GET /login` - Login form
- `POST /login` - Process login
- `POST /logout` - Logout user

### `api.rs`
REST API and SSE endpoints:
- `/api/health` - Health check endpoint
- `/api/stats` - Platform statistics
- `/api/sse/stats` - Real-time stats stream
- `/api/sse/activity` - Real-time activity feed

## Adding New Routes

### 1. Adding to an Existing Module

To add a new route to an existing module (e.g., a new API endpoint):

```rust
// In api.rs
async fn new_endpoint() -> Json<ResponseType> {
    // Implementation
}

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/new-endpoint", get(new_endpoint)) // Add here
        // ... other routes
}
```

### 2. Creating a New Module

For a new route category (e.g., organization routes):

1. Create a new file `orgs.rs`:
```rust
use axum::{Router, Json, routing::get};
use crate::error::Error;

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_orgs))
        .route("/:id", get(get_org))
}

async fn list_orgs() -> Result<Json<Vec<Org>>, Error> {
    // Implementation
}

async fn get_org(Path(id): Path<String>) -> Result<Json<Org>, Error> {
    // Implementation
}
```

2. Add the module to `mod.rs`:
```rust
mod orgs;  // Add module declaration

pub fn app() -> Router {
    Router::new()
        .merge(pages::router())
        .merge(auth::router())
        .nest("/api", api::router())
        .nest("/orgs", orgs::router())  // Mount the new router
        // ... rest of configuration
}
```

## Route Patterns

### RESTful Resources
```rust
Router::new()
    .route("/items", get(list_items).post(create_item))
    .route("/items/:id", get(get_item).put(update_item).delete(delete_item))
```

### Nested Routes
```rust
Router::new()
    .nest("/admin", admin::router())  // All admin routes under /admin/*
```

### Middleware per Route Group
```rust
Router::new()
    .route("/public", get(public_handler))
    .nest("/protected", protected_routes())
    .layer(RequireAuth)  // Only applies to /protected/*
```

## Best Practices

1. **Module Organization**: Keep related routes together in the same module
2. **Error Handling**: Use the centralized `Error` type from `crate::error`
3. **Logging**: Use appropriate tracing macros (`debug!`, `info!`, `error!`)
4. **Type Safety**: Define request/response types in the models module
5. **Documentation**: Add doc comments to public functions and complex handlers

## Testing Routes

Routes can be tested using the `axum::test` helpers:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let app = router();
        
        let response = app
            .oneshot(Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
```
