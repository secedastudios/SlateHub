//! Canonical search service — single source of truth for all search queries.
//!
//! Both the web routes and the MCP server delegate to these functions.
//! Every function follows the same layered pattern:
//!   1. Hard structural filters (location, skill, status, physical attributes) as WHERE clauses
//!   2. Soft semantic gate: text CONTAINS or vector similarity above threshold
//!   3. Scoring: weighted text match + vector similarity
//!
//! All user values flow through `$`-prefixed bind parameters — never `format!()`.
//! All `id` fields are cast via `<string> id AS id` to avoid RecordId deserialization issues.
//! Results are deserialized as `serde_json::Value` to sidestep SurrealValue derive limitations.

use serde::Deserialize;
use tracing::error;

use crate::config::SearchWeights;
use crate::db::DB;
use crate::error::{Error, Result};
use crate::services::search_utils::ParsedQuery;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct PersonSearchResult {
    pub id: String,
    pub name: String,
    pub username: String,
    pub headline: Option<String>,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub skills: Vec<String>,
    pub avatar_url: Option<String>,
    pub embedding_text: Option<String>,
    pub verification_status: String,
    pub score: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrganizationSearchResult {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub logo: Option<String>,
    pub embedding_text: Option<String>,
    pub verified: bool,
    pub score: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocationSearchResult {
    pub id: String,
    pub key: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub description: Option<String>,
    pub profile_photo: Option<String>,
    pub embedding_text: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProductionSearchResult {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub poster_url: Option<String>,
    pub poster_photo: Option<String>,
    pub embedding_text: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobSearchResult {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub poster_name: String,
    pub poster_type: String,
    pub role_count: i64,
    pub embedding_text: Option<String>,
    pub score: f64,
}

// ---------------------------------------------------------------------------
// SearchParams — shared input for every search function
// ---------------------------------------------------------------------------

pub struct SearchParams<'a> {
    pub query: &'a str,
    pub embedding: Option<&'a Vec<f32>>,
    pub weights: &'a SearchWeights,
    pub limit: usize,
    pub offset: usize,
}

// ---------------------------------------------------------------------------
// People
// ---------------------------------------------------------------------------

pub async fn search_people(
    params: &SearchParams<'_>,
    parsed: &ParsedQuery,
    skill: Option<&str>,
) -> Result<Vec<PersonSearchResult>> {
    let query_lower = parsed.cleaned.to_lowercase();
    let empty_emb: Vec<f32> = vec![];
    let w = params.weights;

    // --- hard filter clauses (structural, use bind params) ---
    let mut hard_parts: Vec<String> = Vec::new();

    if parsed.location.is_some() {
        hard_parts.push(
            "(string::lowercase(profile.location ?? '') CONTAINS string::lowercase($location_filter) \
             OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase($location_filter))"
                .to_string(),
        );
    }

    if skill.is_some() {
        hard_parts.push(
            "(string::lowercase(profile.headline ?? '') CONTAINS string::lowercase($skill_filter) \
             OR string::lowercase($skill_filter) INSIDE array::map(profile.skills ?? [], |$v| string::lowercase($v)))"
                .to_string(),
        );
    }

    if parsed.gender.is_some() {
        hard_parts.push(
            "string::lowercase(profile.gender ?? '') = string::lowercase($gender_filter)"
                .to_string(),
        );
    }

    if parsed.age_min.is_some() && parsed.age_max.is_some() {
        hard_parts.push(
            "profile.acting_age_range.min <= $age_max AND profile.acting_age_range.max >= $age_min"
                .to_string(),
        );
    }

    if parsed.hair_color.is_some() {
        hard_parts.push(
            "string::lowercase(profile.hair_color ?? '') = string::lowercase($hair_filter)"
                .to_string(),
        );
    }

    if parsed.eye_color.is_some() {
        hard_parts.push(
            "string::lowercase(profile.eye_color ?? '') = string::lowercase($eye_filter)"
                .to_string(),
        );
    }

    if parsed.body_type.is_some() {
        hard_parts.push(
            "string::lowercase(profile.body_type ?? '') = string::lowercase($body_filter)"
                .to_string(),
        );
    }

    let has_hard_filters = !hard_parts.is_empty();
    let hard_filter = if has_hard_filters {
        format!("AND {}", hard_parts.join(" AND "))
    } else {
        String::new()
    };

    // Text/vector gate — skipped only when hard filters exist AND cleaned query is empty
    let text_vector_gate = if has_hard_filters && query_lower.trim().is_empty() {
        "true".to_string()
    } else {
        format!(
            "(
                string::lowercase(name ?? '') CONTAINS $query_lower
                OR string::lowercase(username ?? '') CONTAINS $query_lower
                OR string::lowercase(profile.name ?? '') CONTAINS $query_lower
                OR string::lowercase(profile.headline ?? '') CONTAINS $query_lower
                OR string::lowercase(profile.bio ?? '') CONTAINS $query_lower
                OR string::lowercase(profile.location ?? '') CONTAINS $query_lower
                OR string::lowercase(embedding_text ?? '') CONTAINS $query_lower
                OR string::lowercase(string::join(', ', profile.skills ?? [])) CONTAINS $query_lower
                OR string::lowercase(string::join(', ', profile.languages ?? [])) CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
            )",
            threshold = w.vector_threshold,
        )
    };

    let sql = format!(
        "SELECT
            <string> id AS id,
            name,
            username,
            profile.headline AS headline,
            profile.bio AS bio,
            profile.location AS location,
            profile.skills AS skills,
            profile.avatar AS avatar_url,
            embedding_text,
            verification_status ?? 'none' AS verification_status,
            <float> (
                (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                + (IF string::lowercase(username ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                + (IF string::lowercase(profile.headline ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                + (IF string::lowercase(profile.bio ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                + (IF string::lowercase(profile.location ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                + (IF embedding IS NOT NONE AND $has_embedding = true
                    THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                    ELSE 0
                END)
            ) AS score
        FROM person
        WHERE verification_status != 'unverified'
            AND {text_vector_gate}
            {hard_filter}
        ORDER BY score DESC
        LIMIT $limit
        START $offset",
        w_name = w.name_match,
        w_headline = w.headline_match,
        w_location = w.location_match,
        w_vector = w.vector_multiplier,
    );

    let has_embedding = params.embedding.is_some();
    let embedding_vec = params.embedding.cloned().unwrap_or(empty_emb);

    let mut response = DB
        .query(&sql)
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", has_embedding))
        .bind(("query_embedding", embedding_vec))
        .bind(("limit", params.limit as i64))
        .bind(("offset", params.offset as i64))
        .bind((
            "location_filter",
            parsed.location.clone().unwrap_or_default(),
        ))
        .bind(("skill_filter", skill.unwrap_or("").to_string()))
        .bind(("gender_filter", parsed.gender.clone().unwrap_or_default()))
        .bind(("age_min", parsed.age_min.unwrap_or(0)))
        .bind(("age_max", parsed.age_max.unwrap_or(0)))
        .bind(("hair_filter", parsed.hair_color.clone().unwrap_or_default()))
        .bind(("eye_filter", parsed.eye_color.clone().unwrap_or_default()))
        .bind(("body_filter", parsed.body_type.clone().unwrap_or_default()))
        .await
        .map_err(|e| {
            error!(error = %e, table = "person", "Search query failed");
            Error::Database(e.to_string())
        })?;

    let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| {
        error!(error = %e, table = "person", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let results = rows
        .into_iter()
        .filter(|r| r["score"].as_f64().unwrap_or(0.0) > 0.0)
        .map(|r| PersonSearchResult {
            id: json_str(&r, "id"),
            name: json_str(&r, "name"),
            username: json_str(&r, "username"),
            headline: json_opt_str(&r, "headline"),
            bio: json_opt_str(&r, "bio"),
            location: json_opt_str(&r, "location"),
            skills: r["skills"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            avatar_url: json_opt_str(&r, "avatar_url"),
            embedding_text: json_opt_str(&r, "embedding_text"),
            verification_status: json_str_or(&r, "verification_status", "none"),
            score: r["score"].as_f64().unwrap_or(0.0),
        })
        .collect();

    Ok(results)
}

// ---------------------------------------------------------------------------
// Organizations
// ---------------------------------------------------------------------------

pub async fn search_organizations(
    params: &SearchParams<'_>,
    location: Option<&str>,
) -> Result<Vec<OrganizationSearchResult>> {
    let query_lower = params.query.to_lowercase();
    let empty_emb: Vec<f32> = vec![];
    let w = params.weights;

    let has_location = location.is_some();
    let hard_filter = if has_location {
        "AND (string::lowercase(location ?? '') CONTAINS string::lowercase($location_filter) \
         OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase($location_filter))"
            .to_string()
    } else {
        String::new()
    };

    let text_vector_gate = if has_location && query_lower.trim().is_empty() {
        "true".to_string()
    } else {
        format!(
            "(
                string::lowercase(name ?? '') CONTAINS $query_lower
                OR string::lowercase(slug ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR string::lowercase(location ?? '') CONTAINS $query_lower
                OR string::lowercase(embedding_text ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
            )",
            threshold = w.vector_threshold,
        )
    };

    let sql = format!(
        "SELECT
            <string> id AS id,
            name,
            slug,
            description,
            location,
            logo,
            embedding_text,
            (verified ?? false) AS verified,
            <float> (
                (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                + (IF string::lowercase(slug ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                + (IF embedding IS NOT NONE AND $has_embedding = true
                    THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                    ELSE 0
                END)
            ) AS score
        FROM organization
        WHERE
            {text_vector_gate}
            {hard_filter}
        ORDER BY score DESC
        LIMIT $limit
        START $offset",
        w_name = w.name_match,
        w_headline = w.headline_match,
        w_location = w.location_match,
        w_vector = w.vector_multiplier,
    );

    let has_embedding = params.embedding.is_some();
    let embedding_vec = params.embedding.cloned().unwrap_or(empty_emb);

    let mut response = DB
        .query(&sql)
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", has_embedding))
        .bind(("query_embedding", embedding_vec))
        .bind(("limit", params.limit as i64))
        .bind(("offset", params.offset as i64))
        .bind(("location_filter", location.unwrap_or("").to_string()))
        .await
        .map_err(|e| {
            error!(error = %e, table = "organization", "Search query failed");
            Error::Database(e.to_string())
        })?;

    let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| {
        error!(error = %e, table = "organization", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let results = rows
        .into_iter()
        .filter(|r| r["score"].as_f64().unwrap_or(0.0) > 0.0)
        .map(|r| OrganizationSearchResult {
            id: json_str(&r, "id"),
            name: json_str(&r, "name"),
            slug: json_str(&r, "slug"),
            description: json_opt_str(&r, "description"),
            location: json_opt_str(&r, "location"),
            logo: json_opt_str(&r, "logo"),
            embedding_text: json_opt_str(&r, "embedding_text"),
            verified: r["verified"].as_bool().unwrap_or(false),
            score: r["score"].as_f64().unwrap_or(0.0),
        })
        .collect();

    Ok(results)
}

// ---------------------------------------------------------------------------
// Locations
// ---------------------------------------------------------------------------

pub async fn search_locations(
    params: &SearchParams<'_>,
    city: Option<&str>,
    state: Option<&str>,
) -> Result<Vec<LocationSearchResult>> {
    let query_lower = params.query.to_lowercase();
    let empty_emb: Vec<f32> = vec![];
    let w = params.weights;

    let mut hard_parts: Vec<String> = Vec::new();

    if city.is_some() {
        hard_parts.push(
            "string::lowercase(city ?? '') CONTAINS string::lowercase($city_filter)".to_string(),
        );
    }

    if state.is_some() {
        hard_parts.push(
            "string::lowercase(state ?? '') CONTAINS string::lowercase($state_filter)".to_string(),
        );
    }

    let has_hard_filters = !hard_parts.is_empty();
    let hard_filter = if has_hard_filters {
        format!("AND {}", hard_parts.join(" AND "))
    } else {
        String::new()
    };

    let text_vector_gate = if has_hard_filters && query_lower.trim().is_empty() {
        "true".to_string()
    } else {
        format!(
            "(
                string::lowercase(name ?? '') CONTAINS $query_lower
                OR string::lowercase(city ?? '') CONTAINS $query_lower
                OR string::lowercase(state ?? '') CONTAINS $query_lower
                OR string::lowercase(address ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR string::lowercase(embedding_text ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
            )",
            threshold = w.vector_threshold,
        )
    };

    let sql = format!(
        "SELECT
            <string> id AS id,
            <string> meta::id(id) AS key,
            name,
            address,
            city,
            state,
            description,
            profile_photo,
            embedding_text,
            <float> (
                (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                + (IF string::lowercase(city ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                + (IF string::lowercase(state ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                + (IF string::lowercase(address ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                + (IF embedding IS NOT NONE AND $has_embedding = true
                    THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                    ELSE 0
                END)
            ) AS score
        FROM location
        WHERE is_public = true AND {text_vector_gate}
        {hard_filter}
        ORDER BY score DESC
        LIMIT $limit
        START $offset",
        w_name = w.name_match,
        w_headline = w.headline_match,
        w_location = w.location_match,
        w_vector = w.vector_multiplier,
    );

    let has_embedding = params.embedding.is_some();
    let embedding_vec = params.embedding.cloned().unwrap_or(empty_emb);

    let mut response = DB
        .query(&sql)
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", has_embedding))
        .bind(("query_embedding", embedding_vec))
        .bind(("limit", params.limit as i64))
        .bind(("offset", params.offset as i64))
        .bind(("city_filter", city.unwrap_or("").to_string()))
        .bind(("state_filter", state.unwrap_or("").to_string()))
        .await
        .map_err(|e| {
            error!(error = %e, table = "location", "Search query failed");
            Error::Database(e.to_string())
        })?;

    let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| {
        error!(error = %e, table = "location", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let results = rows
        .into_iter()
        .filter(|r| r["score"].as_f64().unwrap_or(0.0) > 0.0)
        .map(|r| LocationSearchResult {
            id: json_str(&r, "id"),
            key: json_str(&r, "key"),
            name: json_str(&r, "name"),
            address: json_str(&r, "address"),
            city: json_str(&r, "city"),
            state: json_str(&r, "state"),
            description: json_opt_str(&r, "description"),
            profile_photo: json_opt_str(&r, "profile_photo"),
            embedding_text: json_opt_str(&r, "embedding_text"),
            score: r["score"].as_f64().unwrap_or(0.0),
        })
        .collect();

    Ok(results)
}

// ---------------------------------------------------------------------------
// Productions
// ---------------------------------------------------------------------------

pub async fn search_productions(
    params: &SearchParams<'_>,
    status: Option<&str>,
) -> Result<Vec<ProductionSearchResult>> {
    let query_lower = params.query.to_lowercase();
    let empty_emb: Vec<f32> = vec![];
    let w = params.weights;

    let has_status = status.is_some();
    let hard_filter = if has_status {
        "AND string::lowercase(status ?? '') = string::lowercase($status_filter)".to_string()
    } else {
        String::new()
    };

    let text_vector_gate = if has_status && query_lower.trim().is_empty() {
        "true".to_string()
    } else {
        format!(
            "(
                string::lowercase(title ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR string::lowercase(location ?? '') CONTAINS $query_lower
                OR string::lowercase(embedding_text ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
            )",
            threshold = w.vector_threshold,
        )
    };

    let sql = format!(
        "SELECT
            <string> id AS id,
            title,
            slug,
            status,
            description,
            location,
            poster_url,
            poster_photo,
            embedding_text,
            <float> (
                (IF string::lowercase(title ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                + (IF embedding IS NOT NONE AND $has_embedding = true
                    THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                    ELSE 0
                END)
            ) AS score
        FROM production
        WHERE
            {text_vector_gate}
            {hard_filter}
        ORDER BY score DESC
        LIMIT $limit
        START $offset",
        w_name = w.name_match,
        w_headline = w.headline_match,
        w_location = w.location_match,
        w_vector = w.vector_multiplier,
    );

    let has_embedding = params.embedding.is_some();
    let embedding_vec = params.embedding.cloned().unwrap_or(empty_emb);

    let mut response = DB
        .query(&sql)
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", has_embedding))
        .bind(("query_embedding", embedding_vec))
        .bind(("limit", params.limit as i64))
        .bind(("offset", params.offset as i64))
        .bind(("status_filter", status.unwrap_or("").to_string()))
        .await
        .map_err(|e| {
            error!(error = %e, table = "production", "Search query failed");
            Error::Database(e.to_string())
        })?;

    let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| {
        error!(error = %e, table = "production", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let results = rows
        .into_iter()
        .filter(|r| r["score"].as_f64().unwrap_or(0.0) > 0.0)
        .map(|r| ProductionSearchResult {
            id: json_str(&r, "id"),
            title: json_str(&r, "title"),
            slug: json_str(&r, "slug"),
            status: json_str(&r, "status"),
            description: json_opt_str(&r, "description"),
            location: json_opt_str(&r, "location"),
            poster_url: json_opt_str(&r, "poster_url"),
            poster_photo: json_opt_str(&r, "poster_photo"),
            embedding_text: json_opt_str(&r, "embedding_text"),
            score: r["score"].as_f64().unwrap_or(0.0),
        })
        .collect();

    Ok(results)
}

// ---------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------

pub async fn search_jobs(
    params: &SearchParams<'_>,
    location: Option<&str>,
    open_only: bool,
) -> Result<Vec<JobSearchResult>> {
    let query_lower = params.query.to_lowercase();
    let empty_emb: Vec<f32> = vec![];
    let w = params.weights;

    let mut hard_parts: Vec<String> = Vec::new();

    if open_only {
        hard_parts.push("status = 'open' AND expires_at > time::now()".to_string());
    }

    if location.is_some() {
        hard_parts.push(
            "(string::lowercase(location ?? '') CONTAINS string::lowercase($location_filter) \
             OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase($location_filter))"
                .to_string(),
        );
    }

    let has_hard_filters = !hard_parts.is_empty();
    let hard_filter = if has_hard_filters {
        format!("AND {}", hard_parts.join(" AND "))
    } else {
        String::new()
    };

    let text_vector_gate = if has_hard_filters && query_lower.trim().is_empty() {
        "true".to_string()
    } else {
        format!(
            "(
                string::lowercase(title ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR string::lowercase(location ?? '') CONTAINS $query_lower
                OR string::lowercase(string::join(' ', roles.*.title)) CONTAINS $query_lower
                OR string::lowercase(embedding_text ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
            )",
            threshold = w.vector_threshold,
        )
    };

    let sql = format!(
        "SELECT
            <string> id AS id,
            title,
            description,
            location,
            <string> posted_by AS posted_by_id,
            array::len(roles) AS role_count,
            embedding_text,
            <float> (
                (IF string::lowercase(title ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                + (IF embedding IS NOT NONE AND $has_embedding = true
                    THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                    ELSE 0
                END)
            ) AS score
        FROM job_posting
        WHERE
            {text_vector_gate}
            {hard_filter}
        ORDER BY score DESC
        LIMIT $limit
        START $offset",
        w_name = w.name_match,
        w_headline = w.headline_match,
        w_location = w.location_match,
        w_vector = w.vector_multiplier,
    );

    let has_embedding = params.embedding.is_some();
    let embedding_vec = params.embedding.cloned().unwrap_or(empty_emb);

    let mut response = DB
        .query(&sql)
        .bind(("query_lower", query_lower))
        .bind(("has_embedding", has_embedding))
        .bind(("query_embedding", embedding_vec))
        .bind(("limit", params.limit as i64))
        .bind(("offset", params.offset as i64))
        .bind(("location_filter", location.unwrap_or("").to_string()))
        .await
        .map_err(|e| {
            error!(error = %e, table = "job_posting", "Search query failed");
            Error::Database(e.to_string())
        })?;

    let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| {
        error!(error = %e, table = "job_posting", "Failed to deserialize search results");
        Error::Database(e.to_string())
    })?;

    let mut results = Vec::new();
    for r in &rows {
        let score = r["score"].as_f64().unwrap_or(0.0);
        if score <= 0.0 {
            continue;
        }

        // Resolve poster name from posted_by (person or organization)
        let posted_by_id = r["posted_by_id"].as_str().unwrap_or("");
        let (poster_name, poster_type) = resolve_poster(posted_by_id).await;

        results.push(JobSearchResult {
            id: json_str(r, "id"),
            title: json_str(r, "title"),
            description: json_str(r, "description"),
            location: json_opt_str(r, "location"),
            poster_name,
            poster_type,
            role_count: r["role_count"].as_i64().unwrap_or(0),
            embedding_text: json_opt_str(r, "embedding_text"),
            score,
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a poster's display name from a `posted_by` record ID string.
/// Returns `(name, entity_type)` — e.g. `("Jane Doe", "person")`.
async fn resolve_poster(posted_by_id: &str) -> (String, String) {
    if posted_by_id.is_empty() {
        return (String::new(), String::new());
    }

    let (entity_type, rid) = if posted_by_id.starts_with("person:") {
        (
            "person",
            surrealdb::types::RecordId::parse_simple(posted_by_id).ok(),
        )
    } else if posted_by_id.starts_with("organization:") {
        (
            "organization",
            surrealdb::types::RecordId::parse_simple(posted_by_id).ok(),
        )
    } else {
        return (String::new(), String::new());
    };

    let name: Option<String> = match rid {
        Some(id) => DB
            .query("SELECT VALUE name FROM $id")
            .bind(("id", id))
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok())
            .flatten(),
        None => None,
    };

    (name.unwrap_or_default(), entity_type.to_string())
}

/// Extract a required string field from a JSON value, defaulting to empty.
fn json_str(v: &serde_json::Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}

/// Extract an optional string field — returns `None` for null / missing / empty.
fn json_opt_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v[key].as_str().filter(|s| !s.is_empty()).map(String::from)
}

/// Extract a string field with a custom default.
fn json_str_or(v: &serde_json::Value, key: &str, default: &str) -> String {
    v[key].as_str().unwrap_or(default).to_string()
}
