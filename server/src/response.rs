//! Response helpers for common HTTP responses
//!
//! This module provides utility functions for creating consistent HTTP responses
//! throughout the application, including redirects, HTML responses, and JSON responses.

use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tracing::debug;

/// Create a redirect response to the specified path
///
/// This creates a 303 See Other redirect, which is the recommended status code
/// for redirecting after a successful POST request (Post-Redirect-Get pattern).
///
/// # Example
/// ```
/// return Ok(redirect("/"));
/// ```
pub fn redirect(path: &str) -> Response {
    redirect_with_status(path, StatusCode::SEE_OTHER)
}

/// Create a temporary redirect (302 Found)
///
/// Use this for redirects that might change in the future.
///
/// # Example
/// ```
/// return Ok(redirect_temporary("/login"));
/// ```
pub fn redirect_temporary(path: &str) -> Response {
    redirect_with_status(path, StatusCode::FOUND)
}

/// Create a permanent redirect (301 Moved Permanently)
///
/// Use this for redirects that will never change (e.g., old URLs to new URLs).
///
/// # Example
/// ```
/// return Ok(redirect_permanent("/new-path"));
/// ```
pub fn redirect_permanent(path: &str) -> Response {
    redirect_with_status(path, StatusCode::MOVED_PERMANENTLY)
}

/// Create a redirect with a specific status code
///
/// # Example
/// ```
/// return Ok(redirect_with_status("/", StatusCode::TEMPORARY_REDIRECT));
/// ```
pub fn redirect_with_status(path: &str, status: StatusCode) -> Response {
    // Ensure the path starts with / for absolute path
    let absolute_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };

    debug!(
        "Creating redirect response to '{}' with status {}",
        absolute_path, status
    );

    let response = (
        status,
        [(
            header::LOCATION,
            HeaderValue::from_str(&absolute_path).unwrap_or_else(|_| {
                debug!(
                    "Failed to create HeaderValue from path '{}', using '/' as fallback",
                    absolute_path
                );
                HeaderValue::from_static("/")
            }),
        )],
        // Include empty body for redirect
        "",
    )
        .into_response();

    debug!(
        "Redirect response created: status={:?}, location={:?}",
        response.status(),
        response.headers().get(header::LOCATION)
    );

    response
}

/// Create a redirect with cookies
///
/// This is useful when you need to set or remove cookies while redirecting.
///
/// # Example
/// ```
/// use axum_extra::extract::cookie::{Cookie, CookieJar};
///
/// let cookie = Cookie::build(("session", "value")).path("/").build();
/// return Ok(redirect_with_cookies("/", jar.add(cookie)));
/// ```
pub fn redirect_with_cookies(path: &str, jar: axum_extra::extract::CookieJar) -> Response {
    debug!("Creating redirect with cookies to '{}'", path);
    let redirect_response = redirect(path);
    let response = (jar, redirect_response).into_response();
    debug!(
        "Redirect with cookies created: status={:?}, has set-cookie={:?}",
        response.status(),
        response.headers().get(header::SET_COOKIE).is_some()
    );
    response
}

/// Create an HTML response
///
/// # Example
/// ```
/// return Ok(html("<h1>Hello World</h1>"));
/// ```
pub fn html(content: impl Into<String>) -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        content.into(),
    )
        .into_response()
}

/// Create a JSON response
///
/// # Example
/// ```
/// #[derive(Serialize)]
/// struct Data { message: String }
/// return Ok(json(&Data { message: "Success".into() }));
/// ```
pub fn json<T: Serialize>(data: &T) -> Response {
    match serde_json::to_string(data) {
        Ok(json_string) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            json_string,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to serialize JSON: {}", e),
        )
            .into_response(),
    }
}

/// Create a no content (204) response
///
/// Use this for successful operations that don't return data.
///
/// # Example
/// ```
/// return Ok(no_content());
/// ```
pub fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Create a not found (404) response
///
/// # Example
/// ```
/// return Ok(not_found());
/// ```
pub fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not Found").into_response()
}

/// Create an unauthorized (401) response
///
/// # Example
/// ```
/// return Ok(unauthorized());
/// ```
pub fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
}

/// Create a forbidden (403) response
///
/// # Example
/// ```
/// return Ok(forbidden());
/// ```
pub fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "Forbidden").into_response()
}

/// Create a bad request (400) response with a message
///
/// # Example
/// ```
/// return Ok(bad_request("Invalid input"));
/// ```
pub fn bad_request(message: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, message.into()).into_response()
}

/// Create an internal server error (500) response
///
/// # Example
/// ```
/// return Ok(internal_error());
/// ```
pub fn internal_error() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header;

    #[test]
    fn test_redirect() {
        let response = redirect("/home");
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/home");
    }

    #[test]
    fn test_redirect_adds_leading_slash() {
        let response = redirect("home");
        assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/home");
    }

    #[test]
    fn test_redirect_temporary() {
        let response = redirect_temporary("/login");
        assert_eq!(response.status(), StatusCode::FOUND);
    }

    #[test]
    fn test_redirect_permanent() {
        let response = redirect_permanent("/new-url");
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    }
}
