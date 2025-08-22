use axum::{
    Form, Router,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use std::collections::HashMap;
use surrealdb::opt::auth::Record;
use tracing::{debug, error, info};

use crate::{
    db::DB,
    error::Error,
    models::person::{CreateUser, LoginUser, Person},
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

async fn signup(Form(input): Form<CreateUser>) -> Result<impl IntoResponse, Error> {
    debug!("Attempting to sign up new user: {}", input.username);

    // --- Basic Validation ---
    if input.username.is_empty() || input.email.is_empty() {
        return Err(Error::bad_request("Username and email are required."));
    }
    if input.password.len() < 8 {
        return Err(Error::bad_request(
            "Password must be at least 8 characters long.",
        ));
    }

    // --- Database Signup ---
    // SurrealDB's `signup` will handle the user creation and password hashing
    // as defined in the `user` scope in `schema.surql`.
    // Note: We are using hardcoded NS and DB for now. This should be
    // retrieved from config state in a production setup.
    // SurrealDB's `signup` method expects a tuple containing the scope name
    // and a map of variables.
    let mut vars = HashMap::new();
    vars.insert("user".to_string(), input.username.clone());
    vars.insert("email".to_string(), input.email.clone());
    vars.insert("pass".to_string(), input.password.clone());

    let result = DB
        .signup(Record {
            namespace: "default",
            database: "default",
            access: "user",
            params: vars,
        })
        .await;

    match result {
        Ok(_) => {
            info!("Successfully signed up user: {}", input.username);
            // On success, we return a CREATED status. Datastar can then redirect.
            Ok(StatusCode::CREATED)
        }
        Err(e) => {
            error!("Failed to sign up user '{}': {}", input.username, e);
            if e.to_string().contains("already exists") {
                Err(Error::conflict("Username or email is already in use."))
            } else {
                Err(Error::database("An unexpected error occurred."))
            }
        }
    }
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

async fn login(Form(input): Form<LoginUser>) -> Result<impl IntoResponse, Error> {
    debug!("Attempting to log in user: {}", &input.user);

    if input.user.is_empty() || input.pass.is_empty() {
        return Err(Error::bad_request(
            "Username/email and password are required.",
        ));
    }

    // Manually query to validate credentials without changing the root connection's auth state.
    // This is safer for a shared DB connection. The query will only return a result if the user
    // exists AND the password is correct.
    let sql = "SELECT * FROM person WHERE (username = $user OR email = $user) AND crypto::argon2::compare(password, $pass)";

    // Move the input values into owned strings to satisfy the static lifetime requirement of `bind`.
    let user_identifier = input.user;
    let password = input.pass;

    let mut response = DB
        .query(sql)
        .bind(("user", user_identifier.clone()))
        .bind(("pass", password))
        .await?;

    // Check if the query returned a user record.
    match response.take::<Option<Person>>(0) {
        Ok(Some(_person)) => {
            info!("Successfully authenticated user: {}", &user_identifier);
            // In a real app, you would generate a session token (JWT) here
            // and set it in a secure, HTTP-only cookie.
            // For now, Datastar will receive the 200 OK and can handle redirection.
            Ok(StatusCode::OK)
        }
        Ok(None) => {
            error!("Invalid credentials for user '{}'", &user_identifier);
            Err(Error::Unauthorized)
        }
        Err(e) => {
            error!(
                "Authentication query failed for user '{}': {}",
                &user_identifier, e
            );
            Err(Error::Internal)
        }
    }
}

async fn logout() -> Result<impl IntoResponse, Error> {
    debug!("User logging out");

    // Invalidate the session token on the database side.
    match DB.invalidate().await {
        Ok(_) => {
            info!("User session invalidated successfully.");
            // On success, Datastar will receive a 200 OK and can redirect.
            // In a full implementation, we would also clear the session cookie here.
            Ok(StatusCode::OK)
        }
        Err(e) => {
            error!("Failed to invalidate user session: {}", e);
            Err(Error::Internal)
        }
    }
}
