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
    /// True for "remember me" sessions — these slide: the auth middleware
    /// re-issues the token on activity (see [`should_refresh_session`]).
    /// Defaults to false so tokens minted before this claim existed decode.
    #[serde(default)]
    pub remember: bool,
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

/// Hash a password using SurrealDB-compatible Argon2id settings.
///
/// Argon2id at these parameters costs ~50–100 ms of pure CPU, so this async
/// variant runs the work on tokio's blocking pool — always prefer it inside
/// handlers and async model code. Use [`hash_password_sync`] only from
/// genuinely synchronous contexts (tests, CLI tools).
pub async fn hash_password(password: &str) -> Result<String> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || hash_password_sync(&password))
        .await
        .map_err(|e| Error::Internal(format!("hash task join error: {e}")))?
}

/// Synchronous [`hash_password`]. Blocks the calling thread for the full
/// Argon2id derivation — never call directly from async code.
pub fn hash_password_sync(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = PasswordConfig::argon2();

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| Error::Internal(format!("Failed to hash password: {}", e)))?;

    Ok(password_hash.to_string())
}

/// Verify a password against a SurrealDB-compatible Argon2id hash.
///
/// Verification re-runs the full Argon2id derivation (~50–100 ms CPU), so
/// this async variant uses the blocking pool — prefer it everywhere async.
/// Returns `Ok(false)` on mismatch; `Err` only for malformed hashes.
pub async fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let (password, hash) = (password.to_owned(), hash.to_owned());
    tokio::task::spawn_blocking(move || verify_password_sync(&password, &hash))
        .await
        .map_err(|e| Error::Internal(format!("verify task join error: {e}")))?
}

/// Synchronous [`verify_password`]. Blocks for the full derivation — never
/// call directly from async code.
pub fn verify_password_sync(password: &str, hash: &str) -> Result<bool> {
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
    /// The signing secret from `JWT_SECRET`.
    ///
    /// # Errors
    /// Returns [`Error::Internal`] when the variable is unset — surfacing a
    /// clean 500 instead of panicking the handler task on a misconfigured
    /// deployment.
    pub fn secret() -> Result<String> {
        std::env::var("JWT_SECRET")
            .map_err(|_| Error::Internal("JWT_SECRET environment variable must be set".into()))
    }

    /// Token validity duration in seconds (12 hours by default)
    pub fn token_duration() -> u64 {
        std::env::var("JWT_DURATION")
            .unwrap_or_else(|_| "43200".to_string())
            .parse()
            .unwrap_or(43200)
    }

    /// "Remember me" token validity duration in seconds (30 days by default)
    pub fn remember_duration() -> u64 {
        std::env::var("JWT_REMEMBER_DURATION")
            .unwrap_or_else(|_| "2592000".to_string())
            .parse()
            .unwrap_or(2_592_000)
    }
}

/// Session length in seconds for a login: the standard 12-hour token, or the
/// 30-day "remember me" token when the login form's checkbox was ticked. The
/// same value sizes both the JWT `exp` claim and the cookie `Max-Age`, so the
/// two never disagree.
pub fn session_duration(remember: bool) -> u64 {
    if remember {
        JwtConfig::remember_duration()
    } else {
        JwtConfig::token_duration()
    }
}

/// How old (in seconds) a remembered token must be before the auth middleware
/// re-issues it. Throttles the sliding refresh to roughly once a day instead
/// of minting a token on every request.
pub const SESSION_REFRESH_AFTER_SECS: u64 = 86_400;

/// Whether the auth middleware should re-issue this session's token now:
/// only "remember me" sessions slide, and only once the token is at least
/// [`SESSION_REFRESH_AFTER_SECS`] old. Pure so the policy is unit-testable.
pub fn should_refresh_session(claims: &Claims, now: u64) -> bool {
    claims.remember && now.saturating_sub(claims.iat) >= SESSION_REFRESH_AFTER_SECS
}

/// Create a JWT token for a user with the standard 12-hour validity
pub fn create_jwt(user_id: &str, username: &str, email: &str) -> Result<String> {
    build_jwt(user_id, username, email, JwtConfig::token_duration(), false)
}

/// Create a login-session JWT: 12 hours, or 30 days with the `remember` claim
/// set when the user ticked "remember me" (remembered tokens are re-issued on
/// activity by the auth middleware).
pub fn create_session_jwt(
    user_id: &str,
    username: &str,
    email: &str,
    remember: bool,
) -> Result<String> {
    build_jwt(
        user_id,
        username,
        email,
        session_duration(remember),
        remember,
    )
}

/// Create a JWT token for a user that expires `duration_secs` from now
/// (non-remembered; the session does not slide).
pub fn create_jwt_with_duration(
    user_id: &str,
    username: &str,
    email: &str,
    duration_secs: u64,
) -> Result<String> {
    build_jwt(user_id, username, email, duration_secs, false)
}

fn build_jwt(
    user_id: &str,
    username: &str,
    email: &str,
    duration_secs: u64,
    remember: bool,
) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Internal(format!("System time error: {}", e)))?
        .as_secs();

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        email: email.to_string(),
        iat: now,
        exp: now + duration_secs,
        remember,
    };

    let header = Header::new(JwtAlgorithm::HS256);
    let secret = JwtConfig::secret()?;

    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| Error::Internal(format!("Failed to create JWT: {}", e)))
}

/// Decode and validate a JWT token
pub fn decode_jwt(token: &str) -> Result<Claims> {
    let secret = JwtConfig::secret()?;
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
