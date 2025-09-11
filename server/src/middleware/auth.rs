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
};

// Re-export SessionUser as CurrentUser for compatibility
pub type CurrentUser = SessionUser;

/// Middleware to extract and verify JWT token from cookies
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

    // Check if auth_token cookie exists
    if let Some(auth_cookie) = jar.get("auth_token") {
        let token = auth_cookie.value();
        debug!(
            "Auth middleware: Found auth_token (len={}) in cookies",
            token.len()
        );

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
        debug!("Auth middleware: Missing auth_token cookie");
    }

    debug!("Auth middleware: Passing request to next handler");
    // Continue to the next middleware/handler
    Ok(next.run(request).await)
}

/// Extract user information from ID using the Person model
async fn get_user_from_id(user_id: &str) -> Result<CurrentUser, Error> {
    let span = info_span!(
        "fetch_user",
        user_id = %user_id,
        stripped_id = tracing::field::Empty,
    );
    let _enter = span.enter();

    debug!("Starting user fetch");

    // Extract just the ID part if it's in format "person:xxxxx"
    let id = if user_id.starts_with("person:") {
        &user_id[7..]
    } else {
        user_id
    };

    span.record("stripped_id", &id);
    debug!(stripped_id = %id, "Calling Person::find_by_id");
    match Person::find_by_id(id).await {
        Ok(Some(person)) => {
            debug!(
                person_id = %person.id,
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
                stripped_id = %id,
                "Person not found in database"
            );
            Err(Error::Internal("User not found".to_string()))
        }
        Err(e) => {
            error!(
                "get_user_from_id: Failed to query user data for ID '{}': {}",
                id, e
            );
            debug!("get_user_from_id: Error details: {:?}", e);
            Err(Error::database("Failed to get user information"))
        }
    }
}

/// Extension trait to easily get the current user from a request
pub trait UserExtractor {
    fn get_user(&self) -> Option<Arc<CurrentUser>>;
}

impl UserExtractor for Request {
    fn get_user(&self) -> Option<Arc<CurrentUser>> {
        self.extensions().get::<Arc<CurrentUser>>().cloned()
    }
}

/// Extractor for authenticated users that can be used with Form and other body-consuming extractors
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
