use serde::Deserialize;
use std::env;
use std::net::SocketAddr;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub namespace: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
}

impl Config {
    /// Load configuration from environment variables
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

    /// Get the database connection URL
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

/// Search scoring weights — configurable via env vars.
#[derive(Debug, Clone)]
pub struct SearchWeights {
    pub name_match: i32,
    pub headline_match: i32,
    pub location_match: i32,
    pub vector_multiplier: i32,
    pub vector_threshold: f64,
}

impl SearchWeights {
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

    /// Get the server socket address
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
