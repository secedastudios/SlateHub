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
    models::person::{Person, Photo, Reel, SocialLink},
    record_id_ext::RecordIdExt,
    social_platforms::{self, SOCIAL_PLATFORMS},
    templates::{
        BaseContext, DateRange, Education, InvolvementDisplay, PhotoDisplay, ProfileData,
        ProfileEditTemplate, ReelDisplay, SocialLinkDisplay, SocialPlatformOption, User,
    },
    verification_limits,
    video_platforms,
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

/// Convert stored photos to display format
fn to_photo_displays(photos: &[Photo]) -> Vec<PhotoDisplay> {
    photos
        .iter()
        .map(|photo| PhotoDisplay {
            url: photo.url.clone(),
            thumbnail_url: photo.thumbnail_url.clone(),
            caption: photo.caption.clone(),
        })
        .collect()
}

/// Convert stored reels to display format with computed URLs
fn to_reel_displays(reels: &[Reel]) -> Vec<ReelDisplay> {
    reels
        .iter()
        .map(|reel| ReelDisplay {
            url: reel.url.clone(),
            title: reel.title.clone(),
            platform: reel.platform.clone(),
            video_id: reel.video_id.clone(),
            thumbnail_url: video_platforms::thumbnail_url(&reel.platform, &reel.video_id),
            embed_url: video_platforms::embed_url(&reel.platform, &reel.video_id),
            platform_name: video_platforms::platform_name(&reel.platform).to_string(),
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
        reels: to_reel_displays(
            &profile.map(|p| p.reels.clone()).unwrap_or_default(),
        ),
        photos: to_photo_displays(
            &profile.map(|p| p.photos.clone()).unwrap_or_default(),
        ),
        is_own_profile: true,
        is_public: profile.map(|p| p.is_public).unwrap_or(false),
        verification_status: profile_user.verification_status.clone(),
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

    // Compute upload limits based on verification status
    let limits = verification_limits::limits_for_status(&profile_user.verification_status);
    let is_verified = verification_limits::is_identity_verified(&profile_user.verification_status);

    // Create and render template
    let template = ProfileEditTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        photo_count: profile_data.photos.len(),
        reel_count: profile_data.reels.len(),
        photo_limit: limits.max_photos,
        reel_limit: limits.max_reels,
        is_identity_verified: is_verified,
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

/// Parse height from form fields, converting to mm.
/// Supports metric (cm) and imperial (ft + in).
fn parse_height_mm(form: &HashMap<String, String>) -> Option<i32> {
    let unit = form.get("height_unit").map(|s| s.as_str()).unwrap_or("metric");
    match unit {
        "imperial" => {
            let feet: i32 = form.get("height_feet").and_then(|v| v.parse().ok()).unwrap_or(0);
            let inches: i32 = form.get("height_inches").and_then(|v| v.parse().ok()).unwrap_or(0);
            let total_inches = feet * 12 + inches;
            if total_inches > 0 {
                Some((total_inches as f64 * 25.4).round() as i32)
            } else {
                Some(0)
            }
        }
        _ => {
            let cm: i32 = form.get("height_cm").and_then(|v| v.parse().ok()).unwrap_or(0);
            if cm > 0 { Some(cm * 10) } else { Some(0) }
        }
    }
}

/// Parse weight from form fields, converting to kg.
/// Supports metric (kg) and imperial (lbs).
fn parse_weight_kg(form: &HashMap<String, String>) -> Option<i32> {
    let unit = form.get("weight_unit").map(|s| s.as_str()).unwrap_or("metric");
    match unit {
        "imperial" => {
            let lbs: f64 = form.get("weight_lbs").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            if lbs > 0.0 {
                Some((lbs * 0.453592).round() as i32)
            } else {
                Some(0)
            }
        }
        _ => {
            let kg: i32 = form.get("weight_kg").and_then(|v| v.parse().ok()).unwrap_or(0);
            Some(kg)
        }
    }
}

/// Parse reel form fields from the flat form data.
/// Form fields come as `reels[0][url]`, `reels[0][title]`, etc.
/// When a title is empty, fetches the video title from the platform's oEmbed API.
async fn parse_reels(form: &HashMap<String, String>) -> Vec<Reel> {
    let mut reels: HashMap<usize, (Option<String>, Option<String>)> = HashMap::new();

    for (key, value) in form {
        if let Some(rest) = key.strip_prefix("reels[") {
            if let Some(bracket_pos) = rest.find(']') {
                if let Ok(idx) = rest[..bracket_pos].parse::<usize>() {
                    let field = rest[bracket_pos + 1..]
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    let entry = reels.entry(idx).or_insert((None, None));
                    match field {
                        "url" => entry.0 = Some(value.clone()),
                        "title" => entry.1 = Some(value.clone()),
                        _ => {}
                    }
                }
            }
        }
    }

    let mut sorted: Vec<_> = reels.into_iter().collect();
    sorted.sort_by_key(|(idx, _)| *idx);

    let mut result = Vec::new();
    for (_, (url, title)) in sorted {
        let url = match url {
            Some(u) => u.trim().to_string(),
            None => continue,
        };
        let title = title.unwrap_or_default().trim().to_string();
        if url.is_empty() {
            continue;
        }
        let info = match video_platforms::parse_video_url(&url) {
            Some(i) => i,
            None => continue,
        };
        let title = if title.is_empty() {
            video_platforms::fetch_video_title(info.platform, &url)
                .await
                .unwrap_or_else(|| {
                    format!("{} Video", video_platforms::platform_name(info.platform))
                })
        } else {
            title
        };
        result.push(Reel {
            url,
            title,
            platform: info.platform.to_string(),
            video_id: info.video_id,
        });
    }
    result
}

/// Parse photo form fields from the flat form data.
/// Form fields come as `photos[0][url]`, `photos[0][thumbnail_url]`, `photos[0][caption]`.
fn parse_photos(form: &HashMap<String, String>) -> Vec<Photo> {
    let mut photos: HashMap<usize, (Option<String>, Option<String>, Option<String>)> =
        HashMap::new();

    for (key, value) in form {
        if let Some(rest) = key.strip_prefix("photos[") {
            if let Some(bracket_pos) = rest.find(']') {
                if let Ok(idx) = rest[..bracket_pos].parse::<usize>() {
                    let field = rest[bracket_pos + 1..]
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    let entry = photos.entry(idx).or_insert((None, None, None));
                    match field {
                        "url" => entry.0 = Some(value.clone()),
                        "thumbnail_url" => entry.1 = Some(value.clone()),
                        "caption" => entry.2 = Some(value.clone()),
                        _ => {}
                    }
                }
            }
        }
    }

    let mut sorted: Vec<_> = photos.into_iter().collect();
    sorted.sort_by_key(|(idx, _)| *idx);

    sorted
        .into_iter()
        .filter_map(|(_, (url, thumbnail_url, caption))| {
            let url = url?.trim().to_string();
            let thumbnail_url = thumbnail_url.unwrap_or_default().trim().to_string();
            if url.is_empty() {
                return None;
            }
            Some(Photo {
                url,
                thumbnail_url,
                caption: caption.unwrap_or_default().trim().to_string(),
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
    let reels = parse_reels(&form).await;
    let photos = parse_photos(&form);

    // Enforce verification-based limits on reels and photos
    let person = Person::find_by_id(&current_user.id).await?.ok_or(Error::NotFound)?;
    let limits = verification_limits::limits_for_status(&person.verification_status);
    if let Some(max) = limits.max_reels {
        if reels.len() > max {
            return Err(Error::bad_request(format!("Maximum of {} reels allowed. Get verified to remove this limit.", max)));
        }
    }
    if let Some(max) = limits.max_photos {
        if photos.len() > max {
            return Err(Error::bad_request(format!("Maximum of {} photos allowed. Get verified for more uploads.", max)));
        }
    }

    // Parse physical attribute fields
    let height_mm = parse_height_mm(&form);
    let weight_kg = parse_weight_kg(&form);
    let acting_age_min: Option<i32> = form.get("acting_age_range_min").and_then(|v| v.parse().ok());
    let acting_age_max: Option<i32> = form.get("acting_age_range_max").and_then(|v| v.parse().ok());

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
        Some(reels),
        Some(photos),
        form.get("gender").cloned(),
        form.get("birthday").cloned(),
        height_mm,
        weight_kg,
        form.get("body_type").cloned(),
        form.get("hair_color").cloned(),
        form.get("eye_color").cloned(),
        form.get("ethnicity").cloned(),
        acting_age_min,
        acting_age_max,
        form.get("acting_ethnicities").cloned(),
        form.get("nationality").cloned(),
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
