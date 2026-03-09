use reqwest;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::LazyLock;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum TmdbError {
    #[error("TMDB API request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("TMDB API key not configured")]
    NotConfigured,
    #[error("TMDB API returned error: {0}")]
    ApiError(String),
}

type Result<T> = std::result::Result<T, TmdbError>;

static TMDB_SERVICE: LazyLock<Option<TmdbService>> = LazyLock::new(|| {
    match TmdbService::from_env() {
        Ok(svc) => {
            info!("TMDB service initialized");
            Some(svc)
        }
        Err(e) => {
            warn!("TMDB service not available: {}", e);
            None
        }
    }
});

/// Get the global TMDB service instance, if configured.
pub fn get_service() -> Result<&'static TmdbService> {
    TMDB_SERVICE.as_ref().ok_or(TmdbError::NotConfigured)
}

#[derive(Debug, Clone)]
pub struct TmdbService {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

// --- TMDB API response types ---

#[derive(Debug, Deserialize)]
pub struct TmdbSearchResponse {
    pub results: Vec<TmdbPersonResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TmdbPersonResult {
    pub id: i64,
    pub name: String,
    pub known_for_department: Option<String>,
    pub profile_path: Option<String>,
    pub popularity: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbCombinedCreditsResponse {
    pub cast: Vec<TmdbCreditEntry>,
    pub crew: Vec<TmdbCreditEntry>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbCreditEntry {
    pub id: i64,
    pub media_type: Option<String>,
    // Movie fields
    pub title: Option<String>,
    pub release_date: Option<String>,
    // TV fields
    pub name: Option<String>,
    pub first_air_date: Option<String>,
    // Shared fields
    pub character: Option<String>,
    pub job: Option<String>,
    pub department: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub vote_average: Option<f64>,
    pub popularity: Option<f64>,
}

// --- Serializable output types for our API ---

#[derive(Debug, Serialize)]
pub struct TmdbCredit {
    pub tmdb_id: i64,
    pub title: String,
    pub role: String,
    pub department: Option<String>,
    pub overview: Option<String>,
    pub poster_url: Option<String>,
    pub tmdb_url: String,
    pub release_date: Option<String>,
    pub media_type: String,
    pub vote_average: Option<f64>,
}

impl TmdbService {
    fn from_env() -> Result<Self> {
        let api_key = env::var("TMDB_API_KEY")
            .map_err(|_| TmdbError::NotConfigured)?;

        if api_key.is_empty() {
            return Err(TmdbError::NotConfigured);
        }

        Ok(Self {
            api_key,
            base_url: "https://api.themoviedb.org/3".to_string(),
            client: reqwest::Client::new(),
        })
    }

    fn poster_url(&self, poster_path: &str) -> String {
        format!("https://image.tmdb.org/t/p/w342{}", poster_path)
    }

    fn tmdb_page_url(&self, media_type: &str, id: i64) -> String {
        format!("https://www.themoviedb.org/{}/{}", media_type, id)
    }

    /// Search TMDB for people by name.
    pub async fn search_person(&self, query: &str) -> Result<Vec<TmdbPersonResult>> {
        debug!("Searching TMDB for person: {}", query);

        let resp: TmdbSearchResponse = self
            .client
            .get(format!("{}/search/person", self.base_url))
            .query(&[("api_key", &self.api_key), ("query", &query.to_string())])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| TmdbError::ApiError(e.to_string()))?
            .json()
            .await?;

        Ok(resp.results)
    }

    /// Fetch combined (movie + TV) credits for a TMDB person.
    pub async fn get_person_credits(&self, person_id: i64) -> Result<Vec<TmdbCredit>> {
        debug!("Fetching TMDB credits for person_id: {}", person_id);

        let resp: TmdbCombinedCreditsResponse = self
            .client
            .get(format!("{}/person/{}/combined_credits", self.base_url, person_id))
            .query(&[("api_key", &self.api_key)])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| TmdbError::ApiError(e.to_string()))?
            .json()
            .await?;

        let mut credits: Vec<TmdbCredit> = Vec::new();

        // Process cast credits
        for entry in &resp.cast {
            if let Some(credit) = self.entry_to_credit(entry, true) {
                credits.push(credit);
            }
        }

        // Process crew credits
        for entry in &resp.crew {
            if let Some(credit) = self.entry_to_credit(entry, false) {
                credits.push(credit);
            }
        }

        // Sort by release date descending (most recent first)
        credits.sort_by(|a, b| b.release_date.cmp(&a.release_date));

        // Deduplicate by tmdb_id + role (same person can be cast AND crew on same production)
        credits.dedup_by(|a, b| a.tmdb_id == b.tmdb_id && a.role == b.role);

        Ok(credits)
    }

    fn entry_to_credit(&self, entry: &TmdbCreditEntry, is_cast: bool) -> Option<TmdbCredit> {
        let media_type = entry.media_type.as_deref().unwrap_or("movie");
        let title = entry
            .title
            .as_ref()
            .or(entry.name.as_ref())?
            .clone();

        let role = if is_cast {
            entry.character.clone().unwrap_or_else(|| "Actor".to_string())
        } else {
            entry.job.clone().unwrap_or_else(|| {
                entry
                    .department
                    .clone()
                    .unwrap_or_else(|| "Crew".to_string())
            })
        };

        let release_date = entry
            .release_date
            .as_ref()
            .or(entry.first_air_date.as_ref())
            .cloned();

        Some(TmdbCredit {
            tmdb_id: entry.id,
            title,
            role,
            department: if is_cast { None } else { entry.department.clone() },
            overview: entry.overview.clone(),
            poster_url: entry.poster_path.as_ref().map(|p| self.poster_url(p)),
            tmdb_url: self.tmdb_page_url(media_type, entry.id),
            release_date,
            media_type: media_type.to_string(),
            vote_average: entry.vote_average,
        })
    }
}
