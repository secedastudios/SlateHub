use askama::Template;
use axum::{
    Router,
    extract::Request,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use tracing::error;

use crate::{
    error::Error,
    middleware::UserExtractor,
    templates::{BaseContext, GetVerifiedTemplate, User},
};

pub fn router() -> Router {
    Router::new().route("/get-verified", get(get_verified_page))
}

async fn get_verified_page(request: Request) -> Result<Response, Error> {
    let mut base = BaseContext::new().with_page("get-verified");

    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = GetVerifiedTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render get-verified template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}
