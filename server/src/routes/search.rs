use askama::Template;
use axum::{
    Router,
    extract::{Query, Request},
    response::{Html, IntoResponse},
    routing::get,
};
use chrono::Datelike;
use serde::Deserialize;
use surrealdb::types::SurrealValue;
use tracing::{debug, error};

use crate::db::DB;
use crate::error::Error;
use crate::middleware::UserExtractor;
use crate::services::embedding::generate_embedding;
use crate::templates::User;

#[derive(Template)]
#[template(path = "search/index.html")]
struct SearchTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    query: Option<String>,
    has_results: bool,
    total_results: usize,
    people: Vec<PersonSearchResult>,
    organizations: Vec<OrganizationSearchResult>,
    locations: Vec<LocationSearchResult>,
    productions: Vec<ProductionSearchResult>,
}

#[derive(Debug, serde::Deserialize)]
struct PersonSearchResult {
    id: String,
    name: String,
    username: String,
    headline: Option<String>,
    bio: Option<String>,
    location: Option<String>,
    skills: Vec<String>,
    avatar_url: Option<String>,
    initials: String,
    score: i32,
}

#[derive(Debug, serde::Deserialize)]
struct OrganizationSearchResult {
    id: String,
    name: String,
    slug: String,
    description: Option<String>,
    location: Option<String>,
    logo: Option<String>,
    score: i32,
}

#[derive(Debug, serde::Deserialize)]
struct LocationSearchResult {
    id: String,
    name: String,
    address: String,
    city: String,
    state: String,
    description: Option<String>,
    score: i32,
}

#[derive(Debug, serde::Deserialize)]
struct ProductionSearchResult {
    id: String,
    title: String,
    status: String,
    description: Option<String>,
    location: Option<String>,
    score: i32,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
}

pub fn router() -> Router {
    Router::new().route("/search", get(search_page))
}

async fn search_page(
    Query(params): Query<SearchQuery>,
    request: Request,
) -> Result<impl IntoResponse, Error> {
    let query = params.q.as_deref().unwrap_or("").trim();

    // Extract user from request
    let user = if let Some(session_user) = request.get_user() {
        Some(User::from_session_user(&session_user).await)
    } else {
        None
    };

    if query.is_empty() {
        // Show empty search page
        let template = SearchTemplate {
            app_name: "SlateHub".to_string(),
            year: chrono::Utc::now().year(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            active_page: "search".to_string(),
            user: user.clone(),
            query: None,
            has_results: false,
            total_results: 0,
            people: vec![],
            organizations: vec![],
            locations: vec![],
            productions: vec![],
        };

        let html = template.render().map_err(|e| {
            error!("Failed to render search template: {}", e);
            Error::Template(e.to_string())
        })?;

        return Ok(Html(html));
    }

    debug!("Search query: {}", query);

    // Generate embedding for the search query
    let query_embedding = match generate_embedding(query) {
        Ok(emb) => emb,
        Err(e) => {
            error!(
                error = %e,
                error_debug = ?e,
                query = %query,
                "Failed to generate embedding for search query - embedding service may not be initialized"
            );
            return Err(Error::Internal(
                "Failed to process search query - embedding service error".to_string(),
            ));
        }
    };

    // Search people
    let people = search_people(query_embedding.clone()).await?;

    // Search organizations
    let organizations = search_organizations(query_embedding.clone()).await?;

    // Search locations
    let locations = search_locations(query_embedding.clone()).await?;

    // Search productions
    let productions = search_productions(query_embedding.clone()).await?;

    let total_results = people.len() + organizations.len() + locations.len() + productions.len();

    let template = SearchTemplate {
        app_name: "SlateHub".to_string(),
        year: chrono::Utc::now().year(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        active_page: "search".to_string(),
        user,
        query: Some(query.to_string()),
        has_results: total_results > 0,
        total_results,
        people,
        organizations,
        locations,
        productions,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render search template: {}", e);
        Error::Template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn search_people(query_embedding: Vec<f32>) -> Result<Vec<PersonSearchResult>, Error> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct PersonSearchDb {
        id: String,
        name: String,
        username: String,
        headline: Option<String>,
        bio: Option<String>,
        location: Option<String>,
        skills: Vec<String>,
        avatar_url: Option<String>,
        score: f32,
    }

    let mut response = DB
        .query(
            "SELECT
                <string> id AS id,
                name,
                username,
                profile.headline AS headline,
                profile.bio AS bio,
                profile.location AS location,
                profile.skills AS skills,
                profile.avatar AS avatar_url,
                vector::similarity::cosine(embedding, $query_embedding) AS score
            FROM person
            WHERE embedding IS NOT NONE
            ORDER BY score DESC
            LIMIT 10",
        )
        .bind(("query_embedding", query_embedding))
        .await
        .map_err(|e| {
            error!(
                error = %e,
                error_debug = ?e,
                table = "person",
                "Database error during vector search"
            );
            Error::Database(e.to_string())
        })?;

    let db_people: Vec<PersonSearchDb> = response.take(0).map_err(|e| {
        error!(
            error = %e,
            error_debug = ?e,
            table = "person",
            "Failed to deserialize search results"
        );
        Error::Database(e.to_string())
    })?;

    // Convert to display results with score as percentage
    let people: Vec<PersonSearchResult> = db_people
        .into_iter()
        .map(|p| {
            let initials = p
                .name
                .split_whitespace()
                .filter_map(|word| word.chars().next())
                .take(2)
                .collect::<String>()
                .to_uppercase();

            PersonSearchResult {
                id: p.id,
                name: p.name,
                username: p.username,
                headline: p.headline,
                bio: p.bio,
                location: p.location,
                skills: p.skills,
                avatar_url: p.avatar_url,
                initials,
                score: (p.score * 100.0).round() as i32,
            }
        })
        .filter(|p| p.score >= 50)
        .collect();

    Ok(people)
}

async fn search_organizations(
    query_embedding: Vec<f32>,
) -> Result<Vec<OrganizationSearchResult>, Error> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct OrganizationSearchDb {
        id: String,
        name: String,
        slug: String,
        description: Option<String>,
        location: Option<String>,
        logo: Option<String>,
        score: f32,
    }

    let mut response = DB
        .query(
            "SELECT
                <string> id AS id,
                name,
                slug,
                description,
                location,
                logo,
                vector::similarity::cosine(embedding, $query_embedding) AS score
            FROM organization
            WHERE embedding IS NOT NONE
            ORDER BY score DESC
            LIMIT 10",
        )
        .bind(("query_embedding", query_embedding))
        .await
        .map_err(|e| {
            error!(
                error = %e,
                error_debug = ?e,
                table = "organization",
                "Database error during vector search"
            );
            Error::Database(e.to_string())
        })?;

    let db_organizations: Vec<OrganizationSearchDb> = response.take(0).map_err(|e| {
        error!(
            error = %e,
            error_debug = ?e,
            table = "organization",
            "Failed to deserialize search results"
        );
        Error::Database(e.to_string())
    })?;

    // Convert to display results with score as percentage
    let organizations: Vec<OrganizationSearchResult> = db_organizations
        .into_iter()
        .map(|o| OrganizationSearchResult {
            id: o.id,
            name: o.name,
            slug: o.slug,
            description: o.description,
            location: o.location,
            logo: o.logo,
            score: (o.score * 100.0).round() as i32,
        })
        .filter(|o| o.score >= 50)
        .collect();

    Ok(organizations)
}

async fn search_locations(query_embedding: Vec<f32>) -> Result<Vec<LocationSearchResult>, Error> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct LocationSearchDb {
        id: String,
        name: String,
        address: String,
        city: String,
        state: String,
        description: Option<String>,
        score: f32,
    }

    let mut response = DB
        .query(
            "SELECT
                <string> id AS id,
                name,
                address,
                city,
                state,
                description,
                vector::similarity::cosine(embedding, $query_embedding) AS score
            FROM location
            WHERE embedding IS NOT NONE AND is_public = true
            ORDER BY score DESC
            LIMIT 10",
        )
        .bind(("query_embedding", query_embedding))
        .await
        .map_err(|e| {
            error!(
                error = %e,
                error_debug = ?e,
                table = "location",
                "Database error during vector search"
            );
            Error::Database(e.to_string())
        })?;

    let db_locations: Vec<LocationSearchDb> = response.take(0).map_err(|e| {
        error!(
            error = %e,
            error_debug = ?e,
            table = "location",
            "Failed to deserialize search results"
        );
        Error::Database(e.to_string())
    })?;

    // Convert to display results with score as percentage
    let locations: Vec<LocationSearchResult> = db_locations
        .into_iter()
        .map(|l| LocationSearchResult {
            id: l.id,
            name: l.name,
            address: l.address,
            city: l.city,
            state: l.state,
            description: l.description,
            score: (l.score * 100.0).round() as i32,
        })
        .filter(|l| l.score >= 50)
        .collect();

    Ok(locations)
}

async fn search_productions(
    query_embedding: Vec<f32>,
) -> Result<Vec<ProductionSearchResult>, Error> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct ProductionSearchDb {
        id: String,
        title: String,
        status: String,
        description: Option<String>,
        location: Option<String>,
        score: f32,
    }

    let mut response = DB
        .query(
            "SELECT
                <string> id AS id,
                title,
                status,
                description,
                location,
                vector::similarity::cosine(embedding, $query_embedding) AS score
            FROM production
            WHERE embedding IS NOT NONE
            ORDER BY score DESC
            LIMIT 10",
        )
        .bind(("query_embedding", query_embedding))
        .await
        .map_err(|e| {
            error!(
                error = %e,
                error_debug = ?e,
                table = "production",
                "Database error during vector search"
            );
            Error::Database(e.to_string())
        })?;

    let db_productions: Vec<ProductionSearchDb> = response.take(0).map_err(|e| {
        error!(
            error = %e,
            error_debug = ?e,
            table = "production",
            "Failed to deserialize search results"
        );
        Error::Database(e.to_string())
    })?;

    // Convert to display results with score as percentage
    let productions: Vec<ProductionSearchResult> = db_productions
        .into_iter()
        .map(|p| ProductionSearchResult {
            id: p.id,
            title: p.title,
            status: p.status,
            description: p.description,
            location: p.location,
            score: (p.score * 100.0).round() as i32,
        })
        .filter(|p| p.score >= 50)
        .collect();

    Ok(productions)
}
