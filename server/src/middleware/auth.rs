//! Authentication middleware and extractors.
//!
//! [`auth_middleware`] sits inside the error-response layer and outside the
//! activity layer in the stack built by [`crate::routes::app`]. For every
//! request it looks for a JWT — first in the `Authorization: Bearer` header
//! (used by API clients such as the Chrome extension), then in the
//! `auth_token` cookie set at login. When the token decodes and its `sub`
//! claim resolves to an existing person, the middleware inserts
//! `Arc<CurrentUser>` (an alias for [`SessionUser`]) into the request
//! extensions. A missing or invalid token never fails the request here; the
//! request simply continues anonymously, and enforcement is left to the
//! [`AuthenticatedUser`] extractor and individual handlers.

use axum::{
    extract::{FromRequestParts, Request},
    http::{StatusCode, request::Parts},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use std::sync::Arc;
use tracing::{debug, error, info_span, warn};

use crate::{
    auth,
    error::Error,
    models::person::{Person, SessionUser},
    record_id_ext::RecordIdExt,
};
use surrealdb::types::RecordId;

/// The user type stored in request extensions, aliased from [`SessionUser`]
/// so existing handler code can keep referring to `CurrentUser`.
pub type CurrentUser = SessionUser;

/// Extract and verify the request's JWT, populating the current user.
///
/// The token is read from the `Authorization: Bearer` header first (API
/// clients such as the Chrome extension), falling back to the `auth_token`
/// cookie. When the token decodes and its `sub` claim resolves to an
/// existing person, an `Arc<CurrentUser>` is inserted into the request
/// extensions for downstream middleware and handlers.
///
/// # Errors
///
/// This function always returns `Ok`. A missing token, a failed decode, or
/// an unknown user is logged and the request continues without the user
/// extension — rejecting unauthenticated requests is the job of
/// [`AuthenticatedUser`] and the handlers themselves.
pub async fn auth_middleware(
    jar: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    debug!("Auth middleware: Processing request for {}", request.uri());

    // List all cookies for debugging
    debug!("Auth middleware: Available cookies:");
    for cookie in jar.iter() {
        debug!(
            "  Cookie: name={}, value_len={}",
            cookie.name(),
            cookie.value().len()
        );
    }

    // Check Authorization header first (for API clients like Chrome extension),
    // then fall back to auth_token cookie
    let token_from_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    if let Some(token) = token_from_header
        .as_deref()
        .or(jar.get("auth_token").map(|c| c.value()))
    {
        debug!("Auth middleware: Found auth token (len={})", token.len());

        // Decode JWT to extract user information
        match auth::decode_jwt(token) {
            Ok(claims) => {
                let user_id = &claims.sub;
                debug!(
                    "Auth middleware: Decoded JWT successfully, user ID: '{}', username: '{}'",
                    user_id, claims.username
                );

                // Get user info from database using the ID from JWT
                match get_user_from_id(user_id).await {
                    Ok(user) => {
                        debug!(
                            "Auth middleware: Successfully authenticated user: '{}' with id: '{}' and email: '{}'",
                            user.username, user.id, user.email
                        );
                        // Insert user into request extensions so handlers can access it
                        request.extensions_mut().insert(Arc::new(user));
                        debug!("Auth middleware: User inserted into request extensions");
                    }
                    Err(e) => {
                        warn!(
                            "Auth middleware: Could not fetch user info for ID '{}': {}",
                            user_id, e
                        );
                        debug!(
                            "Auth middleware: User might not exist, continuing without authentication"
                        );
                        // Continue without user in extensions
                    }
                }
            }
            Err(e) => {
                debug!("Auth middleware: Failed to decode JWT: {}", e);
                debug!(
                    "Auth middleware: Token might be invalid or expired, continuing without authentication"
                );
                // Continue without user in extensions
            }
        }
    } else {
        debug!(
            "Auth middleware: No auth token found (checked Authorization header and auth_token cookie)"
        );
    }

    debug!("Auth middleware: Passing request to next handler");
    // Continue to the next middleware/handler
    Ok(next.run(request).await)
}

/// Extract user information from the JWT-sub claim using the Person model.
/// `user_id` is the raw JWT `sub` value — either `"person:abc"` (current
/// format) or a bare key (legacy/fallback). We parse it into a `RecordId`
/// once here so the rest of the codebase never has to.
async fn get_user_from_id(user_id: &str) -> Result<CurrentUser, Error> {
    let span = info_span!(
        "fetch_user",
        user_id = %user_id,
    );
    let _enter = span.enter();

    let rid: RecordId = if user_id.starts_with("person:") {
        RecordId::parse_simple(user_id)
            .map_err(|e| Error::Internal(format!("invalid JWT sub claim: {e}")))?
    } else {
        RecordId::new("person", user_id)
    };

    debug!(record_id = %rid.to_raw_string(), "Calling Person::find_by_record_id");
    match Person::find_by_record_id(&rid).await {
        Ok(Some(person)) => {
            debug!(
                person_id = ?person.id,
                username = %person.username,
                email = %person.email,
                "Person found in database"
            );
            let session_user = person.to_session_user();
            debug!(
                session_user = ?session_user,
                "Converted to SessionUser"
            );
            Ok(session_user)
        }
        Ok(None) => {
            error!(
                record_id = %rid.to_raw_string(),
                "Person not found in database"
            );
            Err(Error::Internal("User not found".to_string()))
        }
        Err(e) => {
            error!(
                "get_user_from_id: Failed to query user data for ID '{}': {}",
                rid.to_raw_string(),
                e
            );
            debug!("get_user_from_id: Error details: {:?}", e);
            Err(Error::database("Failed to get user information"))
        }
    }
}

/// Extension trait to easily get the current user from a request.
pub trait UserExtractor {
    /// Return the authenticated user from the request extensions, if
    /// [`auth_middleware`] resolved one for this request.
    fn get_user(&self) -> Option<Arc<CurrentUser>>;
}

impl UserExtractor for Request {
    fn get_user(&self) -> Option<Arc<CurrentUser>> {
        self.extensions().get::<Arc<CurrentUser>>().cloned()
    }
}

/// Extractor for handlers that require an authenticated user.
///
/// Reads the `Arc<CurrentUser>` placed in the request extensions by
/// [`auth_middleware`]. Because it implements `FromRequestParts`, it can be
/// combined with `Form` and other body-consuming extractors (list it before
/// the body extractor in the handler signature).
///
/// # Errors
///
/// Extraction rejects with [`Error::Unauthorized`] when no user is present,
/// which the error-response middleware renders as a 401 page or JSON body.
pub struct AuthenticatedUser(pub Arc<CurrentUser>);

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Arc<CurrentUser>>()
            .cloned()
            .map(AuthenticatedUser)
            .ok_or(Error::Unauthorized)
    }
}
