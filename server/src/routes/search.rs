use askama::Template;
use axum::{
    Router,
    extract::{Query, Request},
    response::{Html, IntoResponse},
    routing::get,
};
use chrono::Datelike;
use serde::Deserialize;
use tracing::{debug, error};

use regex::Regex;

use surrealdb::types::RecordId;

use crate::config;
use crate::error::Error;
use crate::middleware::UserExtractor;
use crate::models::likes::LikesModel;
use crate::services::embedding::generate_embedding_async;
use crate::services::search::{
    JobSearchResult, LocationSearchResult, OrganizationSearchResult, ProductionSearchResult,
    SearchParams,
};
use crate::services::search_log::log_search;
use crate::services::search_utils;
use crate::templates::User;

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }

    pub fn contains(list: &[String], value: &String) -> askama::Result<bool> {
        Ok(list.contains(value))
    }
}

/// Thin wrapper around `services::search::PersonSearchResult` that adds
/// template-only fields (like `initials`) not present in the canonical type.
#[allow(dead_code)]
struct PersonView {
    id: String,
    name: String,
    username: String,
    headline: Option<String>,
    bio: Option<String>,
    location: Option<String>,
    skills: Vec<String>,
    avatar_url: Option<String>,
    initials: String,
    score: f64,
}

impl From<crate::services::search::PersonSearchResult> for PersonView {
    fn from(p: crate::services::search::PersonSearchResult) -> Self {
        let initials = p
            .name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase();
        PersonView {
            id: p.id,
            name: p.name,
            username: p.username,
            headline: p.headline,
            bio: p.bio,
            location: p.location,
            skills: p.skills,
            avatar_url: p.avatar_url,
            initials,
            score: p.score,
        }
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
    people: Vec<PersonView>,
    organizations: Vec<OrganizationSearchResult>,
    locations: Vec<LocationSearchResult>,
    productions: Vec<ProductionSearchResult>,
    jobs: Vec<JobSearchResult>,
    liked_ids: Vec<String>,
    current_user_id: String,
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
    let (user, current_user_id) = if let Some(session_user) = request.get_user() {
        let uid = session_user.id.clone();
        (Some(User::from_session_user(&session_user).await), Some(uid))
    } else {
        (None, None)
    };

    if query.is_empty() {
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
            jobs: vec![],
            liked_ids: vec![],
            current_user_id: current_user_id.clone().unwrap_or_default(),
        };

        let html = template.render().map_err(|e| {
            error!("Failed to render search template: {}", e);
            Error::Template(e.to_string())
        })?;

        return Ok(Html(html));
    }

    debug!("Search query: {}", query);

    // Detect which entity types the query targets
    let intent = detect_search_intent(query);
    debug!("Search intent: {:?}", intent);

    // Generate embedding once for all search functions
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

    let weights = config::search_weights();

    // --- People: use parse_query for structured filter extraction ---
    let people = if intent.people {
        let parsed = search_utils::parse_query(query);
        let search_params = SearchParams {
            query: &parsed.cleaned,
            embedding: query_embedding.as_ref(),
            weights,
            limit: 20,
            offset: 0,
        };
        crate::services::search::search_people(&search_params, &parsed, None)
            .await?
            .into_iter()
            .map(PersonView::from)
            .collect()
    } else {
        vec![]
    };

    // --- Non-people: extract location, normalize remaining query ---
    let (location, cleaned_query) = search_utils::extract_location(query);
    let normalized = search_utils::normalize_query(&cleaned_query);

    let organizations = if intent.organizations {
        let search_params = SearchParams {
            query: &normalized,
            embedding: query_embedding.as_ref(),
            weights,
            limit: 10,
            offset: 0,
        };
        crate::services::search::search_organizations(
            &search_params,
            location.as_deref(),
        )
        .await?
    } else {
        vec![]
    };

    let locations = if intent.locations {
        let search_params = SearchParams {
            query: &normalized,
            embedding: query_embedding.as_ref(),
            weights,
            limit: 10,
            offset: 0,
        };
        // For locations, pass extracted location as city filter
        crate::services::search::search_locations(
            &search_params,
            location.as_deref(),
            None,
        )
        .await?
    } else {
        vec![]
    };

    let productions = if intent.productions {
        let search_params = SearchParams {
            query: &normalized,
            embedding: query_embedding.as_ref(),
            weights,
            limit: 10,
            offset: 0,
        };
        crate::services::search::search_productions(&search_params, None).await?
    } else {
        vec![]
    };

    let jobs = if intent.jobs {
        let search_params = SearchParams {
            query: &normalized,
            embedding: query_embedding.as_ref(),
            weights,
            limit: 10,
            offset: 0,
        };
        crate::services::search::search_jobs(
            &search_params,
            location.as_deref(),
            true,
        )
        .await?
    } else {
        vec![]
    };

    let total_results =
        people.len() + organizations.len() + locations.len() + productions.len() + jobs.len();

    log_search(query, "web", "all", Some(total_results));

    // Fetch liked IDs for people results if user is logged in
    let liked_ids = if let Some(ref uid) = current_user_id {
        let person_rid = if uid.starts_with("person:") {
            RecordId::parse_simple(uid).ok()
        } else {
            Some(RecordId::new("person", uid.as_str()))
        };
        if let Some(rid) = person_rid {
            let target_ids: Vec<RecordId> = people
                .iter()
                .filter_map(|p| RecordId::parse_simple(&p.id).ok())
                .collect();
            LikesModel::get_liked_ids(&rid, &target_ids)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

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
        jobs,
        liked_ids,
        current_user_id: current_user_id.unwrap_or_default(),
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render search template: {}", e);
        Error::Template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Which entity types a search query targets.
#[derive(Debug)]
struct SearchIntent {
    people: bool,
    organizations: bool,
    locations: bool,
    productions: bool,
    jobs: bool,
}

/// Detect which entity types the query is targeting based on keywords.
/// Returns all true (search everything) if no clear intent is detected.
fn detect_search_intent(query: &str) -> SearchIntent {
    let q = query.to_lowercase();

    // Location signals — physical places, venues, spaces
    let is_locations = Regex::new(
        r"(?i)\b(location|locations|venue|venues|sound stage|soundstage|stage|warehouse|rooftop|loft|desert|beach|forest|outdoor|indoor|studio space|filming location|shoot location)\b"
    ).unwrap().is_match(&q);

    // Organization signals — companies, agencies, service providers
    let is_orgs = Regex::new(
        r"(?i)\b(company|companies|agency|agencies|house|houses|rental|rentals|post house|vfx house|production company|production companies|talent agency|casting agency|studio|studios)\b"
    ).unwrap().is_match(&q)
        // "studio" alone is ambiguous — only count as org if combined with service words
        // but "studios" (plural) is more likely an org
        && !is_locations; // location takes priority if both match (e.g. "studio space")

    // Production signals — films, shows, credits
    let is_productions = Regex::new(
        r"(?i)\b(film|films|movie|movies|show|shows|series|season|documentary|documentaries|short film|shorts|feature|features|directed by|starring|produced by|written by|credits)\b"
    ).unwrap().is_match(&q);

    // Job signals — employment, gigs, hiring
    let is_jobs = Regex::new(
        r"(?i)\b(job|jobs|hiring|position|positions|opening|openings|gig|gigs|vacancy|vacancies|opportunity|opportunities|looking for work|seeking work|casting call|audition|auditions)\b"
    ).unwrap().is_match(&q);

    let any_specific = is_locations || is_orgs || is_productions || is_jobs;

    if any_specific {
        SearchIntent {
            people: is_productions, // include people for "films directed by chris"
            organizations: is_orgs,
            locations: is_locations,
            productions: is_productions,
            jobs: is_jobs,
        }
    } else {
        // No specific entity keyword — search everything
        SearchIntent {
            people: true,
            organizations: true,
            locations: true,
            productions: true,
            jobs: true,
        }
    }
}
