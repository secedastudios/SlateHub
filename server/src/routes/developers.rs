//! Public-facing developer documentation.

use askama::Template;
use axum::{Router, extract::Request, response::Html, routing::get};
use tracing::error;

use crate::{
    config,
    error::Error,
    middleware::UserExtractor,
    templates::{BaseContext, User},
};

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

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
    let template = DevelopersIndexTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        issuer: config::app_url(),
    };
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
    let template = OidcDocTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        issuer: config::app_url(),
    };
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
    let template = SecurityEventsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        issuer: config::app_url(),
    };
    Ok(Html(template.render().map_err(|e| {
        error!("developers security events template: {}", e);
        Error::template(e.to_string())
    })?))
}
