//! Authentication module for password hashing and JWT token management
//!
//! This module provides password hashing compatible with SurrealDB's format
//! and JWT token creation/validation for session management.

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use jsonwebtoken::{
    Algorithm as JwtAlgorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};

/// JWT Claims structure for our tokens
#[derive(Debug, Deserialize, Serialize)]
pub struct Claims {
    /// User ID (format: "person:xxxxx")
    pub sub: String,
    /// Username
    pub username: String,
    /// Email
    pub email: String,
    /// Issued at (Unix timestamp)
    pub iat: u64,
    /// Expiration (Unix timestamp)
    pub exp: u64,
}

/// Configuration for password hashing (matches SurrealDB's settings)
pub struct PasswordConfig;

impl PasswordConfig {
    /// Create Argon2 hasher with SurrealDB-compatible parameters
    pub fn argon2() -> Argon2<'static> {
        // Match SurrealDB's parameters: m=19456, t=2, p=1
        let params = Params::new(
            19456, // memory cost in KiB (19 MiB)
            2,     // time cost (iterations)
            1,     // parallelism
            None,  // output length (default)
        )
        .expect("Valid Argon2 parameters");

        Argon2::new(
            Algorithm::Argon2id, // SurrealDB uses Argon2id
            Version::V0x13,      // Version 19
            params,
        )
    }
}

/// Hash a password using SurrealDB-compatible Argon2id settings
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = PasswordConfig::argon2();

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| Error::Internal(format!("Failed to hash password: {}", e)))?;

    Ok(password_hash.to_string())
}

/// Verify a password against a SurrealDB-compatible Argon2id hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| Error::Internal(format!("Invalid password hash format: {}", e)))?;

    let argon2 = PasswordConfig::argon2();

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// JWT configuration
pub struct JwtConfig;

impl JwtConfig {
    /// Get the JWT secret from environment or use a default for development
    pub fn secret() -> String {
        std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            // In production, this should be a strong random secret
            tracing::warn!("JWT_SECRET not set, using default development secret");
            "development-secret-change-in-production".to_string()
        })
    }

    /// Token validity duration in seconds (12 hours by default)
    pub fn token_duration() -> u64 {
        std::env::var("JWT_DURATION")
            .unwrap_or_else(|_| "43200".to_string())
            .parse()
            .unwrap_or(43200)
    }
}

/// Create a JWT token for a user
pub fn create_jwt(user_id: &str, username: &str, email: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Internal(format!("System time error: {}", e)))?
        .as_secs();

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        email: email.to_string(),
        iat: now,
        exp: now + JwtConfig::token_duration(),
    };

    let header = Header::new(JwtAlgorithm::HS256);
    let secret = JwtConfig::secret();

    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| Error::Internal(format!("Failed to create JWT: {}", e)))
}

/// Decode and validate a JWT token
pub fn decode_jwt(token: &str) -> Result<Claims> {
    let secret = JwtConfig::secret();
    let validation = Validation::new(JwtAlgorithm::HS256);

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        tracing::debug!("JWT decode error: {}", e);
        Error::Unauthorized
    })?;

    Ok(token_data.claims)
}

/// Decode a JWT token without validation (for debugging or when signature validation isn't needed)
pub fn decode_jwt_insecure(token: &str) -> Result<Claims> {
    let mut validation = Validation::new(JwtAlgorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;

    let token_data =
        decode::<Claims>(token, &DecodingKey::from_secret(b""), &validation).map_err(|e| {
            tracing::debug!("JWT decode error: {}", e);
            Error::Unauthorized
        })?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "test_password_123";
        let hash = hash_password(password).expect("Should hash password");

        // Verify the hash format matches SurrealDB's format
        assert!(hash.starts_with("$argon2id$"));
        assert!(hash.contains("$m=19456,t=2,p=1$"));

        // Verify the password
        assert!(verify_password(password, &hash).expect("Should verify password"));
        assert!(!verify_password("wrong_password", &hash).expect("Should verify password"));
    }

    #[test]
    fn test_jwt_creation_and_validation() {
        let user_id = "person:test123";
        let username = "testuser";
        let email = "test@example.com";

        let token = create_jwt(user_id, username, email).expect("Should create JWT");
        assert!(!token.is_empty());

        let claims = decode_jwt(&token).expect("Should decode JWT");
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.username, username);
        assert_eq!(claims.email, email);
    }
}
