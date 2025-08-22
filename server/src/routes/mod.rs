use axum::{
    Form, Json, Router,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, get_service, post},
};
use serde::Serialize;
use std::collections::HashMap;
use surrealdb::opt::auth::Record;
use tower_http::{
    compression::CompressionLayer,
    services::ServeDir,
    trace::{self, TraceLayer},
};
use tracing::{Level, debug, error, info};

use crate::{
    db::DB,
    error::Error,
    models::person::{CreateUser, LoginUser, Person},
    sse, templates,
};

pub fn app() -> Router {
    // Static file service
    let static_service = ServeDir::new("static")
        .append_index_html_on_directories(false)
        .precompressed_gzip()
        .precompressed_br();

    Router::new()
        // Page routes
        .route("/", get(index))
        .route("/projects", get(projects))
        .route("/people", get(people))
        .route("/about", get(about))
        // Auth routes
        .route("/signup", get(signup_form).post(signup))
        .route("/login", get(login_form).post(login))
        .route("/logout", post(logout))
        // API routes
        .route("/api/health", get(health_check))
        .route("/api/stats", get(stats))
        // SSE routes for real-time updates
        .route("/api/sse/stats", get(sse_stats))
        .route("/api/sse/activity", get(sse_activity))
        // Static files
        .nest_service("/static", get_service(static_service))
        // Middleware
        .layer(CompressionLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_request(trace::DefaultOnRequest::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO))
                .on_failure(trace::DefaultOnFailure::new().level(Level::ERROR)),
        )
}

// Page handlers

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

// API handlers

// Auth handlers

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

#[derive(Serialize)]
struct HealthStatus {
    status: String,
    database: String,
    version: String,
    timestamp: String,
}

async fn health_check() -> Json<HealthStatus> {
    debug!("Health check requested");

    // Check database connectivity
    let db_status = match crate::db::DB.health().await {
        Ok(_) => {
            info!("Database health check: OK");
            "connected"
        }
        Err(e) => {
            tracing::warn!("Database health check failed: {:?}", e);
            "disconnected"
        }
    };

    let health = HealthStatus {
        status: "healthy".to_string(),
        database: db_status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    info!(
        "Health check complete: status={}, db={}",
        health.status, health.database
    );

    Json(health)
}

#[derive(Serialize)]
struct PlatformStats {
    projects: u32,
    users: u32,
    connections: u32,
}

async fn stats() -> Json<PlatformStats> {
    debug!("Stats endpoint called");

    // In production, these would be fetched from the database
    // For now, return mock data
    let stats = PlatformStats {
        projects: 1247,
        users: 5892,
        connections: 18453,
    };

    Json(stats)
}

// SSE handlers

async fn sse_stats() -> impl axum::response::IntoResponse {
    debug!("SSE stats stream requested");
    sse::stats_stream().await
}

async fn sse_activity() -> impl axum::response::IntoResponse {
    debug!("SSE activity stream requested");
    sse::activity_stream().await
}

// Error handling is now centralized in src/error.rs
