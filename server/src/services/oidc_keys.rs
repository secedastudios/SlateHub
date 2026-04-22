//! OIDC signing key management.
//!
//! Manages the global ed25519 keypair used to sign id_tokens and publish in JWKS.
//! On startup `ensure_signing_key` verifies an active key exists; if not, generates
//! one and persists it. Rotation (via the `oidc_rotate_key` CLI bin) inserts a new
//! active key, marks the prior key inactive, and sets its `not_after` so JWKS
//! continues to publish it during the overlap window.
//!
//! Tokens are signed with the single currently-active key; JWKS returns all keys
//! whose `not_after` has not elapsed.

use crate::db::DB;
use crate::error::{Error, Result};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use surrealdb::types::SurrealValue;
use tracing::{debug, info};

/// Cached representation of a signing key as loaded from the database.
pub struct SigningKeyEntry {
    pub kid: String,
    pub public_jwk: Value,
    pub private_pkcs8: Vec<u8>,
    pub not_before: DateTime<Utc>,
    pub not_after: Option<DateTime<Utc>>,
    pub active: bool,
}

#[derive(Debug, Deserialize, Serialize, SurrealValue)]
struct SigningKeyRow {
    #[serde(default)]
    #[surreal(default)]
    id: String,
    kid: String,
    public_jwk: Value,
    private_pkcs8: Vec<u8>,
    not_before: DateTime<Utc>,
    #[serde(default)]
    #[surreal(default)]
    not_after: Option<DateTime<Utc>>,
    active: bool,
}

/// Generate a random `kid` (16 chars, base32 alphabet without ambiguous glyphs).
fn random_kid() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..16)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

/// Build a public JWK object from a verifying key.
fn verifying_key_to_jwk(kid: &str, verifying: &ed25519_dalek::VerifyingKey) -> Value {
    let x = URL_SAFE_NO_PAD.encode(verifying.to_bytes());
    json!({
        "kty": "OKP",
        "crv": "Ed25519",
        "use": "sig",
        "alg": "EdDSA",
        "kid": kid,
        "x": x,
    })
}

/// Ensure at least one active signing key exists; generate one if not.
pub async fn ensure_signing_key() -> Result<()> {
    debug!("Checking for existing active OIDC signing key");

    // Query for the active key. Cast RecordId to string per v3.0.1 requirement.
    let mut resp = DB
        .query(
            "SELECT <string> id AS id, kid, public_jwk, private_pkcs8, \
             not_before, not_after, active \
             FROM oidc_signing_key WHERE active = true LIMIT 1",
        )
        .await?;
    let existing: Vec<SigningKeyRow> = resp.take(0).unwrap_or_default();
    if !existing.is_empty() {
        info!(
            kid = %existing[0].kid,
            "OIDC signing key present"
        );
        return Ok(());
    }

    info!("No active OIDC signing key found — generating a fresh ed25519 keypair");
    let signing = SigningKey::generate(&mut OsRng);
    let verifying = signing.verifying_key();
    let kid = random_kid();
    let public_jwk = verifying_key_to_jwk(&kid, &verifying);
    let private_pkcs8 = signing
        .to_pkcs8_der()
        .map_err(|e| Error::Internal(format!("ed25519 pkcs8 encode failed: {e}")))?
        .as_bytes()
        .to_vec();

    let _: Option<Value> = DB
        .query(
            "CREATE oidc_signing_key CONTENT {
                kid: $kid,
                algorithm: 'EdDSA',
                public_jwk: $jwk,
                private_pkcs8: $pkcs8,
                active: true
            } RETURN NONE",
        )
        .bind(("kid", kid.clone()))
        .bind(("jwk", public_jwk))
        .bind(("pkcs8", private_pkcs8))
        .await?
        .take(0)
        .unwrap_or(None);

    info!(kid = %kid, "OIDC signing key created");
    Ok(())
}

/// Load the single active signing key. Self-heals: if the table has no active
/// key (fresh DB, manual wipe, botched rotation) we generate one inline and
/// retry. The startup `ensure_signing_key` is a fast-path; this guarantees
/// `/token`, `/userinfo`, etc. never 500 because of a missing key.
pub async fn load_active_key() -> Result<SigningKeyEntry> {
    if let Some(entry) = fetch_active_key().await? {
        return Ok(entry);
    }
    info!("No active OIDC signing key on read path — generating one");
    ensure_signing_key().await?;
    fetch_active_key()
        .await?
        .ok_or_else(|| Error::Internal("OIDC signing key generation failed".into()))
}

async fn fetch_active_key() -> Result<Option<SigningKeyEntry>> {
    let mut resp = DB
        .query(
            "SELECT <string> id AS id, kid, public_jwk, private_pkcs8, \
             not_before, not_after, active \
             FROM oidc_signing_key WHERE active = true LIMIT 1",
        )
        .await?;
    let rows: Vec<SigningKeyRow> = resp.take(0).unwrap_or_default();
    Ok(rows.into_iter().next().map(|row| SigningKeyEntry {
        kid: row.kid,
        public_jwk: row.public_jwk,
        private_pkcs8: row.private_pkcs8,
        not_before: row.not_before,
        not_after: row.not_after,
        active: row.active,
    }))
}

/// Load every signing key whose `not_after` has not elapsed (for JWKS publication).
pub async fn load_published_keys() -> Result<Vec<SigningKeyEntry>> {
    let mut resp = DB
        .query(
            "SELECT <string> id AS id, kid, public_jwk, private_pkcs8, \
             not_before, not_after, active \
             FROM oidc_signing_key \
             WHERE not_after IS NONE OR not_after > time::now() \
             ORDER BY not_before DESC",
        )
        .await?;
    let rows: Vec<SigningKeyRow> = resp.take(0).unwrap_or_default();
    Ok(rows
        .into_iter()
        .map(|r| SigningKeyEntry {
            kid: r.kid,
            public_jwk: r.public_jwk,
            private_pkcs8: r.private_pkcs8,
            not_before: r.not_before,
            not_after: r.not_after,
            active: r.active,
        })
        .collect())
}

/// Build the JWKS JSON document. Self-heals: if no keys are published yet
/// (fresh DB, manual wipe), generate one inline so RPs can always verify
/// id_tokens.
pub async fn jwks_document() -> Result<Value> {
    let mut keys = load_published_keys().await?;
    if keys.is_empty() {
        info!("JWKS would be empty — generating signing key");
        ensure_signing_key().await?;
        keys = load_published_keys().await?;
    }
    let jwks: Vec<Value> = keys.into_iter().map(|k| k.public_jwk).collect();
    Ok(json!({ "keys": jwks }))
}

/// Sign an id_token JWT with the active key.
pub async fn sign_id_token<C: Serialize>(claims: &C) -> Result<String> {
    let key = load_active_key().await?;
    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = Some(key.kid.clone());
    let signing_key = SigningKey::from_pkcs8_der(&key.private_pkcs8)
        .map_err(|e| Error::Internal(format!("ed25519 pkcs8 decode failed: {e}")))?;
    let pkcs8 = signing_key
        .to_pkcs8_der()
        .map_err(|e| Error::Internal(format!("ed25519 pkcs8 re-encode failed: {e}")))?;
    let encoding_key = EncodingKey::from_ed_der(pkcs8.as_bytes());
    encode(&header, claims, &encoding_key)
        .map_err(|e| Error::Internal(format!("id_token sign failed: {e}")))
}

/// Rotate the active signing key: generate a new active key and mark the current
/// one inactive with a `not_after` grace window (default 7 days) so JWKS keeps
/// publishing it until any outstanding tokens expire.
pub async fn rotate_signing_key(grace_days: i64) -> Result<String> {
    let new_signing = SigningKey::generate(&mut OsRng);
    let verifying = new_signing.verifying_key();
    let new_kid = random_kid();
    let new_jwk = verifying_key_to_jwk(&new_kid, &verifying);
    let new_pkcs8 = new_signing
        .to_pkcs8_der()
        .map_err(|e| Error::Internal(format!("ed25519 pkcs8 encode failed: {e}")))?
        .as_bytes()
        .to_vec();

    // Retire the current active key (if any) and create the new active key in one transaction.
    let not_after = Utc::now() + chrono::Duration::days(grace_days);
    DB.query(
        "BEGIN;
         UPDATE oidc_signing_key SET active = false, not_after = $not_after
             WHERE active = true AND not_after IS NONE;
         CREATE oidc_signing_key CONTENT {
             kid: $kid,
             algorithm: 'EdDSA',
             public_jwk: $jwk,
             private_pkcs8: $pkcs8,
             active: true
         };
         COMMIT;",
    )
    .bind(("not_after", not_after))
    .bind(("kid", new_kid.clone()))
    .bind(("jwk", new_jwk))
    .bind(("pkcs8", new_pkcs8))
    .await?;

    info!(kid = %new_kid, "Rotated OIDC signing key");
    Ok(new_kid)
}
