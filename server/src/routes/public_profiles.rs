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
    db::DB,
    error::Error,
    middleware::UserExtractor,
    models::person::Person,
    templates::{BaseContext, PeopleTemplate, PersonCard, User},
};

pub fn router() -> Router {
    Router::new()
        .route("/people", get(people))
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
        base = base.with_user(User::from_session_user(&user).await);
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

async fn people(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering people page");

    let mut base = BaseContext::new().with_page("people");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let mut template = PeopleTemplate::new(base);

    // Add specialties list (in production, fetch from database)
    template.specialties = vec![
        "Director".to_string(),
        "Producer".to_string(),
        "Cinematographer".to_string(),
        "Editor".to_string(),
        "Sound Designer".to_string(),
        "Actor".to_string(),
        "Writer".to_string(),
        "Composer".to_string(),
    ];

    // Fetch all profiles from the database ordered by created_at descending
    // Note: Since Person model doesn't have a created_at field exposed, we'll use the database query directly
    let query = r#"
        SELECT * FROM person
        WHERE profile.name IS NOT NULL
           OR profile.headline IS NOT NULL
           OR profile.bio IS NOT NULL
        ORDER BY created_at DESC
    "#;

    let persons = match DB.query(query).await {
        Ok(mut result) => match result.take::<Vec<Person>>(0) {
            Ok(persons) => persons,
            Err(e) => {
                error!("Failed to fetch persons from database: {}", e);
                vec![]
            }
        },
        Err(e) => {
            error!("Failed to query persons from database: {}", e);
            vec![]
        }
    };

    // Convert Person objects to PersonCard for the template
    template.people = persons
        .into_iter()
        .filter_map(|person| {
            // Only include profiles that have at least a name
            if let Some(profile) = person.profile {
                if profile.name.is_some() || profile.headline.is_some() || profile.bio.is_some() {
                    Some(PersonCard {
                        id: person.id.to_string(),
                        name: profile
                            .name
                            .clone()
                            .unwrap_or_else(|| person.username.clone()),
                        username: person.username.clone(),
                        headline: profile.headline.clone(),
                        bio: profile.bio.clone(),
                        location: profile.location.clone(),
                        skills: profile.skills,
                        avatar: profile
                            .avatar
                            .clone()
                            .unwrap_or_else(|| format!("/static/images/default-avatar.png")),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    let html = template.render().map_err(|e| {
        error!("Failed to render people template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}
