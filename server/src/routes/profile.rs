use axum::{
    Router,
    extract::{Path, Request},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use tracing::{debug, error, info};

use crate::{error::Error, middleware::UserExtractor, models::person::Person, templates};

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
            error!(username = %username, "Profile user not found");
            return Err(Error::NotFound);
        }
        Err(e) => {
            error!(error = ?e, username = %username, "Failed to fetch profile user");
            return Err(e);
        }
    };

    // Check if profile is public or if it's the user's own profile
    let can_view = if let Some(ref profile) = profile_user.profile {
        profile.is_public || is_own_profile
    } else {
        is_own_profile
    };

    if !can_view {
        info!(username = %username, "Profile is private and not accessible");
        return Err(Error::Forbidden);
    }

    // Prepare template context
    let mut context = templates::base_context();
    context.insert("active_page", "profile");

    // Add current user to context if authenticated
    if let Some(ref user) = current_user {
        context.insert(
            "user",
            &serde_json::json!({
                "id": user.id,
                "name": user.username,
                "email": user.email,
                "avatar": format!("/api/avatar?id={}", user.id)
            }),
        );
    }

    // Add profile user data to context
    let profile_data = serde_json::json!({
        "username": profile_user.username,
        "email": if is_own_profile { Some(&profile_user.email) } else { None },
        "is_own_profile": is_own_profile,
        "profile": if let Some(ref profile) = profile_user.profile {
            serde_json::json!({
                "name": profile.name,
                "headline": profile.headline,
                "bio": profile.bio,
                "location": profile.location,
                "website": profile.website,
                "phone": if is_own_profile { &profile.phone } else { &None },
                "is_public": profile.is_public,

                // Physical attributes (only show if set)
                "physical": if profile.height_mm.is_some() || profile.weight_kg.is_some() ||
                               profile.hair_color.is_some() || profile.eye_color.is_some() {
                    Some(serde_json::json!({
                        "height_mm": profile.height_mm,
                        "weight_kg": profile.weight_kg,
                        "body_type": profile.body_type,
                        "hair_color": profile.hair_color,
                        "eye_color": profile.eye_color,
                        "gender": profile.gender,
                        "ethnicity": profile.ethnicity,
                        "age_range": profile.age_range.as_ref().map(|ar| {
                            serde_json::json!({
                                "min": ar.min,
                                "max": ar.max
                            })
                        })
                    }))
                } else {
                    None
                },

                // Professional details
                "professional": serde_json::json!({
                    "skills": profile.skills,
                    "unions": profile.unions,
                    "languages": profile.languages,
                    "availability": profile.availability,
                    "experience": profile.experience.iter().map(|exp| {
                        serde_json::json!({
                            "role": exp.role,
                            "production": exp.production,
                            "description": exp.description,
                            "dates": exp.dates.as_ref().map(|d| {
                                serde_json::json!({
                                    "start": d.start,
                                    "end": d.end
                                })
                            })
                        })
                    }).collect::<Vec<_>>(),
                    "education": profile.education.iter().map(|edu| {
                        serde_json::json!({
                            "institution": edu.institution,
                            "degree": edu.degree,
                            "field": edu.field,
                            "dates": edu.dates.as_ref().map(|d| {
                                serde_json::json!({
                                    "start": d.start,
                                    "end": d.end
                                })
                            })
                        })
                    }).collect::<Vec<_>>(),
                    "awards": profile.awards.iter().map(|award| {
                        serde_json::json!({
                            "name": award.name,
                            "year": award.year,
                            "description": award.description
                        })
                    }).collect::<Vec<_>>()
                }),

                // Social links
                "social_links": profile.social_links.iter().map(|link| {
                    serde_json::json!({
                        "platform": link.platform,
                        "url": link.url
                    })
                }).collect::<Vec<_>>()
            })
        } else {
            serde_json::json!(null)
        }
    });

    context.insert("profile_user", &profile_data);

    // Render template
    let html = templates::render_with_context("profile.html", &context).map_err(|e| {
        error!(error = ?e, "Failed to render profile template");
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Handler for displaying the profile edit form
async fn edit_profile_form(request: Request) -> Result<Response, Error> {
    debug!("Handling edit profile form request");

    // Check if user is authenticated
    let current_user = match request.get_user() {
        Some(user) => user,
        None => {
            info!("Unauthenticated user trying to edit profile, redirecting to login");
            return Ok(Redirect::to("/login").into_response());
        }
    };

    // Fetch the full user data with profile
    let user = match Person::find_by_username(&current_user.username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!(username = %current_user.username, "Current user not found in database");
            return Err(Error::Internal("User data not found".to_string()));
        }
        Err(e) => {
            error!(error = ?e, username = %current_user.username, "Failed to fetch user data");
            return Err(e);
        }
    };

    // Prepare template context
    let mut context = templates::base_context();
    context.insert("active_page", "profile");

    // Add current user to context
    context.insert(
        "user",
        &serde_json::json!({
            "id": current_user.id,
            "name": current_user.username,
            "email": current_user.email,
            "avatar": format!("/api/avatar?id={}", current_user.id)
        }),
    );

    // Add profile data for editing
    let profile_data = if let Some(ref profile) = user.profile {
        serde_json::json!({
            "name": profile.name,
            "headline": profile.headline,
            "bio": profile.bio,
            "location": profile.location,
            "website": profile.website,
            "phone": profile.phone,
            "is_public": profile.is_public,
            "height_mm": profile.height_mm,
            "weight_kg": profile.weight_kg,
            "body_type": profile.body_type,
            "hair_color": profile.hair_color,
            "eye_color": profile.eye_color,
            "gender": profile.gender,
            "ethnicity": profile.ethnicity,
            "age_range": profile.age_range.as_ref().map(|ar| {
                serde_json::json!({
                    "min": ar.min,
                    "max": ar.max
                })
            }),
            "skills": profile.skills,
            "unions": profile.unions,
            "languages": profile.languages,
            "availability": profile.availability
        })
    } else {
        serde_json::json!({
            "is_public": false
        })
    };

    context.insert("profile", &profile_data);

    // Render template
    let html = templates::render_with_context("profile_edit.html", &context).map_err(|e| {
        error!(error = ?e, "Failed to render profile edit template");
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

/// Handler for updating the user's profile
async fn update_profile(request: Request) -> Result<Response, Error> {
    debug!("Handling update profile request");

    // Check if user is authenticated
    let current_user = match request.get_user() {
        Some(user) => user,
        None => {
            info!("Unauthenticated user trying to update profile, redirecting to login");
            return Ok(Redirect::to("/login").into_response());
        }
    };

    // TODO: Parse form data and update profile in database
    // For now, just redirect back to the profile
    info!(username = %current_user.username, "Profile update not yet implemented");

    Ok(Redirect::to(&format!("/profile/{}", current_user.username)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::person::SessionUser;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };

    #[test]
    fn test_router_creation() {
        // Test that router can be created without panicking
        let router = router();
        assert!(format!("{:?}", router).contains("Router"));
    }

    #[tokio::test]
    async fn test_own_profile_without_auth() {
        // Create a request without authentication
        let request = Request::builder()
            .uri("/profile")
            .body(Body::empty())
            .unwrap();

        let response = own_profile(request).await.unwrap();

        // Should redirect to login
        let response_status = response.into_response().status();
        assert_eq!(response_status, StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn test_own_profile_with_auth() {
        // Create a request with a mock user
        let mut request = Request::builder()
            .uri("/profile")
            .body(Body::empty())
            .unwrap();

        // Add mock user to request extensions
        let mock_user = SessionUser {
            id: "test_id".to_string(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
        };
        request.extensions_mut().insert(mock_user.clone());

        let response = own_profile(request).await.unwrap();

        // Should redirect to user's profile
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok());

        assert_eq!(location, Some("/profile/testuser"));
    }

    #[tokio::test]
    async fn test_edit_profile_form_without_auth() {
        // Create a request without authentication
        let request = Request::builder()
            .uri("/profile/edit")
            .body(Body::empty())
            .unwrap();

        let response = edit_profile_form(request).await.unwrap();

        // Should redirect to login
        let response_status = response.into_response().status();
        assert_eq!(response_status, StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn test_update_profile_without_auth() {
        // Create a request without authentication
        let request = Request::builder()
            .method("POST")
            .uri("/profile/edit")
            .body(Body::empty())
            .unwrap();

        let response = update_profile(request).await.unwrap();

        // Should redirect to login
        let response_status = response.into_response().status();
        assert_eq!(response_status, StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn test_update_profile_with_auth() {
        // Create a request with a mock user
        let mut request = Request::builder()
            .method("POST")
            .uri("/profile/edit")
            .body(Body::empty())
            .unwrap();

        // Add mock user to request extensions
        let mock_user = SessionUser {
            id: "test_id".to_string(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
        };
        request.extensions_mut().insert(mock_user.clone());

        let response = update_profile(request).await.unwrap();

        // Should redirect to user's profile (implementation not complete, but redirect works)
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok());

        assert_eq!(location, Some("/profile/testuser"));
    }

    #[test]
    fn test_profile_data_serialization() {
        use serde_json::json;

        // Test that profile data can be serialized properly for template context
        let profile_data = json!({
            "username": "testuser",
            "email": None::<String>,
            "is_own_profile": false,
            "profile": json!({
                "name": "Test User",
                "headline": "Test Headline",
                "bio": "Test Bio",
                "location": "Test Location",
                "website": "https://example.com",
                "phone": None::<String>,
                "is_public": true,
                "skills": vec!["Skill1", "Skill2"],
                "languages": vec!["English"],
                "unions": Vec::<String>::new(),
                "availability": "available"
            })
        });

        // Verify the JSON structure is valid
        assert!(profile_data["username"].is_string());
        assert!(profile_data["profile"]["is_public"].is_boolean());
        assert!(profile_data["profile"]["skills"].is_array());
    }
}
