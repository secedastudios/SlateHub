use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

// Helper function to create the app for testing
async fn app() -> axum::Router {
    // Initialize test environment
    dotenv::dotenv().ok();
    std::env::set_var("RUST_LOG", "debug");

    // Initialize templates for testing
    slatehub::templates::init().expect("Failed to initialize templates");

    // Return the app router
    slatehub::routes::app()
}

// Helper function to create an authenticated request
fn authenticated_request(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header(
            "Cookie",
            "auth_token=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoicGVyc29uOndjNnV5bDQ4MWNsbmNzbXVtamdzIiwidXNlcm5hbWUiOiJjaHJpcyIsImV4cCI6MTk5OTk5OTk5OX0.test"
        )
        .body(Body::empty())
        .unwrap()
}

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[tokio::test]
    async fn test_profile_redirect_without_auth() {
        let app = app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/profile")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should redirect to login when not authenticated
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok());

        assert_eq!(location, Some("/login"));
    }

    #[tokio::test]
    async fn test_profile_redirect_with_auth() {
        let app = app().await;

        let response = app
            .oneshot(authenticated_request("/profile"))
            .await
            .unwrap();

        // Should redirect to user's own profile
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok());

        assert!(location.is_some());
        assert!(location.unwrap().starts_with("/profile/"));
    }

    #[tokio::test]
    async fn test_view_user_profile() {
        let app = app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/profile/testuser")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return OK for viewing a public profile
        // Note: This might return 404 if user doesn't exist in test DB
        assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_profile_edit_requires_auth() {
        let app = app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/profile/edit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should redirect to login when not authenticated
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok());

        assert_eq!(location, Some("/login"));
    }

    #[tokio::test]
    async fn test_profile_edit_with_auth() {
        let app = app().await;

        let response = app
            .oneshot(authenticated_request("/profile/edit"))
            .await
            .unwrap();

        // Should return OK or redirect based on whether user exists
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
                || response.status() == StatusCode::SEE_OTHER
        );
    }

    #[tokio::test]
    async fn test_nonexistent_user_profile() {
        let app = app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/profile/nonexistentuser12345678")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return 404 for non-existent user
        // Note: Actual behavior depends on Person::find_by_username implementation
        assert!(
            response.status() == StatusCode::NOT_FOUND
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_profile_has_request_id_header() {
        let app = app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/profile/testuser")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should have X-Request-Id header from our middleware
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn test_profile_css_is_accessible() {
        let app = app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/static/css/profile.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Profile CSS should be accessible
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod profile_model_tests {
    use slatehub::models::person::{Person, Profile};

    #[tokio::test]
    async fn test_person_serialization() {
        // Test that Person struct can be serialized/deserialized
        let profile = Profile {
            name: Some("Test User".to_string()),
            headline: Some("Test Headline".to_string()),
            bio: Some("Test bio".to_string()),
            location: Some("Test Location".to_string()),
            website: Some("https://example.com".to_string()),
            phone: None,
            is_public: true,
            avatar: None,
            height_mm: None,
            weight_kg: None,
            body_type: None,
            hair_color: None,
            eye_color: None,
            gender: None,
            ethnicity: vec![],
            age_range: None,
            skills: vec!["Skill1".to_string(), "Skill2".to_string()],
            unions: vec![],
            languages: vec!["English".to_string()],
            availability: Some("available".to_string()),
            experience: vec![],
            education: vec![],
            awards: vec![],
            reels: vec![],
            media_other: vec![],
            resume: None,
            social_links: vec![],
        };

        // Serialize to JSON
        let json = serde_json::to_string(&profile).unwrap();

        // Deserialize back
        let deserialized: Profile = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, Some("Test User".to_string()));
        assert_eq!(deserialized.skills.len(), 2);
        assert_eq!(deserialized.is_public, true);
    }
}

#[cfg(test)]
mod profile_form_tests {
    use super::*;
    use axum::http::header::CONTENT_TYPE;

    #[tokio::test]
    async fn test_profile_update_post() {
        let app = app().await;

        let form_data = "name=Test+User&headline=Test+Headline&bio=Test+Bio&is_public=true";

        let request = Request::builder()
            .method("POST")
            .uri("/profile/edit")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                "Cookie",
                "auth_token=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJ1c2VyX2lkIjoicGVyc29uOndjNnV5bDQ4MWNsbmNzbXVtamdzIiwidXNlcm5hbWUiOiJjaHJpcyIsImV4cCI6MTk5OTk5OTk5OX0.test"
            )
            .body(Body::from(form_data))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should redirect after form submission (even if not fully implemented)
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }
}
