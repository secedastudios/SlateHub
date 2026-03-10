use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Request},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use std::collections::HashMap;
use tracing::{debug, error, info};

use crate::{
    error::Error,
    middleware::{AuthenticatedUser, UserExtractor},
    models::involvement::InvolvementModel,
    models::person::{Person, SocialLink},
    record_id_ext::RecordIdExt,
    social_platforms::{self, SOCIAL_PLATFORMS},
    templates::{
        BaseContext, DateRange, Education, InvolvementDisplay, ProfileData,
        ProfileEditTemplate, SocialLinkDisplay, SocialPlatformOption, User,
    },
};

pub fn router() -> Router {
    Router::new()
        .route("/profile", get(own_profile))
        .route("/profile/{username}", get(user_profile))
        .route("/profile/edit", get(edit_profile_form).post(update_profile))
}

/// Convert stored social links to display format with platform metadata
fn to_social_link_displays(links: &[SocialLink]) -> Vec<SocialLinkDisplay> {
    links
        .iter()
        .map(|link| {
            let platform = social_platforms::find_platform(&link.platform);
            SocialLinkDisplay {
                platform: link.platform.clone(),
                url: link.url.clone(),
                name: platform.name.to_string(),
                icon_svg: platform.icon_svg.to_string(),
            }
        })
        .collect()
}

/// Build platform options for the edit form dropdown
fn platform_options() -> Vec<SocialPlatformOption> {
    SOCIAL_PLATFORMS
        .iter()
        .map(|p| SocialPlatformOption {
            id: p.id.to_string(),
            name: p.name.to_string(),
            placeholder: p.placeholder.to_string(),
            base_url: p.base_url.map(|s| s.to_string()),
        })
        .collect()
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

    // Redirect to the user's public profile page
    Ok(Redirect::to(&format!("/{}", current_user.username)).into_response())
}

/// Handler for /profile/{username} — redirects to /{username}
async fn user_profile(
    Path(username): Path<String>,
) -> Response {
    Redirect::permanent(&format!("/{}", username)).into_response()
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
        id: profile_user.id.to_raw_string(),
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
        involvements: {
            let pid = profile_user.id.to_raw_string();
            match InvolvementModel::get_for_person(&pid).await {
                Ok(invs) => invs
                    .into_iter()
                    .map(|inv| InvolvementDisplay {
                        involvement_id: inv.id.to_raw_string(),
                        role: inv.role,
                        relation_type: inv.relation_type,
                        department: inv.department,
                        verification_status: inv.verification_status,
                        production_title: inv.production_title,
                        production_slug: inv.production_slug,
                        production_type: inv.production_type,
                        poster_url: inv.poster_url,
                        tmdb_url: inv.tmdb_url,
                        release_date: inv.release_date,
                        media_type: inv.media_type,
                        is_claimed: inv.is_claimed,
                    })
                    .collect(),
                Err(e) => {
                    tracing::error!("Failed to fetch involvements for edit profile: {}", e);
                    vec![]
                }
            }
        },
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
        social_links: to_social_link_displays(
            &profile.map(|p| p.social_links.clone()).unwrap_or_default(),
        ),
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
        platforms: platform_options(),
        error: None,
        success: None,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render profile edit template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

/// Parse social link form fields from the flat form data.
/// Form fields come as `social_links[0][platform]`, `social_links[0][url]`, etc.
fn parse_social_links(form: &HashMap<String, String>) -> Vec<SocialLink> {
    let mut links: HashMap<usize, (Option<String>, Option<String>)> = HashMap::new();

    for (key, value) in form {
        if let Some(rest) = key.strip_prefix("social_links[") {
            if let Some(bracket_pos) = rest.find(']') {
                if let Ok(idx) = rest[..bracket_pos].parse::<usize>() {
                    let field = rest[bracket_pos + 1..]
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    let entry = links.entry(idx).or_insert((None, None));
                    match field {
                        "platform" => entry.0 = Some(value.clone()),
                        "url" => entry.1 = Some(value.clone()),
                        _ => {}
                    }
                }
            }
        }
    }

    let mut sorted: Vec<_> = links.into_iter().collect();
    sorted.sort_by_key(|(idx, _)| *idx);

    sorted
        .into_iter()
        .filter_map(|(_, (platform, url))| {
            let platform = platform?.trim().to_string();
            let url = url?.trim().to_string();
            if platform.is_empty() || url.is_empty() {
                return None;
            }
            let expanded_url = social_platforms::expand_url(&platform, &url);
            if expanded_url.is_empty() {
                return None;
            }
            Some(SocialLink {
                platform,
                url: expanded_url,
            })
        })
        .collect()
}

/// Handler for updating the user's profile
async fn update_profile(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Response, Error> {
    debug!("Handling profile update request");

    let social_links = parse_social_links(&form);

    // Update the profile in the database
    match Person::update_profile(
        &current_user.id,
        form.get("name").cloned(),
        form.get("headline").cloned(),
        form.get("bio").cloned(),
        form.get("location").cloned(),
        form.get("website").cloned(),
        form.get("skills").cloned(),
        form.get("languages").cloned(),
        form.get("availability").cloned(),
        Some(social_links),
    )
    .await
    {
        Ok(Some(_)) => {
            info!(
                "Successfully updated profile for user: {}",
                current_user.username
            );
            Ok(Redirect::to(&format!("/{}", current_user.username)).into_response())
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
