use askama::Template;
use axum::{Router, extract::Request, response::Html, routing::get};
use tracing::error;

use crate::{
    error::Error,
    middleware::UserExtractor,
    models::analytics::AnalyticsModel,
    models::person::Person,
    templates::{BaseContext, ProfileAnalyticsTemplate, User},
};

pub fn router() -> Router {
    Router::new().route("/profile/analytics", get(analytics_page))
}

async fn analytics_page(request: Request) -> Result<Html<String>, Error> {
    let current_user = match request.get_user() {
        Some(u) => u,
        None => return Err(Error::Unauthorized),
    };

    let mut base = BaseContext::new().with_page("profile");
    base = base.with_user(User::from_session_user(&current_user).await);

    let person = Person::find_by_username(&current_user.username)
        .await?
        .ok_or(Error::NotFound)?;

    let profile_id = &person.id;
    let analytics = AnalyticsModel::get_profile_analytics(profile_id).await?;

    let template = ProfileAnalyticsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        profile_name: person.get_display_name(),
        profile_username: person.username.clone(),
        total_views: analytics.total_views,
        unique_views: analytics.unique_views,
        likes_received: analytics.likes_received,
        views_30d: analytics.views_30d,
        views_90d: analytics.views_90d,
        views_1y: analytics.views_1y,
        referrer_breakdown: analytics.referrer_breakdown,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render analytics template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}
