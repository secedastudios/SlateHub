use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use axum_extra::extract::cookie::CookieJar;
use std::sync::Arc;
use tracing::{debug, error, warn};

use crate::{
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
    // Check if both auth_token and username cookies exist
    if let (Some(auth_cookie), Some(username_cookie)) = (jar.get("auth_token"), jar.get("username"))
    {
        let token = auth_cookie.value();
        let username = username_cookie.value();
        debug!("Found auth token and username in cookies: {}", username);

        // Try to authenticate with SurrealDB using the token
        match Person::authenticate_token(token).await {
            Ok(_) => {
                debug!("Token is valid, fetching user info for: {}", username);
                // Token is valid, get user info from database
                match get_user_from_username(username).await {
                    Ok(user) => {
                        debug!("Authenticated user: {}", user.username);
                        // Insert user into request extensions so handlers can access it
                        request.extensions_mut().insert(Arc::new(user));
                    }
                    Err(e) => {
                        warn!("Could not fetch user info for {}: {}", username, e);
                        // Continue without user in extensions
                    }
                }
            }
            Err(e) => {
                debug!("Invalid or expired token: {}", e);
                // Token is invalid or expired, continue without authentication
            }
        }
    } else {
        debug!("Missing auth token or username cookie");
    }

    // Continue to the next middleware/handler
    Ok(next.run(request).await)
}

/// Extract user information from username using the Person model
async fn get_user_from_username(username: &str) -> Result<CurrentUser, Error> {
    debug!("Fetching user info for username: {}", username);

    match Person::find_by_username(username).await {
        Ok(Some(person)) => {
            let session_user = person.to_session_user();
            debug!(
                "Found user: {} with email: {} and id: {}",
                session_user.username, session_user.email, session_user.id
            );
            Ok(session_user)
        }
        Ok(None) => {
            error!("No user found with username: {}", username);
            Err(Error::Internal)
        }
        Err(e) => {
            error!("Failed to query user data for {}: {}", username, e);
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
