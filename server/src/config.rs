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
        if let Ok(url) = env::var("DATABASE_URL") {
            if !url.is_empty() {
                return url;
            }
        }

        // Otherwise construct it from individual components
        format!("{}:{}", self.host, self.port)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_connection_url() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 8000,
            username: "root".to_string(),
            password: "root".to_string(),
            namespace: "test".to_string(),
            name: "testdb".to_string(),
        };

        assert_eq!(config.connection_url(), "ws://localhost:8000");
    }

    #[test]
    fn test_server_socket_addr() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        };

        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:3000");
    }
}
