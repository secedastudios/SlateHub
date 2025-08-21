use axum::{
    Json, Router,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, get_service},
};
use serde::Serialize;
use tower_http::{
    compression::CompressionLayer,
    services::ServeDir,
    trace::{self, TraceLayer},
};
use tracing::{Level, debug, error, info};

use crate::templates;

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
        // API routes
        .route("/api/health", get(health_check))
        .route("/api/stats", get(stats))
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

async fn index() -> Result<Html<String>, AppError> {
    debug!("Rendering index page");

    let mut context = templates::base_context();
    context.insert("active_page", "home");

    let html = templates::render_with_context("index.html", &context).map_err(|e| {
        error!("Failed to render index template: {}", e);
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

async fn projects() -> Result<Html<String>, AppError> {
    debug!("Rendering projects page");

    let mut context = templates::base_context();
    context.insert("active_page", "projects");
    context.insert("page_title", "Projects");

    // For now, return a simple page. In production, you'd fetch actual projects
    let html = format!(
        r#"<!DOCTYPE html>
        <html lang="en" data-theme="light">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Projects - SlateHub</title>
            <link rel="stylesheet" href="/static/css/semantic.css">
            <script type="module" src="https://cdn.jsdelivr.net/npm/@sudodevnull/datastar@0.19.10/+esm"></script>
            <script>
                const savedTheme = localStorage.getItem('theme') || 'light';
                document.documentElement.setAttribute('data-theme', savedTheme);
            </script>
        </head>
        <body>
            <header id="site-header">
                <nav class="main-navigation">
                    <div class="nav-brand">
                        <a href="/" class="brand-link">SlateHub</a>
                    </div>
                    <ul class="nav-menu">
                        <li class="nav-item">
                            <a href="/" class="nav-link">Home</a>
                        </li>
                        <li class="nav-item">
                            <a href="/projects" class="nav-link active">Projects</a>
                        </li>
                        <li class="nav-item">
                            <a href="/people" class="nav-link">People</a>
                        </li>
                        <li class="nav-item">
                            <a href="/about" class="nav-link">About</a>
                        </li>
                    </ul>
                    <div class="nav-actions">
                        <button id="theme-toggle" class="theme-switcher" onclick="toggleTheme()" aria-label="Toggle dark mode">
                            <span class="theme-icon light-icon" aria-hidden="true">☀️</span>
                            <span class="theme-icon dark-icon" aria-hidden="true">🌙</span>
                        </button>
                    </div>
                </nav>
            </header>
            <main id="main-content">
                <div class="container">
                    <h1>Projects</h1>
                    <p>Coming soon: Browse and manage creative projects.</p>
                </div>
            </main>
            <script>
                function toggleTheme() {{
                    const currentTheme = document.documentElement.getAttribute('data-theme');
                    const newTheme = currentTheme === 'light' ? 'dark' : 'light';
                    document.documentElement.setAttribute('data-theme', newTheme);
                    localStorage.setItem('theme', newTheme);
                }}
            </script>
        </body>
        </html>"#
    );

    Ok(Html(html))
}

async fn people() -> Result<Html<String>, AppError> {
    debug!("Rendering people page");

    let mut context = templates::base_context();
    context.insert("active_page", "people");
    context.insert("page_title", "People");

    // Temporary fallback
    let html = format!(
        r#"<!DOCTYPE html>
        <html lang="en" data-theme="light">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>People - SlateHub</title>
            <link rel="stylesheet" href="/static/css/semantic.css">
            <script type="module" src="https://cdn.jsdelivr.net/npm/@sudodevnull/datastar@0.19.10/+esm"></script>
            <script>
                const savedTheme = localStorage.getItem('theme') || 'light';
                document.documentElement.setAttribute('data-theme', savedTheme);
            </script>
        </head>
        <body>
            <header id="site-header">
                <nav class="main-navigation">
                    <div class="nav-brand">
                        <a href="/" class="brand-link">SlateHub</a>
                    </div>
                    <ul class="nav-menu">
                        <li class="nav-item">
                            <a href="/" class="nav-link">Home</a>
                        </li>
                        <li class="nav-item">
                            <a href="/projects" class="nav-link">Projects</a>
                        </li>
                        <li class="nav-item">
                            <a href="/people" class="nav-link active">People</a>
                        </li>
                        <li class="nav-item">
                            <a href="/about" class="nav-link">About</a>
                        </li>
                    </ul>
                    <div class="nav-actions">
                        <button id="theme-toggle" class="theme-switcher" onclick="toggleTheme()" aria-label="Toggle dark mode">
                            <span class="theme-icon light-icon" aria-hidden="true">☀️</span>
                            <span class="theme-icon dark-icon" aria-hidden="true">🌙</span>
                        </button>
                    </div>
                </nav>
            </header>
            <main id="main-content">
                <div class="container">
                    <h1>People</h1>
                    <p>Coming soon: Connect with creative professionals.</p>
                </div>
            </main>
            <script>
                function toggleTheme() {{
                    const currentTheme = document.documentElement.getAttribute('data-theme');
                    const newTheme = currentTheme === 'light' ? 'dark' : 'light';
                    document.documentElement.setAttribute('data-theme', newTheme);
                    localStorage.setItem('theme', newTheme);
                }}
            </script>
        </body>
        </html>"#
    );

    Ok(Html(html))
}

async fn about() -> Result<Html<String>, AppError> {
    debug!("Rendering about page");

    let mut context = templates::base_context();
    context.insert("active_page", "about");
    context.insert("page_title", "About");

    // Temporary fallback
    let html = format!(
        r#"<!DOCTYPE html>
        <html lang="en" data-theme="light">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>About - SlateHub</title>
            <link rel="stylesheet" href="/static/css/semantic.css">
            <script type="module" src="https://cdn.jsdelivr.net/npm/@sudodevnull/datastar@0.19.10/+esm"></script>
            <script>
                const savedTheme = localStorage.getItem('theme') || 'light';
                document.documentElement.setAttribute('data-theme', savedTheme);
            </script>
        </head>
        <body>
            <header id="site-header">
                <nav class="main-navigation">
                    <div class="nav-brand">
                        <a href="/" class="brand-link">SlateHub</a>
                    </div>
                    <ul class="nav-menu">
                        <li class="nav-item">
                            <a href="/" class="nav-link">Home</a>
                        </li>
                        <li class="nav-item">
                            <a href="/projects" class="nav-link">Projects</a>
                        </li>
                        <li class="nav-item">
                            <a href="/people" class="nav-link">People</a>
                        </li>
                        <li class="nav-item">
                            <a href="/about" class="nav-link active">About</a>
                        </li>
                    </ul>
                    <div class="nav-actions">
                        <button id="theme-toggle" class="theme-switcher" onclick="toggleTheme()" aria-label="Toggle dark mode">
                            <span class="theme-icon light-icon" aria-hidden="true">☀️</span>
                            <span class="theme-icon dark-icon" aria-hidden="true">🌙</span>
                        </button>
                    </div>
                </nav>
            </header>
            <main id="main-content">
                <div class="container">
                    <h1>About SlateHub</h1>
                    <p>SlateHub is a free, open-source SaaS platform for the TV, film, and content industries.</p>
                    <p>We combine the networking capabilities of LinkedIn with the project management features of GitHub,
                       specifically tailored for creative professionals.</p>
                </div>
            </main>
            <script>
                function toggleTheme() {{
                    const currentTheme = document.documentElement.getAttribute('data-theme');
                    const newTheme = currentTheme === 'light' ? 'dark' : 'light';
                    document.documentElement.setAttribute('data-theme', newTheme);
                    localStorage.setItem('theme', newTheme);
                }}
            </script>
        </body>
        </html>"#
    );

    Ok(Html(html))
}

// API handlers

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

// Error handling

#[derive(Debug)]
enum AppError {
    TemplateError(String),
    DatabaseError(String),
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::TemplateError(msg) => {
                error!("Template error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Template rendering failed",
                )
            }
            AppError::DatabaseError(msg) => {
                error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error occurred")
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, "Page not found"),
        };

        (status, message).into_response()
    }
}
