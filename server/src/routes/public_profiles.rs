use askama::Template;
use axum::{
    Router,
    extract::{Path, Request},
    response::Html,
    routing::get,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::{
    error::Error,
    middleware::UserExtractor,
    models::person::Person,
    templates::{BaseContext, User},
};

pub fn router() -> Router {
    Router::new()
        // User profile route - must be last to avoid conflicts with other routes
        .route("/{username}", get(user_profile))
}

/// Organization summary for displaying user's organizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSummary {
    pub org_name: String,
    pub org_slug: String,
    pub org_type: String,
    pub role: String,
}

/// Template structure for public profile page
#[derive(Template)]
#[template(path = "persons/public_profile.html")]
pub struct PublicProfileTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub person: Person,
    pub is_own_profile: bool,
    pub organizations: Vec<OrganizationSummary>,
}

/// List of reserved routes that should not be treated as usernames
const RESERVED_ROUTES: &[&str] = &[
    "about",
    "admin",
    "api",
    "auth",
    "contact",
    "dashboard",
    "help",
    "home",
    "login",
    "logout",
    "org",
    "orgs",
    "people",
    "profile",
    "project",
    "projects",
    "search",
    "settings",
    "signup",
    "static",
    "support",
    "terms",
    "privacy",
];

/// Handler for viewing a user's public profile
async fn user_profile(
    Path(username): Path<String>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Attempting to view public profile: {}", username);

    // Check if this is a reserved route
    if RESERVED_ROUTES.contains(&username.as_str()) {
        debug!("Username {} is a reserved route", username);
        return Err(Error::NotFound);
    }

    // Get current user if authenticated
    let current_user = request.get_user();
    let is_own_profile = current_user
        .as_ref()
        .map(|u| u.username == username)
        .unwrap_or(false);

    // Fetch the user's profile data using the Person model
    let person = match Person::find_by_username(&username).await? {
        Some(p) => p,
        None => {
            info!("User profile not found for username: {}", username);
            return Err(Error::NotFound);
        }
    };

    // TODO: Fix organization query once we understand the SurrealDB syntax better
    // The query should fetch organization memberships for the user
    // For now, just use an empty list to bypass the organization query issue
    let organizations: Vec<OrganizationSummary> = Vec::new();

    // TODO: Fix organization query once we understand the SurrealDB syntax better
    // The query should fetch organization memberships for the user

    // Build base context
    let mut base = BaseContext::new().with_page("public-profile");
    if let Some(ref user) = current_user {
        base = base.with_user(User {
            id: user.id.clone(),
            name: user.username.clone(),
            email: user.email.clone(),
            avatar: format!("/api/avatar?id={}", user.id),
        });
    }

    // Create and render template
    let template = PublicProfileTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        person,
        is_own_profile,
        organizations,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render public profile template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Check if a username is available (not taken or reserved)
pub async fn check_username_availability(username: &str) -> Result<bool, Error> {
    // Check if it's a reserved route
    if RESERVED_ROUTES.contains(&username) {
        return Ok(false);
    }

    // Check if username already exists using the Person model
    match Person::find_by_username(username).await? {
        Some(_) => Ok(false),
        None => Ok(true),
    }
}

/// Get the public URL for a user profile
pub fn get_user_profile_url(username: &str) -> String {
    format!("/{}", username)
}

/// Get the public URL for an organization profile
pub fn get_organization_profile_url(slug: &str) -> String {
    format!("/org/{}", slug)
}
