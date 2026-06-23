//! HTTP middleware for the SlateHub server.
//!
//! This module collects the axum middleware that wraps every route, together
//! with the request extensions and extractors they provide. The full stack is
//! assembled in [`crate::routes::app`]; because axum applies `.layer()` calls
//! bottom-up (the last layer added becomes the outermost), the request-path
//! order is:
//!
//! 1. [`request_id_middleware`] — adopts or generates a request ID, inserts
//!    [`RequestId`] into the request extensions, wraps the rest of the stack
//!    in a tracing span, and echoes the ID back in the `X-Request-Id`
//!    response header.
//! 2. tower-http layers — `TraceLayer` (request/response logs), `CompressionLayer`,
//!    `CorsLayer` (Chrome-extension origins), and four `SetResponseHeaderLayer`s
//!    for security headers. These add spans and response headers but insert
//!    nothing into the request extensions.
//! 3. [`error_response_middleware`] — reads the [`RequestId`] for log
//!    correlation on the way in; on the way out it logs 4xx/5xx responses and
//!    rewrites those carrying an `X-Error-Message` header into full HTML
//!    error pages for clients that accept `text/html`.
//! 4. [`auth_middleware`] — decodes the JWT from the `Authorization: Bearer`
//!    header or the `auth_token` cookie and, when it resolves to a known
//!    person, inserts `Arc<CurrentUser>` into the request extensions. It
//!    never rejects a request itself.
//! 5. [`activity::activity_middleware`] — reads the `Arc<CurrentUser>`
//!    extension and, after the handler responds, records a `page_view`
//!    activity event for successful GET requests to user-facing pages.
//! 6. `DefaultBodyLimit` (50 MB) and the route handler.
//!
//! Responses unwind through the same layers in reverse order.
//!
//! The [`logging`] module also provides standalone per-request logging
//! middleware ([`logging_middleware`], [`filtered_logging_middleware`]); it is
//! re-exported here but not installed in [`crate::routes::app`], where request
//! logging comes from `TraceLayer` and [`request_id_middleware`] instead.

pub mod activity;
pub mod auth;
pub mod error_handler;
pub mod logging;
pub mod request_id;

pub use auth::{AuthenticatedUser, CurrentUser, UserExtractor, auth_middleware};
pub use error_handler::{ErrorWithContext, ResultExt, error_response_middleware};
pub use logging::{filtered_logging_middleware, logging_middleware};
pub use request_id::{RequestId, RequestIdExt, request_id_middleware};
