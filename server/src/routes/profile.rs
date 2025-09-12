use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Request},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use tracing::{debug, error, info};

use crate::{
    error::Error,
    middleware::{AuthenticatedUser, UserExtractor},
    models::person::Person,
    templates::{
        BaseContext, DateRange, Education, Experience, ProfileData, ProfileEditTemplate,
        ProfileTemplate, User,
    },
};

pub fn router() -> Router {
    Router::new()
        .route("/profile", get(own_profile))
        .route("/profile/{username}", get(user_profile))
        .route("/profile/edit", get(edit_profile_form).post(update_profile))
}

/// Handler for viewing the logged-in user's own profile
async fn own_profile(request: Request) -> Result<Response, Error> {
    debug!("Handling own profile request");

    // Check if user is authenticated
    let current_user = match request.get_user() {
        Some(user) => user,
        None => {
            info!("Unauthenticated user trying to access profile, redirecting to login");
            return Ok(Redirect::to("/login").into_response());
        }
    };

    // Redirect to the user's profile page
    Ok(Redirect::to(&format!("/profile/{}", current_user.username)).into_response())
}

/// Handler for viewing a specific user's profile
async fn user_profile(
    Path(username): Path<String>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!(username = %username, "Handling user profile request");

    // Get current user if authenticated
    let current_user = request.get_user();
    let is_own_profile = current_user
        .as_ref()
        .map(|u| u.username == username)
        .unwrap_or(false);

    // Fetch the profile user's data
    let profile_user = match Person::find_by_username(&username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!("Profile not found for username: {}", username);
            return Err(Error::NotFound);
        }
        Err(e) => {
            error!("Failed to fetch user profile: {}", e);
            return Err(e);
        }
    };

    // Build base context
    let mut base = BaseContext::new().with_page("profile");
    if let Some(ref user) = current_user {
        base = base.with_user(User::from_session_user(&user).await);
    }

    // Convert Person model to ProfileData
    let profile = profile_user.profile.as_ref();
    let profile_data = ProfileData {
        id: profile_user.id.to_string(),
        name: profile_user.get_display_name(),
        username: profile_user.username.clone(),
        email: profile_user.email.clone(),
        avatar: profile_user.get_avatar_url(),
        initials: profile_user.get_initials(),
        headline: profile.and_then(|p| p.headline.clone()),
        bio: profile.and_then(|p| p.bio.clone()),
        location: profile.and_then(|p| p.location.clone()),
        website: profile.and_then(|p| p.website.clone()),
        skills: profile.map(|p| p.skills.clone()).unwrap_or_default(),
        languages: profile.map(|p| p.languages.clone()).unwrap_or_default(),
        availability: profile.and_then(|p| p.availability.clone()),
        experience: profile
            .map(|p| p.experience.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|e| Experience {
                role: e.role,
                production: e.production,
                description: e.description,
                dates: e.dates.map(|d| DateRange {
                    start: d.start,
                    end: d.end,
                }),
            })
            .collect(),
        education: profile
            .map(|p| p.education.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|e| Education {
                institution: e.institution,
                degree: e.degree,
                field: e.field,
                dates: e.dates.map(|d| DateRange {
                    start: d.start,
                    end: d.end,
                }),
            })
            .collect(),
        is_own_profile,
        is_public: profile.map(|p| p.is_public).unwrap_or(false),
    };

    // Create and render template
    let template = ProfileTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        profile: profile_data,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render profile template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Handler for displaying the profile edit form
async fn edit_profile_form(request: Request) -> Result<Response, Error> {
    debug!("Handling profile edit form request");

    // Check if user is authenticated
    let current_user = match request.get_user() {
        Some(user) => user,
        None => {
            info!("Unauthenticated user trying to edit profile, redirecting to login");
            return Ok(Redirect::to("/login").into_response());
        }
    };

    // Fetch the user's current profile data
    let profile_user = match Person::find_by_username(&current_user.username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!(
                "Profile not found for authenticated user: {}",
                current_user.username
            );
            return Err(Error::NotFound);
        }
        Err(e) => {
            error!("Failed to fetch user profile for editing: {}", e);
            return Err(e);
        }
    };

    // Build base context
    let base = BaseContext::new()
        .with_page("profile")
        .with_user(User::from_session_user(&current_user).await);

    // Convert Person model to ProfileData
    let profile = profile_user.profile.as_ref();
    let profile_data = ProfileData {
        id: profile_user.id.to_string(),
        name: profile_user.get_display_name(),
        username: profile_user.username.clone(),
        email: profile_user.email.clone(),
        avatar: profile_user.get_avatar_url(),
        initials: profile_user.get_initials(),
        headline: profile.and_then(|p| p.headline.clone()),
        bio: profile.and_then(|p| p.bio.clone()),
        location: profile.and_then(|p| p.location.clone()),
        website: profile.and_then(|p| p.website.clone()),
        skills: profile.map(|p| p.skills.clone()).unwrap_or_default(),
        languages: profile.map(|p| p.languages.clone()).unwrap_or_default(),
        availability: profile.and_then(|p| p.availability.clone()),
        experience: profile
            .map(|p| p.experience.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|e| Experience {
                role: e.role,
                production: e.production,
                description: e.description,
                dates: e.dates.map(|d| DateRange {
                    start: d.start,
                    end: d.end,
                }),
            })
            .collect(),
        education: profile
            .map(|p| p.education.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|e| Education {
                institution: e.institution,
                degree: e.degree,
                field: e.field,
                dates: e.dates.map(|d| DateRange {
                    start: d.start,
                    end: d.end,
                }),
            })
            .collect(),
        is_own_profile: true,
        is_public: profile.map(|p| p.is_public).unwrap_or(false),
    };

    // Create and render template
    let template = ProfileEditTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        profile: profile_data,
        error: None,
        success: None,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render profile edit template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

/// Handler for updating the user's profile
async fn update_profile(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Form(form): Form<UpdateProfileForm>,
) -> Result<Response, Error> {
    debug!("Handling profile update request");

    // Update the profile in the database
    match Person::update_profile(
        &current_user.id,
        form.name,
        form.headline,
        form.bio,
        form.location,
        form.website,
        form.skills,
        form.languages,
        form.availability,
    )
    .await
    {
        Ok(Some(_)) => {
            info!(
                "Successfully updated profile for user: {}",
                current_user.username
            );
            Ok(Redirect::to(&format!("/profile/{}", current_user.username)).into_response())
        }
        Ok(None) => {
            error!("Profile not found for user: {}", current_user.username);
            Err(Error::NotFound)
        }
        Err(e) => {
            error!(
                "Failed to update profile for user {}: {}",
                current_user.username, e
            );
            Err(e)
        }
    }
}

/// Form data for updating a user profile
#[derive(Debug, serde::Deserialize)]
pub struct UpdateProfileForm {
    pub name: Option<String>,
    pub headline: Option<String>,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub skills: Option<String>,    // Comma-separated list
    pub languages: Option<String>, // Comma-separated list
    pub availability: Option<String>,
}
