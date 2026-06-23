//! Environment-driven application configuration.
//!
//! `main.rs` calls [`Config::from_env`] at startup (loading a `.env` file if
//! present) to obtain the SurrealDB connection settings and the HTTP listener
//! address. The module also exposes [`app_url`] — the canonical base URL used
//! wherever absolute links are built (templates, verification routes, MCP) —
//! and the lazily-loaded [`SearchWeights`] consumed by the model search
//! queries and the MCP server's search tools.

use serde::Deserialize;
use std::env;
use std::net::SocketAddr;
use thiserror::Error;

/// Top-level application configuration, assembled from environment variables
/// by [`Config::from_env`].
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
}

/// SurrealDB connection settings, read from the `DB_*` environment variables.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub namespace: String,
    pub name: String,
}

/// HTTP listener settings, read from `SERVER_HOST` and `SERVER_PORT`.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

/// Errors produced when configuration is missing or malformed.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
}

impl Config {
    /// Loads the full configuration from environment variables, reading a
    /// `.env` file first if one exists.
    ///
    /// # Errors
    /// Returns [`ConfigError::MissingEnvVar`] when the database credentials
    /// (`DB_USERNAME`/`DB_USER`, `DB_PASSWORD`/`DB_PASS`) are absent, or
    /// [`ConfigError::InvalidValue`] when a port fails to parse.
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if it exists (safe to call multiple times)
        dotenv::dotenv().ok();

        Ok(Config {
            database: DatabaseConfig::from_env()?,
            server: ServerConfig::from_env()?,
        })
    }
}

impl DatabaseConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(DatabaseConfig {
            host: env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("DB_PORT")
                .unwrap_or_else(|_| "8000".to_string())
                .parse()
                .map_err(|_| {
                    ConfigError::InvalidValue(
                        "DB_PORT".to_string(),
                        "must be a valid port number".to_string(),
                    )
                })?,
            username: env::var("DB_USERNAME")
                .or_else(|_| env::var("DB_USER"))
                .map_err(|_| ConfigError::MissingEnvVar("DB_USERNAME or DB_USER".to_string()))?,
            password: env::var("DB_PASSWORD")
                .or_else(|_| env::var("DB_PASS"))
                .map_err(|_| ConfigError::MissingEnvVar("DB_PASSWORD or DB_PASS".to_string()))?,
            namespace: env::var("DB_NAMESPACE").unwrap_or_else(|_| "slatehub".to_string()),
            name: env::var("DB_NAME").unwrap_or_else(|_| "main".to_string()),
        })
    }

    /// Returns the database connection URL: `DATABASE_URL` verbatim when set
    /// and non-empty, otherwise `host:port` built from the `DB_*` values.
    pub fn connection_url(&self) -> String {
        // Check if DATABASE_URL is explicitly set
        if let Ok(url) = env::var("DATABASE_URL")
            && !url.is_empty()
        {
            return url;
        }

        // Otherwise construct it from individual components
        format!("{}:{}", self.host, self.port)
    }
}

/// Get the application base URL (e.g. "https://slatehub.com").
/// Reads from APP_URL env var, defaults to "http://localhost:3000".
/// Returned without a trailing slash.
pub fn app_url() -> String {
    env::var("APP_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// The Meta (Facebook) Pixel id used across the public conversion funnel
/// (the `/a/{campaign}` landing pages, `/signup`, and `/verify-email`).
///
/// Read solely from the `META_PIXEL_ID` environment variable — no id is baked
/// into the binary. Unset or empty → `None`, which omits the pixel snippet
/// entirely (the default for local dev / tests). Production sets the id in its
/// environment; see `.env.example`.
pub fn meta_pixel_id() -> Option<String> {
    env::var("META_PIXEL_ID")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Search scoring weights — configurable via env vars.
///
/// Consumed by the model search queries (people, jobs, organizations,
/// productions, locations) when composing relevance scores from text and
/// vector matches.
#[derive(Debug, Clone)]
pub struct SearchWeights {
    /// Score added when the query matches the record's name.
    pub name_match: i32,
    /// Score added when the query matches the headline.
    pub headline_match: i32,
    /// Score added when the query matches the location.
    pub location_match: i32,
    /// Multiplier applied to the cosine-similarity score of a vector match.
    pub vector_multiplier: i32,
    /// Minimum cosine similarity (0.0–1.0) for an embedding to count as a match.
    pub vector_threshold: f64,
}

impl SearchWeights {
    /// Builds weights from the `SEARCH_WEIGHT_NAME`, `SEARCH_WEIGHT_HEADLINE`,
    /// `SEARCH_WEIGHT_LOCATION`, `SEARCH_WEIGHT_VECTOR`, and
    /// `SEARCH_VECTOR_THRESHOLD` env vars, falling back to built-in defaults
    /// when a variable is unset or unparsable.
    pub fn from_env() -> Self {
        fn parse_or(var: &str, default: i32) -> i32 {
            env::var(var)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }
        fn parse_f64_or(var: &str, default: f64) -> f64 {
            env::var(var)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }
        Self {
            name_match: parse_or("SEARCH_WEIGHT_NAME", 50),
            headline_match: parse_or("SEARCH_WEIGHT_HEADLINE", 20),
            location_match: parse_or("SEARCH_WEIGHT_LOCATION", 10),
            vector_multiplier: parse_or("SEARCH_WEIGHT_VECTOR", 50),
            vector_threshold: parse_f64_or("SEARCH_VECTOR_THRESHOLD", 0.75),
        }
    }
}

/// Global search weights — loaded once from env at first access.
static SEARCH_WEIGHTS: std::sync::LazyLock<SearchWeights> = std::sync::LazyLock::new(|| {
    dotenv::dotenv().ok();
    SearchWeights::from_env()
});

/// Returns the process-wide search weights, loading them from the environment
/// on first access.
pub fn search_weights() -> &'static SearchWeights {
    &SEARCH_WEIGHTS
}

/// MCP-specific search weights — typically lower thresholds since the LLM filters results itself.
static MCP_SEARCH_WEIGHTS: std::sync::LazyLock<SearchWeights> = std::sync::LazyLock::new(|| {
    dotenv::dotenv().ok();
    fn parse_or(var: &str, default: i32) -> i32 {
        env::var(var)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
    fn parse_f64_or(var: &str, default: f64) -> f64 {
        env::var(var)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
    SearchWeights {
        name_match: parse_or("MCP_SEARCH_WEIGHT_NAME", 50),
        headline_match: parse_or("MCP_SEARCH_WEIGHT_HEADLINE", 20),
        location_match: parse_or("MCP_SEARCH_WEIGHT_LOCATION", 10),
        vector_multiplier: parse_or("MCP_SEARCH_WEIGHT_VECTOR", 50),
        vector_threshold: parse_f64_or("MCP_SEARCH_VECTOR_THRESHOLD", 0.55),
    }
});

/// Returns the search weights used by the MCP server tools, read from the
/// `MCP_SEARCH_WEIGHT_*` env vars on first access.
pub fn mcp_search_weights() -> &'static SearchWeights {
    &MCP_SEARCH_WEIGHTS
}

impl ServerConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(ServerConfig {
            host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|_| {
                    ConfigError::InvalidValue(
                        "SERVER_PORT".to_string(),
                        "must be a valid port number".to_string(),
                    )
                })?,
        })
    }

    /// Returns `host:port` parsed as a [`SocketAddr`] for binding the HTTP
    /// listener.
    ///
    /// # Errors
    /// Returns [`ConfigError::InvalidValue`] when the pair does not form a
    /// valid socket address (e.g. the host is a name rather than an IP).
    pub fn socket_addr(&self) -> Result<SocketAddr, ConfigError> {
        let addr = format!("{}:{}", self.host, self.port);
        addr.parse().map_err(|_| {
            ConfigError::InvalidValue(
                "SERVER_ADDRESS".to_string(),
                format!("invalid socket address: {}", addr),
            )
        })
    }
}
