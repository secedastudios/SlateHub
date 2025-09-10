use askama::Template;
use axum::{
    Form, Router,
    extract::Request,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use std::env;

use tracing::{debug, error, info};

use crate::{
    error::Error,
    middleware::UserExtractor,
    models::person::{CreateUser, LoginUser, Person},
    response,
    templates::{BaseContext, LoginTemplate, SignupTemplate, User},
};

pub fn router() -> Router {
    Router::new()
        .route("/signup", get(signup_form).post(signup))
        .route("/login", get(login_form).post(login))
        .route("/logout", post(logout))
}

async fn signup_form(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering signup page");

    let mut base = BaseContext::new().with_page("signup");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User {
            id: user.id.clone(),
            name: user.username.clone(),
            email: user.email.clone(),
            avatar: format!("/api/avatar?id={}", user.id),
        });
    }

    let template = SignupTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render signup template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[axum::debug_handler]
async fn signup(Form(form): Form<CreateUser>) -> Result<Response, Error> {
    debug!("Processing signup for email: {}", form.email);

    // Try to create the user
    match Person::signup(form.username, form.email, form.password).await {
        Ok(token) => {
            info!("User created successfully");

            // Create authentication cookie with the JWT token
            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .same_site(SameSite::Lax)
                .http_only(true)
                .secure(env::var("COOKIE_SECURE").unwrap_or_default() == "true")
                .build();

            // Redirect to profile or dashboard
            Ok((CookieJar::new().add(cookie), response::redirect("/profile")).into_response())
        }
        Err(e) => {
            error!("Signup failed: {}", e);

            // Re-render the signup form with error
            let base = BaseContext::new().with_page("signup");

            let mut template = SignupTemplate::new(base);
            template.error = Some(e.to_string());

            let html = template.render().map_err(|e| {
                error!("Failed to render signup template with error: {}", e);
                Error::template(e.to_string())
            })?;

            Ok(Html(html).into_response())
        }
    }
}

async fn login_form(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering login page");

    let mut base = BaseContext::new().with_page("login");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User {
            id: user.id.clone(),
            name: user.username.clone(),
            email: user.email.clone(),
            avatar: format!("/api/avatar?id={}", user.id),
        });
    }

    let template = LoginTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render login template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[axum::debug_handler]
async fn login(Form(form): Form<LoginUser>) -> Result<Response, Error> {
    debug!("Processing login for: {}", form.email);

    // Try to authenticate the user (signin accepts username or email as identifier)
    match Person::signin(form.email.clone(), form.password).await {
        Ok(token) => {
            info!("User logged in successfully");

            // Create authentication cookie with the JWT token
            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .same_site(SameSite::Lax)
                .http_only(true)
                .secure(env::var("COOKIE_SECURE").unwrap_or_default() == "true")
                .build();

            // Redirect to profile or the originally requested page
            let redirect_to = form.redirect_to.unwrap_or_else(|| "/profile".to_string());

            Ok((
                CookieJar::new().add(cookie),
                response::redirect(&redirect_to),
            )
                .into_response())
        }
        Err(e) => {
            error!("Login failed for {}: {}", form.email, e);

            // Re-render the login form with error
            let base = BaseContext::new().with_page("login");

            let mut template = LoginTemplate::new(base);
            template.error = Some("Invalid email or password".to_string());
            template.redirect_to = form.redirect_to;

            let html = template.render().map_err(|e| {
                error!("Failed to render login template with error: {}", e);
                Error::template(e.to_string())
            })?;

            Ok(Html(html).into_response())
        }
    }
}

#[axum::debug_handler]
async fn logout() -> Response {
    debug!("Processing logout");

    // Create a cookie that expires immediately to clear the auth
    let cookie = Cookie::build(("auth_token", ""))
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(env::var("COOKIE_SECURE").unwrap_or_default() == "true")
        .max_age(Default::default())
        .build();

    (CookieJar::new().remove(cookie), response::redirect("/")).into_response()
}
