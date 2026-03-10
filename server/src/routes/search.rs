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
use crate::services::embedding::generate_embedding_async;
use crate::templates::User;

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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

    // Generate embedding for the search query (optional — text search works without it)
    let query_embedding = match generate_embedding_async(query).await {
        Ok(emb) => Some(emb),
        Err(e) => {
            debug!(
                error = %e,
                query = %query,
                "Embedding generation failed, falling back to text-only search"
            );
            None
        }
    };

    // Search all entity types
    let people = search_people(query, query_embedding.clone()).await?;
    let organizations = search_organizations(query, query_embedding.clone()).await?;
    let locations = search_locations(query, query_embedding.clone()).await?;
    let productions = search_productions(query, query_embedding).await?;

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

async fn search_people(
    query: &str,
    query_embedding: Option<Vec<f32>>,
) -> Result<Vec<PersonSearchResult>, Error> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct PersonSearchDb {
        id: String,
        name: Option<String>,
        username: Option<String>,
        headline: Option<String>,
        bio: Option<String>,
        location: Option<String>,
        skills: Option<Vec<String>>,
        avatar_url: Option<String>,
        score: f32,
    }

    let query_lower = query.to_lowercase();
    let empty_embedding: Vec<f32> = vec![];

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
                (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(username ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(profile.headline ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS score
            FROM person
            WHERE
                string::lowercase(name ?? '') CONTAINS $query_lower
                OR string::lowercase(username ?? '') CONTAINS $query_lower
                OR string::lowercase(profile.headline ?? '') CONTAINS $query_lower
                OR string::lowercase(profile.bio ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > 0.75)
            ORDER BY score DESC
            LIMIT 10",
        )
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", query_embedding.is_some()))
        .bind(("query_embedding", query_embedding.unwrap_or(empty_embedding)))
        .await
        .map_err(|e| {
            error!(error = %e, table = "person", "Database error during search");
            Error::Database(e.to_string())
        })?;

    let db_people: Vec<PersonSearchDb> = response.take(0).map_err(|e| {
        error!(error = %e, table = "person", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let people: Vec<PersonSearchResult> = db_people
        .into_iter()
        .filter(|p| p.score > 0.0)
        .map(|p| {
            let name = p.name.unwrap_or_default();
            let username = p.username.unwrap_or_default();
            let initials = name
                .split_whitespace()
                .filter_map(|word| word.chars().next())
                .take(2)
                .collect::<String>()
                .to_uppercase();

            PersonSearchResult {
                id: p.id,
                name,
                username,
                headline: p.headline,
                bio: p.bio,
                location: p.location,
                skills: p.skills.unwrap_or_default(),
                avatar_url: p.avatar_url,
                initials,
                score: p.score.round() as i32,
            }
        })
        .collect();

    Ok(people)
}

async fn search_organizations(
    query: &str,
    query_embedding: Option<Vec<f32>>,
) -> Result<Vec<OrganizationSearchResult>, Error> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct OrganizationSearchDb {
        id: String,
        name: Option<String>,
        slug: Option<String>,
        description: Option<String>,
        location: Option<String>,
        logo: Option<String>,
        score: f32,
    }

    let query_lower = query.to_lowercase();
    let empty_embedding: Vec<f32> = vec![];

    let mut response = DB
        .query(
            "SELECT
                <string> id AS id,
                name,
                slug,
                description,
                location,
                logo,
                (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(slug ?? '') CONTAINS $query_lower THEN 30 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS score
            FROM organization
            WHERE
                string::lowercase(name ?? '') CONTAINS $query_lower
                OR string::lowercase(slug ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > 0.75)
            ORDER BY score DESC
            LIMIT 10",
        )
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", query_embedding.is_some()))
        .bind(("query_embedding", query_embedding.unwrap_or(empty_embedding)))
        .await
        .map_err(|e| {
            error!(error = %e, table = "organization", "Database error during search");
            Error::Database(e.to_string())
        })?;

    let db_organizations: Vec<OrganizationSearchDb> = response.take(0).map_err(|e| {
        error!(error = %e, table = "organization", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let organizations: Vec<OrganizationSearchResult> = db_organizations
        .into_iter()
        .filter(|o| o.score > 0.0)
        .map(|o| OrganizationSearchResult {
            id: o.id,
            name: o.name.unwrap_or_default(),
            slug: o.slug.unwrap_or_default(),
            description: o.description,
            location: o.location,
            logo: o.logo,
            score: o.score.round() as i32,
        })
        .collect();

    Ok(organizations)
}

async fn search_locations(
    query: &str,
    query_embedding: Option<Vec<f32>>,
) -> Result<Vec<LocationSearchResult>, Error> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct LocationSearchDb {
        id: String,
        name: Option<String>,
        address: Option<String>,
        city: Option<String>,
        state: Option<String>,
        description: Option<String>,
        score: f32,
    }

    let query_lower = query.to_lowercase();
    let empty_embedding: Vec<f32> = vec![];

    let mut response = DB
        .query(
            "SELECT
                <string> id AS id,
                name,
                address,
                city,
                state,
                description,
                (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(city ?? '') CONTAINS $query_lower THEN 30 ELSE 0 END)
                    + (IF string::lowercase(state ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF string::lowercase(address ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN 10 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS score
            FROM location
            WHERE is_public = true AND (
                string::lowercase(name ?? '') CONTAINS $query_lower
                OR string::lowercase(city ?? '') CONTAINS $query_lower
                OR string::lowercase(state ?? '') CONTAINS $query_lower
                OR string::lowercase(address ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > 0.75)
            )
            ORDER BY score DESC
            LIMIT 10",
        )
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", query_embedding.is_some()))
        .bind(("query_embedding", query_embedding.unwrap_or(empty_embedding)))
        .await
        .map_err(|e| {
            error!(error = %e, table = "location", "Database error during search");
            Error::Database(e.to_string())
        })?;

    let db_locations: Vec<LocationSearchDb> = response.take(0).map_err(|e| {
        error!(error = %e, table = "location", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let locations: Vec<LocationSearchResult> = db_locations
        .into_iter()
        .filter(|l| l.score > 0.0)
        .map(|l| LocationSearchResult {
            id: l.id,
            name: l.name.unwrap_or_default(),
            address: l.address.unwrap_or_default(),
            city: l.city.unwrap_or_default(),
            state: l.state.unwrap_or_default(),
            description: l.description,
            score: l.score.round() as i32,
        })
        .collect();

    Ok(locations)
}

async fn search_productions(
    query: &str,
    query_embedding: Option<Vec<f32>>,
) -> Result<Vec<ProductionSearchResult>, Error> {
    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct ProductionSearchDb {
        id: String,
        title: Option<String>,
        status: Option<String>,
        description: Option<String>,
        location: Option<String>,
        score: f32,
    }

    let query_lower = query.to_lowercase();
    let empty_embedding: Vec<f32> = vec![];

    let mut response = DB
        .query(
            "SELECT
                <string> id AS id,
                title,
                status,
                description,
                location,
                (
                    (IF string::lowercase(title ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS score
            FROM production
            WHERE
                string::lowercase(title ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR string::lowercase(location ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > 0.75)
            ORDER BY score DESC
            LIMIT 10",
        )
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", query_embedding.is_some()))
        .bind(("query_embedding", query_embedding.unwrap_or(empty_embedding)))
        .await
        .map_err(|e| {
            error!(error = %e, table = "production", "Database error during search");
            Error::Database(e.to_string())
        })?;

    let db_productions: Vec<ProductionSearchDb> = response.take(0).map_err(|e| {
        error!(error = %e, table = "production", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let productions: Vec<ProductionSearchResult> = db_productions
        .into_iter()
        .filter(|p| p.score > 0.0)
        .map(|p| ProductionSearchResult {
            id: p.id,
            title: p.title.unwrap_or_default(),
            status: p.status.unwrap_or_default(),
            description: p.description,
            location: p.location,
            score: p.score.round() as i32,
        })
        .collect();

    Ok(productions)
}
