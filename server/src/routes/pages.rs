use axum::{Router, response::Html, routing::get};
use tracing::{debug, error};

use crate::{error::Error, templates};

pub fn router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/projects", get(projects))
        .route("/people", get(people))
        .route("/about", get(about))
}

async fn index() -> Result<Html<String>, Error> {
    debug!("Rendering index page");

    let mut context = templates::base_context();
    context.insert("active_page", "home");

    let html = templates::render_with_context("index.html", &context).map_err(|e| {
        error!("Failed to render index template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn projects() -> Result<Html<String>, Error> {
    debug!("Rendering projects page");

    let mut context = templates::base_context();
    context.insert("active_page", "projects");

    let html = templates::render_with_context("projects.html", &context).map_err(|e| {
        error!("Failed to render projects template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn people() -> Result<Html<String>, Error> {
    debug!("Rendering people page");

    let mut context = templates::base_context();
    context.insert("active_page", "people");

    let html = templates::render_with_context("people.html", &context).map_err(|e| {
        error!("Failed to render people template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn about() -> Result<Html<String>, Error> {
    debug!("Rendering about page");

    let mut context = templates::base_context();
    context.insert("active_page", "about");

    let html = templates::render_with_context("about.html", &context).map_err(|e| {
        error!("Failed to render about template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}
