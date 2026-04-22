-- Migration 009: OIDC signing key private material is stored as a base64url
-- string, not raw bytes. The Rust SDK's Vec<u8>→bytes binding wasn't working
-- under v3.0.1 and CREATE was silently failing. No data to preserve because
-- CREATE never succeeded — the table is empty on every deploy that hit this.

REMOVE TABLE oidc_signing_key;

DEFINE TABLE oidc_signing_key TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD kid ON oidc_signing_key TYPE string PERMISSIONS FULL;
DEFINE FIELD algorithm ON oidc_signing_key TYPE string DEFAULT "EdDSA" PERMISSIONS FULL;
DEFINE FIELD public_jwk ON oidc_signing_key TYPE object FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD private_pkcs8 ON oidc_signing_key TYPE string PERMISSIONS FULL;  -- base64url of PKCS8 DER
DEFINE FIELD active ON oidc_signing_key TYPE bool DEFAULT false PERMISSIONS FULL;
DEFINE FIELD not_before ON oidc_signing_key TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD not_after ON oidc_signing_key TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD created_at ON oidc_signing_key TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;

DEFINE INDEX idx_oidc_signing_key_kid ON oidc_signing_key FIELDS kid UNIQUE;
DEFINE INDEX idx_oidc_signing_key_active ON oidc_signing_key FIELDS active;
