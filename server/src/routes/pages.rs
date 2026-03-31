use askama::Template;
use axum::{Router, extract::Request, response::{Html, IntoResponse, Redirect, Response}, routing::get};
use axum::http::{header, HeaderValue};
use tracing::{debug, error};

use crate::{
    db::DB,
    error::Error,
    middleware::UserExtractor,
    templates::{
        AboutTemplate, Activity, BaseContext, ImpressumTemplate, IndexTemplate, PrivacyTemplate,
        TermsTemplate, User,
    },
};

pub fn router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/about", get(about))
        .route("/terms", get(terms))
        .route("/privacy", get(privacy))
        .route("/impressum", get(impressum))
        .route("/healthcheck", get(healthcheck))
        .route("/robots.txt", get(robots_txt))
        .route("/llms.txt", get(llms_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/favicon.ico", get(favicon))
}

async fn index(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering index page");

    let mut base = BaseContext::new().with_page("home");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    // Create the index template with sample data
    let mut template = IndexTemplate::new(base);

    // Add static stats data (in production, fetch from database)
    template.production_count = 1247;
    template.user_count = 5892;
    template.connection_count = 18453;

    // Add sample activities (in production, fetch from database)
    template.activities = vec![
        Activity {
            user: "Sarah Johnson".to_string(),
            action: "created a new production".to_string(),
            time: "2 minutes ago".to_string(),
        },
        Activity {
            user: "Mike Chen".to_string(),
            action: "joined the platform".to_string(),
            time: "15 minutes ago".to_string(),
        },
        Activity {
            user: "Emily Rodriguez".to_string(),
            action: "posted a job opening".to_string(),
            time: "1 hour ago".to_string(),
        },
        Activity {
            user: "David Kim".to_string(),
            action: "completed a collaboration".to_string(),
            time: "3 hours ago".to_string(),
        },
        Activity {
            user: "Lisa Thompson".to_string(),
            action: "updated their portfolio".to_string(),
            time: "5 hours ago".to_string(),
        },
    ];

    let html = template.render().map_err(|e| {
        error!("Failed to render index template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn terms(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering terms of service page");

    let mut base = BaseContext::new().with_page("terms");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = TermsTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render terms template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn privacy(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering privacy policy page");

    let mut base = BaseContext::new().with_page("privacy");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = PrivacyTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render privacy template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn about(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering about page");

    let mut base = BaseContext::new().with_page("about");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    // Query real stats from the database
    fn extract_count(row: Option<serde_json::Value>) -> usize {
        row.and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0) as usize
    }

    let (stat_creatives, stat_organizations, stat_locations, stat_jobs, stat_connections) =
        match DB
            .query(
                "SELECT count() AS count FROM person GROUP ALL;
                 SELECT count() AS count FROM organization GROUP ALL;
                 SELECT count() AS count FROM location GROUP ALL;
                 SELECT count() AS count FROM job_posting GROUP ALL;
                 SELECT count() AS count FROM member_of GROUP ALL;
                 SELECT count() AS count FROM likes GROUP ALL;
                 SELECT count() AS count FROM involvement GROUP ALL;
                 SELECT count() AS count FROM application GROUP ALL",
            )
            .await
        {
            Ok(mut response) => {
                let creatives = extract_count(response.take(0).unwrap_or(None));
                let organizations = extract_count(response.take(1).unwrap_or(None));
                let locations = extract_count(response.take(2).unwrap_or(None));
                let jobs = extract_count(response.take(3).unwrap_or(None));
                let connections = extract_count(response.take(4).unwrap_or(None))
                    + extract_count(response.take(5).unwrap_or(None))
                    + extract_count(response.take(6).unwrap_or(None))
                    + extract_count(response.take(7).unwrap_or(None));

                (creatives, organizations, locations, jobs, connections)
            }
            Err(e) => {
                error!("Failed to query about page stats: {}", e);
                (0, 0, 0, 0, 0)
            }
        };

    let template = AboutTemplate::new(base, stat_creatives, stat_organizations, stat_locations, stat_jobs, stat_connections);

    let html = template.render().map_err(|e| {
        error!("Failed to render about template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn impressum(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering impressum page");

    let mut base = BaseContext::new().with_page("impressum");

    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = ImpressumTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render impressum template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn favicon() -> Redirect {
    Redirect::permanent("/static/icons/sh-icon-red-32x32.png")
}

async fn robots_txt() -> Response {
    let base = crate::config::app_url();
    let body = format!(
        "\
User-agent: *
Allow: /
Disallow: /account
Disallow: /api/
Disallow: /admin
Disallow: /profile/edit
Disallow: /notifications
Disallow: /messages

Sitemap: {base}/sitemap.xml

# LLM/AI-friendly content index
# See https://llmstxt.org/
LLMs-Txt: {base}/llms.txt
"
    );

    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"))],
        body,
    )
        .into_response()
}

async fn llms_txt() -> Response {
    let base = crate::config::app_url();
    let mut body = format!(
        "\
# SlateHub

> The free home for filmmakers, actors, crew, and creators across film, TV, YouTube, and streaming. No subscriptions. No ads. Ever.

## About

SlateHub is a free, open-source creative networking platform — the professional home base for everyone who makes video. It connects actors, crew members, filmmakers, directors, producers, content creators, and brands across every format: feature films, television, YouTube, streaming, vertical content, branded campaigns, and podcasts. The platform features natural-language search, verified profiles, organization and community tools, production management, and a job board.

## Key Features

- **Free Forever**: No subscriptions, no ads, no pay-to-apply — talent should never pay to be discovered.
- **One Profile, Every Screen**: Film credits, YouTube collabs, branded campaigns, podcast appearances — build your entire career in one place.
- **Smart Search**: Describe who you need in plain English — by skill, location, experience, or look — and get matched instantly.
- **Verified Profiles**: One-time verification ensures real people with confirmed credits. No fake listings, no spam.
- **Open Source**: See exactly how the platform works. No hidden algorithms deciding who gets seen. Nobody pays to rank higher.
- **Organizations & Communities**: Film schools, local film communities, production companies, and collectives can manage members, share opportunities, and build their presence.
- **Production Management**: Create productions, invite cast and crew, manage roles and credits.
- **Job Board**: Post and discover opportunities across the creative industry.
- **Direct Messaging**: Contact talent and collaborators directly on the platform.

## Public Pages

- [Home]({base}/)
- [About]({base}/about)
- [Search]({base}/search)
- [People Directory]({base}/people)
- [Productions]({base}/productions)
- [Organizations]({base}/orgs)
- [Locations]({base}/locations)
- [Jobs]({base}/jobs)
- [Terms of Service]({base}/terms)
- [Privacy Policy]({base}/privacy)
- [Impressum]({base}/impressum)
"
    );

    // Dynamic entries
    if let Ok(mut result) = DB
        .query(
            "SELECT username, profile.name AS name FROM person ORDER BY username ASC;
             SELECT slug, title FROM production ORDER BY slug ASC;
             SELECT slug, name FROM organization ORDER BY slug ASC;
             SELECT <string> meta::id(id) AS key, name FROM location ORDER BY name ASC;
             SELECT <string> meta::id(id) AS key, title FROM job_posting WHERE status = 'open' ORDER BY title ASC;"
        )
        .await
    {
        // Profiles
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(0) {
            if !rows.is_empty() {
                body.push_str("\n## Profiles\n\n");
                for row in rows {
                    let username = row.get("username").and_then(|v| v.as_str()).unwrap_or_default();
                    let name = row.get("name").and_then(|v| v.as_str()).unwrap_or(username);
                    body.push_str(&format!("- [{name}]({base}/{username})\n"));
                }
            }
        }

        // Productions
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(1) {
            if !rows.is_empty() {
                body.push_str("\n## Productions\n\n");
                for row in rows {
                    let slug = row.get("slug").and_then(|v| v.as_str()).unwrap_or_default();
                    let title = row.get("title").and_then(|v| v.as_str()).unwrap_or(slug);
                    body.push_str(&format!("- [{title}]({base}/productions/{slug})\n"));
                }
            }
        }

        // Organizations
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(2) {
            if !rows.is_empty() {
                body.push_str("\n## Organizations\n\n");
                for row in rows {
                    let slug = row.get("slug").and_then(|v| v.as_str()).unwrap_or_default();
                    let name = row.get("name").and_then(|v| v.as_str()).unwrap_or(slug);
                    body.push_str(&format!("- [{name}]({base}/orgs/{slug})\n"));
                }
            }
        }

        // Locations
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(3) {
            if !rows.is_empty() {
                body.push_str("\n## Locations\n\n");
                for row in rows {
                    let key = row.get("key").and_then(|v| v.as_str()).unwrap_or_default();
                    let name = row.get("name").and_then(|v| v.as_str()).unwrap_or(key);
                    body.push_str(&format!("- [{name}]({base}/locations/{key})\n"));
                }
            }
        }

        // Jobs
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(4) {
            if !rows.is_empty() {
                body.push_str("\n## Open Jobs\n\n");
                for row in rows {
                    let key = row.get("key").and_then(|v| v.as_str()).unwrap_or_default();
                    let title = row.get("title").and_then(|v| v.as_str()).unwrap_or(key);
                    body.push_str(&format!("- [{title}]({base}/jobs/{key})\n"));
                }
            }
        }
    }

    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"))],
        body,
    )
        .into_response()
}

async fn sitemap_xml() -> Response {
    let base = crate::config::app_url();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let mut urls = Vec::new();

    // Static pages
    let static_pages = [
        ("/", "1.0", "weekly"),
        ("/about", "0.8", "monthly"),
        ("/search", "0.9", "daily"),
        ("/people", "0.9", "daily"),
        ("/productions", "0.9", "daily"),
        ("/orgs", "0.8", "daily"),
        ("/locations", "0.8", "daily"),
        ("/jobs", "0.9", "daily"),
        ("/terms", "0.3", "yearly"),
        ("/privacy", "0.3", "yearly"),
        ("/impressum", "0.3", "yearly"),
    ];

    for (path, priority, changefreq) in static_pages {
        urls.push(format!(
            "  <url>\n    <loc>{base}{path}</loc>\n    <lastmod>{today}</lastmod>\n    <changefreq>{changefreq}</changefreq>\n    <priority>{priority}</priority>\n  </url>"
        ));
    }

    // Dynamic entries — single query for all entity types
    if let Ok(mut result) = DB
        .query(
            "SELECT username FROM person ORDER BY username ASC;
             SELECT slug FROM production ORDER BY slug ASC;
             SELECT slug FROM organization ORDER BY slug ASC;
             SELECT <string> meta::id(id) AS key FROM location ORDER BY key ASC;
             SELECT <string> meta::id(id) AS key FROM job_posting ORDER BY key ASC;"
        )
        .await
    {
        // Profiles: /{username}
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(0) {
            for row in rows {
                if let Some(v) = row.get("username").and_then(|v| v.as_str()) {
                    urls.push(format!(
                        "  <url>\n    <loc>{base}/{v}</loc>\n    <changefreq>weekly</changefreq>\n    <priority>0.7</priority>\n  </url>"
                    ));
                }
            }
        }

        // Productions: /productions/{slug}
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(1) {
            for row in rows {
                if let Some(v) = row.get("slug").and_then(|v| v.as_str()) {
                    urls.push(format!(
                        "  <url>\n    <loc>{base}/productions/{v}</loc>\n    <changefreq>weekly</changefreq>\n    <priority>0.6</priority>\n  </url>"
                    ));
                }
            }
        }

        // Organizations: /orgs/{slug}
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(2) {
            for row in rows {
                if let Some(v) = row.get("slug").and_then(|v| v.as_str()) {
                    urls.push(format!(
                        "  <url>\n    <loc>{base}/orgs/{v}</loc>\n    <changefreq>weekly</changefreq>\n    <priority>0.6</priority>\n  </url>"
                    ));
                }
            }
        }

        // Locations: /locations/{key}
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(3) {
            for row in rows {
                if let Some(v) = row.get("key").and_then(|v| v.as_str()) {
                    urls.push(format!(
                        "  <url>\n    <loc>{base}/locations/{v}</loc>\n    <changefreq>weekly</changefreq>\n    <priority>0.5</priority>\n  </url>"
                    ));
                }
            }
        }

        // Jobs: /jobs/{key}
        if let Ok(rows) = result.take::<Vec<serde_json::Value>>(4) {
            for row in rows {
                if let Some(v) = row.get("key").and_then(|v| v.as_str()) {
                    urls.push(format!(
                        "  <url>\n    <loc>{base}/jobs/{v}</loc>\n    <changefreq>daily</changefreq>\n    <priority>0.6</priority>\n  </url>"
                    ));
                }
            }
        }
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{}\n</urlset>\n",
        urls.join("\n")
    );

    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/xml; charset=utf-8"))],
        xml,
    )
        .into_response()
}

async fn healthcheck() -> impl IntoResponse {
    use crate::{version, stats};

    // Check DB
    let db_ok = crate::db::DB
        .query("RETURN true")
        .await
        .is_ok();

    // Check S3
    let s3_ok = match crate::services::s3::s3() {
        Ok(s3) => s3.file_exists("_healthcheck").await.is_ok(),
        Err(_) => false,
    };

    let disk_low = stats::disk_space_low();
    let all_ok = db_ok && s3_ok && !disk_low;
    let system_stats = stats::get_stats().await;

    // Get binary file size
    let binary_size = std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| {
            let bytes = m.len();
            if bytes >= 1_073_741_824 {
                format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
            } else if bytes >= 1_048_576 {
                format!("{:.1} MB", bytes as f64 / 1_048_576.0)
            } else {
                format!("{:.0} KB", bytes as f64 / 1_024.0)
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    let body = serde_json::json!({
        "status": if all_ok { "ok" } else { "degraded" },
        "version": version::VERSION,
        "binary_size": binary_size,
        "checks": {
            "database": if db_ok { "ok" } else { "error" },
            "s3": if s3_ok { "ok" } else { "error" },
            "disk": if disk_low { "warning: < 5 GB free" } else { "ok" },
        },
        "stats": system_stats,
    });

    let status = if all_ok {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };

    let pretty = serde_json::to_string_pretty(&body).unwrap_or_default();

    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        pretty,
    )
}

