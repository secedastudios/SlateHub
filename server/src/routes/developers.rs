//! Public-facing developer documentation pages under `/developers`: an
//! index plus OIDC and security-events (SSF/CAEP) guides. Pages render for
//! anonymous visitors too; each template receives the issuer URL so code
//! samples show the deployment's real endpoints.

use askama::Template;
use axum::{Router, extract::Request, response::Html, routing::get};
use tracing::error;

use crate::{
    config,
    error::Error,
    middleware::UserExtractor,
    templates::{BaseContext, User},
};

// Shared Askama filters (abs_url, …) for the in-file Template derives.
use crate::templates::filters;

/// Routes for the `/developers` documentation index and topic pages.
pub fn router() -> Router {
    Router::new()
        .route("/developers", get(index))
        .route("/developers/oidc", get(oidc_doc))
        .route("/developers/security-events", get(security_events_doc))
}

#[derive(Template)]
#[template(path = "developers/index.html")]
struct DevelopersIndexTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    issuer: String,
}

#[derive(Template)]
#[template(path = "developers/oidc.html")]
struct OidcDocTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    issuer: String,
}

#[derive(Template)]
#[template(path = "developers/security_events.html")]
struct SecurityEventsTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    issuer: String,
}

async fn index(request: Request) -> Result<Html<String>, Error> {
    let mut base = BaseContext::new().with_page("developers");
    if let Some(u) = request.get_user() {
        base = base.with_user(User::from_session_user(&u).await);
    }
    let template = crate::with_base!(DevelopersIndexTemplate, base, {
        issuer: config::app_url(),
    });
    Ok(Html(template.render().map_err(|e| {
        error!("developers index template: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn oidc_doc(request: Request) -> Result<Html<String>, Error> {
    let mut base = BaseContext::new().with_page("developers");
    if let Some(u) = request.get_user() {
        base = base.with_user(User::from_session_user(&u).await);
    }
    let template = crate::with_base!(OidcDocTemplate, base, {
        issuer: config::app_url(),
    });
    Ok(Html(template.render().map_err(|e| {
        error!("developers oidc template: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn security_events_doc(request: Request) -> Result<Html<String>, Error> {
    let mut base = BaseContext::new().with_page("developers");
    if let Some(u) = request.get_user() {
        base = base.with_user(User::from_session_user(&u).await);
    }
    let template = crate::with_base!(SecurityEventsTemplate, base, {
        issuer: config::app_url(),
    });
    Ok(Html(template.render().map_err(|e| {
        error!("developers security events template: {}", e);
        Error::template(e.to_string())
    })?))
}
