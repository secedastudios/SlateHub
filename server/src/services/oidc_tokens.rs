//! Helpers for generating, hashing, and validating OIDC tokens
//! (authorization codes, access tokens, refresh tokens).

use crate::db::DB;
use crate::error::{Error, Result};
use crate::record_id_ext::RecordIdExt;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use surrealdb::types::{RecordId, SurrealValue};

/// Length of opaque random tokens (codes + access + refresh): 32 base32 chars ≈ 160 bits.
const TOKEN_LEN: usize = 32;

/// Lifetime of an authorization code (RFC 6749 §4.1.2 recommends ≤ 10 minutes).
pub const AUTHORIZATION_CODE_TTL_SECONDS: i64 = 60;
/// Lifetime of an access token.
pub const ACCESS_TOKEN_TTL_SECONDS: i64 = 3600;
/// Lifetime of a refresh token (absolute, not sliding).
pub const REFRESH_TOKEN_TTL_SECONDS: i64 = 60 * 60 * 24 * 30; // 30 days

pub fn random_opaque_token() -> String {
    const CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..TOKEN_LEN)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let bytes = hasher.finalize();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// PKCE S256 transform: BASE64URL-NOPAD(SHA256(verifier)) == challenge.
pub fn verify_pkce_s256(verifier: &str, challenge: &str) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    URL_SAFE_NO_PAD.encode(digest) == challenge
}

/// Generate a new opaque session id (associates one logical session of
/// access + refresh tokens for revocation propagation).
pub fn new_session_id() -> String {
    random_opaque_token()
}

// ----- Authorization code -----

#[derive(Debug, Clone, SurrealValue, Serialize, Deserialize)]
pub struct AuthorizationCodeRow {
    pub id: RecordId,
    pub code: String,
    pub client: RecordId,
    pub person: RecordId,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
    #[serde(default)]
    #[surreal(default)]
    pub nonce: Option<String>,
    pub session_id: String,
    pub expires_at: DateTime<Utc>,
    pub consumed: bool,
}

pub struct CreateAuthorizationCode<'a> {
    pub client: &'a RecordId,
    pub person: &'a RecordId,
    pub redirect_uri: &'a str,
    pub scopes: &'a [String],
    pub code_challenge: &'a str,
    pub code_challenge_method: &'a str,
    pub nonce: Option<&'a str>,
}

pub async fn create_authorization_code(
    args: CreateAuthorizationCode<'_>,
) -> Result<(String, String)> {
    let code = random_opaque_token();
    let session_id = new_session_id();
    let expires_at = Utc::now() + Duration::seconds(AUTHORIZATION_CODE_TTL_SECONDS);
    DB.query(
        "CREATE authorization_code CONTENT {
            code: $code,
            client: $client,
            person: $person,
            redirect_uri: $ru,
            scopes: $scopes,
            code_challenge: $challenge,
            code_challenge_method: $method,
            nonce: $nonce,
            session_id: $sid,
            expires_at: $exp,
            consumed: false
        } RETURN NONE",
    )
    .bind(("code", code.clone()))
    .bind(("client", args.client.clone()))
    .bind(("person", args.person.clone()))
    .bind(("ru", args.redirect_uri.to_string()))
    .bind(("scopes", args.scopes.to_vec()))
    .bind(("challenge", args.code_challenge.to_string()))
    .bind(("method", args.code_challenge_method.to_string()))
    .bind(("nonce", args.nonce.map(|s| s.to_string())))
    .bind(("sid", session_id.clone()))
    .bind(("exp", expires_at))
    .await?;
    Ok((code, session_id))
}

/// Atomically consume an authorization code by `code` value. Returns the row
/// only if the code was unused and not expired.
pub async fn consume_authorization_code(code: &str) -> Result<Option<AuthorizationCodeRow>> {
    let mut resp = DB
        .query(
            "BEGIN;
             LET $row = (SELECT * FROM authorization_code \
                         WHERE code = $code AND consumed = false AND expires_at > time::now() \
                         LIMIT 1)[0];
             IF $row != NONE THEN UPDATE $row.id SET consumed = true END;
             RETURN $row;
             COMMIT;",
        )
        .bind(("code", code.to_string()))
        .await?;
    let row: Option<AuthorizationCodeRow> = resp.take(0).unwrap_or(None);
    Ok(row)
}

// ----- Access token -----

#[derive(Debug, Clone, SurrealValue, Serialize, Deserialize)]
pub struct AccessTokenRow {
    pub id: RecordId,
    pub token_hash: String,
    pub client: RecordId,
    pub person: RecordId,
    pub scopes: Vec<String>,
    pub session_id: String,
    #[serde(default)]
    #[surreal(default)]
    pub refresh_token: Option<RecordId>,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    #[surreal(default)]
    pub revoked_at: Option<DateTime<Utc>>,
}

pub struct CreateAccessToken<'a> {
    pub client: &'a RecordId,
    pub person: &'a RecordId,
    pub scopes: &'a [String],
    pub session_id: &'a str,
    pub refresh_token: Option<&'a RecordId>,
}

pub async fn create_access_token(args: CreateAccessToken<'_>) -> Result<String> {
    let token = random_opaque_token();
    let token_hash = sha256_hex(&token);
    let expires_at = Utc::now() + Duration::seconds(ACCESS_TOKEN_TTL_SECONDS);
    DB.query(
        "CREATE access_token CONTENT {
            token_hash: $hash,
            client: $client,
            person: $person,
            scopes: $scopes,
            session_id: $sid,
            refresh_token: $rt,
            expires_at: $exp
        } RETURN NONE",
    )
    .bind(("hash", token_hash))
    .bind(("client", args.client.clone()))
    .bind(("person", args.person.clone()))
    .bind(("scopes", args.scopes.to_vec()))
    .bind(("sid", args.session_id.to_string()))
    .bind(("rt", args.refresh_token.cloned()))
    .bind(("exp", expires_at))
    .await?;
    Ok(token)
}

pub async fn lookup_access_token(token: &str) -> Result<Option<AccessTokenRow>> {
    let token_hash = sha256_hex(token);
    let mut resp = DB
        .query(
            "SELECT * FROM access_token \
             WHERE token_hash = $hash AND revoked_at IS NONE AND expires_at > time::now() \
             LIMIT 1",
        )
        .bind(("hash", token_hash))
        .await?;
    let rows: Vec<AccessTokenRow> = resp.take(0).unwrap_or_default();
    Ok(rows.into_iter().next())
}

// ----- Refresh token -----

#[derive(Debug, Clone, SurrealValue, Serialize, Deserialize)]
pub struct RefreshTokenRow {
    pub id: RecordId,
    pub token_hash: String,
    pub client: RecordId,
    pub person: RecordId,
    pub scopes: Vec<String>,
    pub session_id: String,
    #[serde(default)]
    #[surreal(default)]
    pub rotated_from: Option<RecordId>,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    #[surreal(default)]
    pub revoked_at: Option<DateTime<Utc>>,
}

pub struct CreateRefreshToken<'a> {
    pub client: &'a RecordId,
    pub person: &'a RecordId,
    pub scopes: &'a [String],
    pub session_id: &'a str,
    pub rotated_from: Option<&'a RecordId>,
}

pub async fn create_refresh_token(args: CreateRefreshToken<'_>) -> Result<(String, RecordId)> {
    let token = random_opaque_token();
    let token_hash = sha256_hex(&token);
    let expires_at = Utc::now() + Duration::seconds(REFRESH_TOKEN_TTL_SECONDS);
    let mut resp = DB
        .query(
            "CREATE refresh_token CONTENT {
                token_hash: $hash,
                client: $client,
                person: $person,
                scopes: $scopes,
                session_id: $sid,
                rotated_from: $rotated_from,
                expires_at: $exp
            }",
        )
        .bind(("hash", token_hash))
        .bind(("client", args.client.clone()))
        .bind(("person", args.person.clone()))
        .bind(("scopes", args.scopes.to_vec()))
        .bind(("sid", args.session_id.to_string()))
        .bind(("rotated_from", args.rotated_from.cloned()))
        .bind(("exp", expires_at))
        .await?;
    let rows: Vec<RefreshTokenRow> = resp.take(0).unwrap_or_default();
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| Error::Internal("refresh_token create returned no row".into()))?;
    Ok((token, row.id))
}

pub async fn lookup_refresh_token(token: &str) -> Result<Option<RefreshTokenRow>> {
    let token_hash = sha256_hex(token);
    let mut resp = DB
        .query(
            "SELECT * FROM refresh_token \
             WHERE token_hash = $hash AND revoked_at IS NONE AND expires_at > time::now() \
             LIMIT 1",
        )
        .bind(("hash", token_hash))
        .await?;
    let rows: Vec<RefreshTokenRow> = resp.take(0).unwrap_or_default();
    Ok(rows.into_iter().next())
}

/// Revoke an entire session chain (refresh + access tokens). Used on:
///   - Logout
///   - Refresh-token reuse detection (RFC 6749 §10.4)
///   - Org-membership revocation
pub async fn revoke_session(session_id: &str) -> Result<()> {
    DB.query(
        "UPDATE refresh_token SET revoked_at = time::now() \
         WHERE session_id = $sid AND revoked_at IS NONE;
         UPDATE access_token SET revoked_at = time::now() \
         WHERE session_id = $sid AND revoked_at IS NONE;",
    )
    .bind(("sid", session_id.to_string()))
    .await?;
    Ok(())
}

/// Mark a single refresh_token revoked.
pub async fn revoke_refresh_token(id: &RecordId) -> Result<()> {
    DB.query("UPDATE $id SET revoked_at = time::now()")
        .bind(("id", id.clone()))
        .await?;
    Ok(())
}

/// Mark a single access_token revoked.
pub async fn revoke_access_token_by_hash(token_hash: &str) -> Result<()> {
    DB.query("UPDATE access_token SET revoked_at = time::now() WHERE token_hash = $h")
        .bind(("h", token_hash.to_string()))
        .await?;
    Ok(())
}

/// List active sessions (one row per session_id) for a client. Returns a
/// summary suitable for the admin UI.
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub person_id: String,
    pub scopes: Vec<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

pub async fn list_sessions_for_client(client: &RecordId) -> Result<Vec<SessionSummary>> {
    #[derive(Debug, SurrealValue, Serialize, Deserialize)]
    struct Row {
        session_id: String,
        person: String,
        scopes: Vec<String>,
        created_at: DateTime<Utc>,
        #[serde(default)]
        #[surreal(default)]
        last_used_at: Option<DateTime<Utc>>,
    }
    let mut resp = DB
        .query(
            "SELECT session_id, <string> person AS person, scopes, created_at, \
             NONE AS last_used_at \
             FROM refresh_token \
             WHERE client = $client AND revoked_at IS NONE AND expires_at > time::now() \
             ORDER BY created_at DESC",
        )
        .bind(("client", client.clone()))
        .await?;
    let rows: Vec<Row> = resp.take(0).unwrap_or_default();
    Ok(rows
        .into_iter()
        .map(|r| SessionSummary {
            session_id: r.session_id,
            person_id: r.person,
            scopes: r.scopes,
            created_at: r.created_at.to_rfc3339(),
            last_used_at: r.last_used_at.map(|d| d.to_rfc3339()),
        })
        .collect())
}

/// Revoke all sessions for a client (used by Revoke All button).
pub async fn revoke_all_sessions_for_client(client: &RecordId) -> Result<()> {
    DB.query(
        "UPDATE refresh_token SET revoked_at = time::now() \
            WHERE client = $client AND revoked_at IS NONE;
         UPDATE access_token SET revoked_at = time::now() \
            WHERE client = $client AND revoked_at IS NONE;",
    )
    .bind(("client", client.clone()))
    .await?;
    Ok(())
}

/// For the SSF subject lookup: find a session_id by (client, person).
pub async fn find_active_session_ids_for_person_and_client(
    client: &RecordId,
    person: &RecordId,
) -> Result<Vec<String>> {
    #[derive(SurrealValue, Deserialize, Serialize)]
    struct Row {
        session_id: String,
    }
    let mut resp = DB
        .query(
            "SELECT session_id FROM refresh_token \
             WHERE client = $client AND person = $person \
             AND revoked_at IS NONE AND expires_at > time::now()",
        )
        .bind(("client", client.clone()))
        .bind(("person", person.clone()))
        .await?;
    let rows: Vec<Row> = resp.take(0).unwrap_or_default();
    Ok(rows.into_iter().map(|r| r.session_id).collect())
}

/// Format a RecordId for log display.
pub fn fmt_record(id: &RecordId) -> String {
    id.to_raw_string()
}
