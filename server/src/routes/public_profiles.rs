use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, Request},
    response::Html,
    routing::get,
};
use serde::Deserialize;
use tracing::{debug, error, info};

use crate::{
    db::DB,
    error::Error,
    middleware::UserExtractor,
    models::involvement::InvolvementModel,
    models::person::Person,
    record_id_ext::RecordIdExt,
    social_platforms,
    templates::{
        BaseContext, DateRange, Education, InvolvementDisplay, PeopleTemplate, PersonCard,
        ProfileData, ProfileTemplate, SocialLinkDisplay, User,
    },
};

pub fn router() -> Router {
    Router::new()
        .route("/people", get(people))
        // User profile route - must be last to avoid conflicts with other routes
        .route("/{username}", get(user_profile))
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

/// Convert stored social links to display format with platform metadata
fn to_social_link_displays(links: &[crate::models::person::SocialLink]) -> Vec<SocialLinkDisplay> {
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

/// Handler for viewing a user's public profile at /{username}
/// Uses the same ProfileTemplate as the authenticated profile view
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
    let profile_user = match Person::find_by_username(&username).await? {
        Some(p) => p,
        None => {
            info!("User profile not found for username: {}", username);
            return Err(Error::NotFound);
        }
    };

    // Build base context
    let mut base = BaseContext::new().with_page("profile");
    if let Some(ref user) = current_user {
        base = base.with_user(User::from_session_user(&user).await);
    }

    // Convert Person model to ProfileData (same structure as /profile/{username} used)
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
                    error!("Failed to fetch involvements for {}: {}", username, e);
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
        is_own_profile,
        is_public: profile.map(|p| p.is_public).unwrap_or(false),
        gender: profile.and_then(|p| p.gender.clone()),
        birthday: profile.and_then(|p| p.birthday.clone()),
        height_mm: profile.and_then(|p| p.height_mm),
        weight_kg: profile.and_then(|p| p.weight_kg),
        body_type: profile.and_then(|p| p.body_type.clone()),
        hair_color: profile.and_then(|p| p.hair_color.clone()),
        eye_color: profile.and_then(|p| p.eye_color.clone()),
        ethnicity: profile.map(|p| p.ethnicity.clone()).unwrap_or_default(),
        acting_age_range_min: profile.and_then(|p| p.acting_age_range.as_ref().map(|r| r.min)),
        acting_age_range_max: profile.and_then(|p| p.acting_age_range.as_ref().map(|r| r.max)),
        acting_ethnicities: profile.map(|p| p.acting_ethnicities.clone()).unwrap_or_default(),
        nationality: profile.and_then(|p| p.nationality.clone()),
    };

    // Create and render template using the same ProfileTemplate
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

#[derive(Deserialize)]
struct PeopleQuery {
    filter: Option<String>,
}

async fn people(
    Query(params): Query<PeopleQuery>,
    request: Request,
) -> Result<Html<String>, Error> {
    let filter = params.filter.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());
    debug!("Rendering people page, filter: {:?}", filter);

    let mut base = BaseContext::new().with_page("people");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let mut template = PeopleTemplate::new(base);
    template.filter = filter.map(|s| s.to_string());

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

    // Fetch profiles from the database, optionally filtered
    let persons = if let Some(filter_text) = filter {
        let filter_lower = filter_text.to_lowercase();
        let query = r#"
            SELECT * FROM person
            WHERE (profile.name IS NOT NULL
               OR profile.headline IS NOT NULL
               OR profile.bio IS NOT NULL)
              AND (
                string::lowercase(name ?? '') CONTAINS $filter
                OR string::lowercase(username ?? '') CONTAINS $filter
                OR string::lowercase(profile.name ?? '') CONTAINS $filter
                OR string::lowercase(profile.headline ?? '') CONTAINS $filter
                OR string::lowercase(profile.bio ?? '') CONTAINS $filter
                OR string::lowercase(profile.location ?? '') CONTAINS $filter
                OR $filter IN profile.skills.map(|$v| string::lowercase($v))
                OR $filter IN profile.languages.map(|$v| string::lowercase($v))
              )
            ORDER BY created_at DESC
        "#;
        match DB.query(query).bind(("filter", filter_lower)).await {
            Ok(mut result) => match result.take::<Vec<Person>>(0) {
                Ok(persons) => persons,
                Err(e) => {
                    error!("Failed to fetch filtered persons: {}", e);
                    vec![]
                }
            },
            Err(e) => {
                error!("Failed to query filtered persons: {}", e);
                vec![]
            }
        }
    } else {
        let query = r#"
            SELECT * FROM person
            WHERE profile.name IS NOT NULL
               OR profile.headline IS NOT NULL
               OR profile.bio IS NOT NULL
            ORDER BY created_at DESC
        "#;
        match DB.query(query).await {
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
                        id: person.id.to_raw_string(),
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
