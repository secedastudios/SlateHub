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
    response, templates,
};

pub fn router() -> Router {
    Router::new()
        .route("/signup", get(signup_form).post(signup))
        .route("/login", get(login_form).post(login))
        .route("/logout", post(logout))
}

async fn signup_form(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering signup page");

    let mut context = templates::base_context();
    context.insert("active_page", "signup");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
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

    let html = templates::render_with_context("signup.html", &context).map_err(|e| {
        error!("Failed to render signup template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[axum::debug_handler]
async fn signup(Form(input): Form<CreateUser>) -> Result<Response, Error> {
    debug!("Attempting to sign up new user: {}", input.username);

    // --- Basic Validation ---
    if input.username.is_empty() || input.email.is_empty() {
        return render_signup_with_error(
            "Username and email are required.",
            Some(&input.username),
            Some(&input.email),
        )
        .await;
    }
    if input.password.len() < 8 {
        return render_signup_with_error(
            "Password must be at least 8 characters long.",
            Some(&input.username),
            Some(&input.email),
        )
        .await;
    }

    // --- Database Signup ---
    let result = Person::signup(
        input.username.clone(),
        input.email.clone(),
        input.password.clone(),
    )
    .await;

    match result {
        Ok(_token) => {
            info!("Successfully signed up user: {}", input.username);
            // Render success page that will redirect to login
            let mut context = templates::base_context();
            context.insert("active_page", "signup");
            context.insert(
                "success_message",
                "Account created successfully! Redirecting to login...",
            );
            context.insert("redirect_to", "/login");

            let html = templates::render_with_context("signup.html", &context).map_err(|e| {
                error!("Failed to render signup template: {}", e);
                Error::template(e.to_string())
            })?;

            Ok(Html(html).into_response())
        }
        Err(e) => {
            error!("Failed to sign up user '{}': {}", input.username, e);
            let error_msg = if e.to_string().contains("already exists") {
                "Username or email is already in use."
            } else {
                "An unexpected error occurred. Please try again."
            };

            render_signup_with_error(error_msg, Some(&input.username), Some(&input.email)).await
        }
    }
}

async fn render_signup_with_error(
    error_message: &str,
    username: Option<&str>,
    email: Option<&str>,
) -> Result<Response, Error> {
    let mut context = templates::base_context();
    context.insert("active_page", "signup");
    context.insert("error_message", error_message);

    if let Some(u) = username {
        context.insert("username_value", u);
    }
    if let Some(e) = email {
        context.insert("email_value", e);
    }

    let html = templates::render_with_context("signup.html", &context).map_err(|e| {
        error!("Failed to render signup template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

async fn login_form(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering login page");

    let mut context = templates::base_context();
    context.insert("active_page", "login");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
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

    let html = templates::render_with_context("login.html", &context).map_err(|e| {
        error!("Failed to render login template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[axum::debug_handler]
async fn login(jar: CookieJar, Form(input): Form<LoginUser>) -> Result<Response, Error> {
    debug!("Attempting to log in user: {}", &input.user);

    if input.user.is_empty() || input.pass.is_empty() {
        return render_login_with_error(
            "Username/email and password are required.",
            Some(&input.user),
        )
        .await;
    }

    // Use the Person model's signin method
    let result = Person::signin(input.user.clone(), input.pass.clone()).await;

    match result {
        Ok(token_str) => {
            info!("Successfully authenticated user: {}", &input.user);

            // Debug: Log the JWT token
            debug!("JWT token string: {}", &token_str);
            debug!("Login successful, preparing to redirect to home page '/'");

            // In development, don't require HTTPS for cookies
            let is_production =
                env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "production";

            // Create auth token cookie
            let auth_cookie = Cookie::build(("auth_token", token_str))
                .http_only(true)
                .secure(is_production) // Only require HTTPS in production
                .same_site(SameSite::Strict)
                .path("/")
                .build();

            debug!("Auth cookie created, redirecting to home page '/'");
            // Add auth cookie to response and redirect
            let redirect_response = response::redirect_with_cookies("/", jar.add(auth_cookie));
            debug!(
                "Redirect response created with status: {:?}",
                redirect_response.status()
            );
            Ok(redirect_response)
        }
        Err(e) => {
            let error_msg =
                if e.to_string().contains("Authentication") || e.to_string().contains("Invalid") {
                    error!("Invalid credentials for user '{}'", &input.user);
                    "Invalid username/email or password."
                } else {
                    error!("Authentication failed for user '{}': {}", &input.user, e);
                    "An error occurred during login. Please try again."
                };

            render_login_with_error(error_msg, Some(&input.user)).await
        }
    }
}

async fn render_login_with_error(
    error_message: &str,
    username: Option<&str>,
) -> Result<Response, Error> {
    let mut context = templates::base_context();
    context.insert("active_page", "login");
    context.insert("error_message", error_message);

    if let Some(u) = username {
        context.insert("user_value", u);
    }

    let html = templates::render_with_context("login.html", &context).map_err(|e| {
        error!("Failed to render login template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

#[axum::debug_handler]
async fn logout(jar: CookieJar) -> Result<Response, Error> {
    debug!("User logging out");

    // We don't invalidate the DB session since we're using a singleton root connection
    // Just remove the auth cookie - the JWT token will expire on its own
    info!("User logging out - removing auth cookie");

    // Remove auth cookie
    let auth_cookie = Cookie::build("auth_token").path("/").build();

    // Clear cookie and redirect to home page
    Ok(response::redirect_with_cookies(
        "/",
        jar.remove(auth_cookie),
    ))
}
