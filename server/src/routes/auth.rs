use axum::{
    Form, Router,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};

use std::{collections::HashMap, env};
use surrealdb::opt::auth::Record;
use tracing::{debug, error, info};

use crate::{
    db::DB,
    error::Error,
    models::person::{CreateUser, LoginUser},
    templates,
};

pub fn router() -> Router {
    Router::new()
        .route("/signup", get(signup_form).post(signup))
        .route("/login", get(login_form).post(login))
        .route("/logout", post(logout))
}

async fn signup_form() -> Result<Html<String>, Error> {
    debug!("Rendering signup page");

    let mut context = templates::base_context();
    context.insert("active_page", "signup");

    let html = templates::render_with_context("signup.html", &context).map_err(|e| {
        error!("Failed to render signup template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

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
    // Get namespace and database from environment, using same defaults as config
    let namespace = env::var("DB_NAMESPACE").unwrap_or_else(|_| "slatehub".to_string());
    let database = env::var("DB_NAME").unwrap_or_else(|_| "main".to_string());

    let mut vars = HashMap::new();
    vars.insert("user".to_string(), input.username.clone());
    vars.insert("email".to_string(), input.email.clone());
    vars.insert("pass".to_string(), input.password.clone());

    let result = DB
        .signup(Record {
            namespace: &namespace,
            database: &database,
            access: "user",
            params: vars,
        })
        .await;

    match result {
        Ok(_) => {
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

async fn login_form() -> Result<Html<String>, Error> {
    debug!("Rendering login page");

    let mut context = templates::base_context();
    context.insert("active_page", "login");

    let html = templates::render_with_context("login.html", &context).map_err(|e| {
        error!("Failed to render login template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn login(Form(input): Form<LoginUser>) -> Result<Response, Error> {
    debug!("Attempting to log in user: {}", &input.user);

    if input.user.is_empty() || input.pass.is_empty() {
        return render_login_with_error(
            "Username/email and password are required.",
            Some(&input.user),
        )
        .await;
    }

    // Get namespace and database from environment, using same defaults as config
    let namespace = env::var("DB_NAMESPACE").unwrap_or_else(|_| "slatehub".to_string());
    let database = env::var("DB_NAME").unwrap_or_else(|_| "main".to_string());

    // Use SurrealDB's built-in signin method
    let mut params = HashMap::new();
    params.insert("user".to_string(), input.user.clone());
    params.insert("pass".to_string(), input.pass.clone());

    let result = DB
        .signin(Record {
            namespace: &namespace,
            database: &database,
            access: "user",
            params,
        })
        .await;

    match result {
        Ok(_token) => {
            info!("Successfully authenticated user: {}", &input.user);

            // In a production app, you would:
            // 1. Use the returned token for session management
            // 2. Store it in a secure, HTTP-only cookie
            // 3. Save session info in database or cache

            // For now, just redirect to home page
            Ok(Redirect::to("/").into_response())
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

async fn logout() -> Result<Response, Error> {
    debug!("User logging out");

    // Invalidate the session token on the database side
    match DB.invalidate().await {
        Ok(_) => {
            info!("User session invalidated successfully.");

            // In a production app, you would:
            // 1. Clear the session cookie
            // 2. Remove session from database/cache

            // Redirect to home page after logout
            Ok(Redirect::to("/").into_response())
        }
        Err(e) => {
            error!("Failed to invalidate user session: {}", e);
            // Even if invalidation fails, redirect to home
            Ok(Redirect::to("/").into_response())
        }
    }
}
