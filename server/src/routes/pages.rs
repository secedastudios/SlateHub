use askama::Template;
use axum::{Router, extract::Request, response::Html, routing::get};
use tracing::{debug, error};

use crate::{
    error::Error,
    middleware::UserExtractor,
    templates::{AboutTemplate, Activity, BaseContext, IndexTemplate, PeopleTemplate, User},
};

pub fn router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/people", get(people))
        .route("/about", get(about))
}

async fn index(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering index page");

    let mut base = BaseContext::new().with_page("home");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    // Create the index template with sample data
    let mut template = IndexTemplate::new(base);

    // Add static stats data (in production, fetch from database)
    template.production_count = 1247;
    template.user_count = 5892;
    template.connection_count = 18453;

    // Add sample activities (in production, fetch from database)
    template.activities = vec![
        Activity {
            user: "Sarah Johnson".to_string(),
            action: "created a new production".to_string(),
            time: "2 minutes ago".to_string(),
        },
        Activity {
            user: "Mike Chen".to_string(),
            action: "joined the platform".to_string(),
            time: "15 minutes ago".to_string(),
        },
        Activity {
            user: "Emily Rodriguez".to_string(),
            action: "posted a job opening".to_string(),
            time: "1 hour ago".to_string(),
        },
        Activity {
            user: "David Kim".to_string(),
            action: "completed a collaboration".to_string(),
            time: "3 hours ago".to_string(),
        },
        Activity {
            user: "Lisa Thompson".to_string(),
            action: "updated their portfolio".to_string(),
            time: "5 hours ago".to_string(),
        },
    ];

    let html = template.render().map_err(|e| {
        error!("Failed to render index template: {}", e);
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

    // In production, you would fetch people from the database here
    // and populate template.people

    let html = template.render().map_err(|e| {
        error!("Failed to render people template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn about(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering about page");

    let mut base = BaseContext::new().with_page("about");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = AboutTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render about template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}
