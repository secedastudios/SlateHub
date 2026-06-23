//! Helpers for generating, hashing, and validating OIDC tokens
//! (authorization codes, access tokens, refresh tokens).
//!
//! # Token model
//!
//! All tokens are *opaque* random strings (no JWTs here — id_tokens are
//! signed separately in [`crate::services::oidc_keys`]). They are produced by
//! [`random_opaque_token`]: 32 chars from a 32-symbol alphabet ≈ 160 bits.
//!
//! * **Authorization codes** are stored in cleartext in `authorization_code`
//!   (acceptable: single-use, 5-minute TTL, consumed atomically).
//! * **Access and refresh tokens** are never stored raw — only their
//!   [`sha256_hex`] digest goes in the `token_hash` column, so a DB leak
//!   doesn't yield usable bearer tokens. Lookups re-hash the presented token.
//!
//! # Lifetimes
//!
//! | Token | TTL | Constant |
//! |---|---|---|
//! | Authorization code | 5 min | [`AUTHORIZATION_CODE_TTL_SECONDS`] |
//! | Access token | 1 hour | [`ACCESS_TOKEN_TTL_SECONDS`] |
//! | Refresh token | 30 days (absolute, not sliding) | [`REFRESH_TOKEN_TTL_SECONDS`] |
//!
//! # Sessions and rotation
//!
//! Every authorization code mints a `session_id` ([`new_session_id`]) that is
//! stamped onto all access + refresh tokens descended from it, so one call to
//! [`revoke_session`] kills the whole chain (logout, refresh-token reuse
//! detection, org-membership revocation). Refresh-token rotation links the
//! replacement to its predecessor via `rotated_from`; the `/token` route is
//! responsible for revoking the old token and detecting reuse.
//!
//! No init step — all functions hit the global [`crate::db::DB`] connection
//! directly. State lives in the `authorization_code`, `access_token`, and
//! `refresh_token` tables.

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
/// 5 minutes matches the Google/Microsoft default. Single-use enforcement via
/// the `consumed` flag is the actual replay protection — the TTL just shouldn't
/// punish honest clients whose browser pauses on the consent screen.
pub const AUTHORIZATION_CODE_TTL_SECONDS: i64 = 300;
/// Lifetime of an access token.
pub const ACCESS_TOKEN_TTL_SECONDS: i64 = 3600;
/// Lifetime of a refresh token (absolute, not sliding).
pub const REFRESH_TOKEN_TTL_SECONDS: i64 = 60 * 60 * 24 * 30; // 30 days

/// Generate an opaque random token: [`TOKEN_LEN`] (32) chars drawn from a
/// 32-symbol base32-style alphabet with the ambiguous glyphs (`0`/`o`,
/// `1`/`l`) removed — ≈160 bits of entropy from the thread-local CSPRNG.
/// Used for authorization codes, access/refresh tokens, session ids, and
/// SET `jti` values.
pub fn random_opaque_token() -> String {
    const CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..TOKEN_LEN)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

/// Lowercase hex SHA-256 digest of `input`. This is the at-rest form of
/// access and refresh tokens (`token_hash` column) — the raw token is only
/// ever returned once to the client and re-hashed on lookup.
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

/// One row of the `authorization_code` table — a single-use OAuth 2.0
/// authorization code bound to a client, person, redirect URI, and PKCE
/// challenge.
#[derive(Debug, Clone, SurrealValue, Serialize, Deserialize)]
pub struct AuthorizationCodeRow {
    /// Record id of the row itself.
    pub id: RecordId,
    /// The opaque code value handed to the client (stored cleartext; see
    /// module docs for why that's acceptable).
    pub code: String,
    /// `oauth_client` record this code was issued to.
    pub client: RecordId,
    /// `person` record that authorized the request.
    pub person: RecordId,
    /// Exact redirect URI from the authorize request; the `/token` handler
    /// must see the same value again (RFC 6749 §4.1.3).
    pub redirect_uri: String,
    /// Scopes granted on the consent screen.
    pub scopes: Vec<String>,
    /// PKCE code challenge supplied by the client.
    pub code_challenge: String,
    /// PKCE challenge method (e.g. `S256`).
    pub code_challenge_method: String,
    /// OIDC nonce from the authorize request, echoed into the id_token.
    #[serde(default)]
    #[surreal(default)]
    pub nonce: Option<String>,
    /// Session id minted at creation; inherited by all tokens exchanged
    /// from this code.
    pub session_id: String,
    /// Hard expiry ([`AUTHORIZATION_CODE_TTL_SECONDS`] after creation).
    pub expires_at: DateTime<Utc>,
    /// Set to `true` atomically on first (and only) exchange.
    pub consumed: bool,
}

/// Borrowed arguments for [`create_authorization_code`].
pub struct CreateAuthorizationCode<'a> {
    /// `oauth_client` the code is issued to.
    pub client: &'a RecordId,
    /// `person` granting the authorization.
    pub person: &'a RecordId,
    /// Redirect URI to pin for the later `/token` exchange.
    pub redirect_uri: &'a str,
    /// Granted scopes.
    pub scopes: &'a [String],
    /// PKCE code challenge.
    pub code_challenge: &'a str,
    /// PKCE challenge method (e.g. `S256`).
    pub code_challenge_method: &'a str,
    /// Optional OIDC nonce to round-trip into the id_token.
    pub nonce: Option<&'a str>,
}

/// Mint a fresh authorization code (and the session id that will tie the
/// eventual access/refresh tokens together) and persist it with a
/// 5-minute expiry.
///
/// Returns `(code, session_id)` — both opaque random strings.
///
/// # Errors
///
/// Returns an error only if the `CREATE authorization_code` query fails
/// (database unavailable or schema mismatch).
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
///
/// The `WHERE` clause guarantees we only match a code that is currently
/// unconsumed and not expired, and the `UPDATE` flips `consumed` in the same
/// statement. `RETURN BEFORE` gives us the row's pre-update state — proof it
/// was valid at the moment of consumption. Single-statement, no transaction
/// needed, no ambiguity about which statement index carries the result.
pub async fn consume_authorization_code(code: &str) -> Result<Option<AuthorizationCodeRow>> {
    let mut resp = DB
        .query(
            "UPDATE authorization_code \
             SET consumed = true \
             WHERE code = $code AND consumed = false AND expires_at > time::now() \
             RETURN BEFORE;",
        )
        .bind(("code", code.to_string()))
        .await?;
    let rows: Vec<AuthorizationCodeRow> = resp.take(0).unwrap_or_default();
    Ok(rows.into_iter().next())
}

/// Read an authorization-code row by `code` without mutating it. Used by the
/// `/token` handler after a failed consume to report the precise failure
/// reason (not found / already consumed / expired) in `error_description`.
pub async fn peek_authorization_code(code: &str) -> Result<Option<AuthorizationCodeRow>> {
    let mut resp = DB
        .query("SELECT * FROM authorization_code WHERE code = $code LIMIT 1")
        .bind(("code", code.to_string()))
        .await?;
    let rows: Vec<AuthorizationCodeRow> = resp.take(0).unwrap_or_default();
    Ok(rows.into_iter().next())
}

// ----- Access token -----

/// One row of the `access_token` table. The bearer token itself is never
/// stored — only its SHA-256 hex digest in `token_hash`.
#[derive(Debug, Clone, SurrealValue, Serialize, Deserialize)]
pub struct AccessTokenRow {
    /// Record id of the row itself.
    pub id: RecordId,
    /// [`sha256_hex`] digest of the raw bearer token.
    pub token_hash: String,
    /// `oauth_client` the token was issued to.
    pub client: RecordId,
    /// `person` the token acts on behalf of.
    pub person: RecordId,
    /// Scopes carried by the token.
    pub scopes: Vec<String>,
    /// Session id shared with the sibling refresh token (revocation unit).
    pub session_id: String,
    /// `refresh_token` row this access token was minted alongside / from,
    /// if the grant included one.
    #[serde(default)]
    #[surreal(default)]
    pub refresh_token: Option<RecordId>,
    /// Hard expiry ([`ACCESS_TOKEN_TTL_SECONDS`] after creation).
    pub expires_at: DateTime<Utc>,
    /// Set when the token (or its whole session) is revoked; revoked tokens
    /// are excluded from [`lookup_access_token`].
    #[serde(default)]
    #[surreal(default)]
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Borrowed arguments for [`create_access_token`].
pub struct CreateAccessToken<'a> {
    /// `oauth_client` the token is issued to.
    pub client: &'a RecordId,
    /// `person` the token acts on behalf of.
    pub person: &'a RecordId,
    /// Scopes to grant.
    pub scopes: &'a [String],
    /// Session id inherited from the authorization code / refresh grant.
    pub session_id: &'a str,
    /// Sibling refresh-token row, when the grant mints one.
    pub refresh_token: Option<&'a RecordId>,
}

/// Mint a 1-hour access token: generates the raw bearer string, stores only
/// its hash, and returns the raw token (the single time it ever exists
/// outside the client).
///
/// # Errors
///
/// Returns an error only if the `CREATE access_token` query fails.
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

/// Validate a presented bearer token: hash it and return the matching row
/// only if it is unrevoked and unexpired. `None` means "reject with 401" —
/// the caller cannot distinguish unknown / revoked / expired (by design;
/// don't leak token state to unauthenticated callers).
///
/// # Errors
///
/// Returns an error only if the lookup query itself fails.
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

/// One row of the `refresh_token` table. Like access tokens, only the
/// SHA-256 hash of the token is stored; `rotated_from` chains each rotation
/// to its predecessor for reuse detection.
#[derive(Debug, Clone, SurrealValue, Serialize, Deserialize)]
pub struct RefreshTokenRow {
    /// Record id of the row itself.
    pub id: RecordId,
    /// [`sha256_hex`] digest of the raw refresh token.
    pub token_hash: String,
    /// `oauth_client` the token was issued to.
    pub client: RecordId,
    /// `person` the token acts on behalf of.
    pub person: RecordId,
    /// Scopes the token can mint access tokens for.
    pub scopes: Vec<String>,
    /// Session id shared across the whole access/refresh chain.
    pub session_id: String,
    /// Previous `refresh_token` row when this one was minted by rotation;
    /// `None` for the first token of a session.
    #[serde(default)]
    #[surreal(default)]
    pub rotated_from: Option<RecordId>,
    /// Absolute expiry ([`REFRESH_TOKEN_TTL_SECONDS`] after creation —
    /// rotation does not extend the original session's horizon).
    pub expires_at: DateTime<Utc>,
    /// Set when revoked (logout, rotation, reuse detection); revoked tokens
    /// are excluded from [`lookup_refresh_token`].
    #[serde(default)]
    #[surreal(default)]
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Borrowed arguments for [`create_refresh_token`].
pub struct CreateRefreshToken<'a> {
    /// `oauth_client` the token is issued to.
    pub client: &'a RecordId,
    /// `person` the token acts on behalf of.
    pub person: &'a RecordId,
    /// Scopes to carry forward.
    pub scopes: &'a [String],
    /// Session id inherited from the original authorization.
    pub session_id: &'a str,
    /// The refresh token being rotated out, when this mint is a rotation.
    pub rotated_from: Option<&'a RecordId>,
}

/// Mint a 30-day refresh token. Returns `(raw_token, row_id)` — the raw
/// string goes to the client (only the hash is stored), and the row id is
/// what callers thread into [`CreateAccessToken::refresh_token`] and the
/// next rotation's `rotated_from`.
///
/// # Errors
///
/// Fails if the `CREATE refresh_token` query errors, or with
/// `Error::Internal` if the create unexpectedly returns no row.
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

/// Validate a presented refresh token: hash it and return the matching row
/// only if it is unrevoked and unexpired. A `None` here for a token that
/// *was* once valid is the reuse-detection signal — the `/token` route
/// should then look the hash up without the liveness filters and revoke the
/// whole session (RFC 6749 §10.4).
///
/// # Errors
///
/// Returns an error only if the lookup query itself fails.
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

/// One active session as shown in the client admin UI — a render-ready
/// projection of a live `refresh_token` row (timestamps pre-formatted as
/// RFC 3339 strings).
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    /// Opaque session id grouping this refresh token with its access tokens.
    pub session_id: String,
    /// The person's record id, as a `person:key` string.
    pub person_id: String,
    /// Scopes granted to the session.
    pub scopes: Vec<String>,
    /// Session creation time, RFC 3339.
    pub created_at: String,
    /// Last-use time, RFC 3339. Currently always `None` — the query selects
    /// a `NONE` placeholder because last-use tracking isn't recorded yet.
    pub last_used_at: Option<String>,
}

/// List active sessions for a client (one row per live, unexpired refresh
/// token, newest first) as [`SessionSummary`] values for the admin UI.
///
/// # Errors
///
/// Returns an error only if the query itself fails.
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
