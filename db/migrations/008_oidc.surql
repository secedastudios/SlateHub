-- Migration 008: OpenID Connect provider
--
-- Adds tables for Slatehub to act as an OpenID Connect provider for org apps.
-- Companion to the OAuth/OIDC feature: one oauth_client per org, ed25519-signed
-- id_tokens, opaque access + refresh tokens, consent grants as a graph edge,
-- and SSF/CAEP/RISC security event delivery.

-- ------------------------------
-- Reserved names additions
-- ------------------------------
INSERT INTO reserved_names (name) VALUES
    ("oidc"), ("oauth"), ("authorize"), ("token"), ("userinfo"), ("jwks"),
    ("revoke"), ("introspect"), ("ssf"), ("well-known"),
    ("developer"), ("developers");

-- ------------------------------
-- TABLE: oidc_signing_key (global ed25519 keys for id_token + JWKS)
-- ------------------------------

DEFINE TABLE oidc_signing_key TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD kid ON oidc_signing_key TYPE string PERMISSIONS FULL;
DEFINE FIELD algorithm ON oidc_signing_key TYPE string DEFAULT "EdDSA" PERMISSIONS FULL;
DEFINE FIELD public_jwk ON oidc_signing_key TYPE object FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD private_pkcs8 ON oidc_signing_key TYPE bytes PERMISSIONS FULL;
DEFINE FIELD active ON oidc_signing_key TYPE bool DEFAULT false PERMISSIONS FULL;
DEFINE FIELD not_before ON oidc_signing_key TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD not_after ON oidc_signing_key TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD created_at ON oidc_signing_key TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;

DEFINE INDEX idx_oidc_signing_key_kid ON oidc_signing_key FIELDS kid UNIQUE;
DEFINE INDEX idx_oidc_signing_key_active ON oidc_signing_key FIELDS active;

-- ------------------------------
-- TABLE: oauth_client (one per org)
-- ------------------------------

DEFINE TABLE oauth_client TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD organization ON oauth_client TYPE record<organization> PERMISSIONS FULL;
DEFINE FIELD client_id ON oauth_client TYPE string PERMISSIONS FULL;
DEFINE FIELD client_secret_hash ON oauth_client TYPE string PERMISSIONS FULL;
DEFINE FIELD client_secret_hash_previous ON oauth_client TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD previous_expires_at ON oauth_client TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD name ON oauth_client TYPE string PERMISSIONS FULL;
DEFINE FIELD logo_uri ON oauth_client TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD redirect_uris ON oauth_client TYPE array<string> DEFAULT [] PERMISSIONS FULL;
DEFINE FIELD post_logout_redirect_uris ON oauth_client TYPE array<string> DEFAULT [] PERMISSIONS FULL;
DEFINE FIELD allowed_scopes ON oauth_client TYPE array<string>
    DEFAULT ["openid", "profile", "email", "offline_access", "slatehub:org_membership"]
    PERMISSIONS FULL;
DEFINE FIELD token_endpoint_auth_method ON oauth_client TYPE string
    DEFAULT "client_secret_basic" PERMISSIONS FULL;  -- "client_secret_basic" | "client_secret_post"
DEFINE FIELD require_pkce ON oauth_client TYPE bool DEFAULT true PERMISSIONS FULL;
-- SSF / CAEP / RISC receiver config
DEFINE FIELD ssf_receiver_endpoint ON oauth_client TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD ssf_receiver_jwks_uri ON oauth_client TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD ssf_delivery_method ON oauth_client TYPE string DEFAULT "push" PERMISSIONS FULL;  -- "push" | "poll"
DEFINE FIELD ssf_events_subscribed ON oauth_client TYPE array<string> DEFAULT [] PERMISSIONS FULL;
DEFINE FIELD created_at ON oauth_client TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON oauth_client TYPE datetime VALUE time::now() PERMISSIONS FULL;
DEFINE FIELD rotated_at ON oauth_client TYPE option<datetime> PERMISSIONS FULL;

DEFINE INDEX idx_oauth_client_client_id ON oauth_client FIELDS client_id UNIQUE;
DEFINE INDEX idx_oauth_client_org ON oauth_client FIELDS organization UNIQUE;

-- ------------------------------
-- TABLE: authorization_code (short-lived, single-use)
-- ------------------------------

DEFINE TABLE authorization_code TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD code ON authorization_code TYPE string PERMISSIONS FULL;
DEFINE FIELD client ON authorization_code TYPE record<oauth_client> PERMISSIONS FULL;
DEFINE FIELD person ON authorization_code TYPE record<person> PERMISSIONS FULL;
DEFINE FIELD redirect_uri ON authorization_code TYPE string PERMISSIONS FULL;
DEFINE FIELD scopes ON authorization_code TYPE array<string> PERMISSIONS FULL;
DEFINE FIELD code_challenge ON authorization_code TYPE string PERMISSIONS FULL;
DEFINE FIELD code_challenge_method ON authorization_code TYPE string DEFAULT "S256" PERMISSIONS FULL;
DEFINE FIELD nonce ON authorization_code TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD session_id ON authorization_code TYPE string PERMISSIONS FULL;
DEFINE FIELD expires_at ON authorization_code TYPE datetime PERMISSIONS FULL;
DEFINE FIELD consumed ON authorization_code TYPE bool DEFAULT false PERMISSIONS FULL;
DEFINE FIELD created_at ON authorization_code TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;

DEFINE INDEX idx_authorization_code_code ON authorization_code FIELDS code UNIQUE;
DEFINE INDEX idx_authorization_code_expires ON authorization_code FIELDS expires_at;

-- ------------------------------
-- TABLE: access_token (opaque bearer, sha256-hashed at rest)
-- ------------------------------

DEFINE TABLE access_token TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD token_hash ON access_token TYPE string PERMISSIONS FULL;
DEFINE FIELD client ON access_token TYPE record<oauth_client> PERMISSIONS FULL;
DEFINE FIELD person ON access_token TYPE record<person> PERMISSIONS FULL;
DEFINE FIELD scopes ON access_token TYPE array<string> PERMISSIONS FULL;
DEFINE FIELD session_id ON access_token TYPE string PERMISSIONS FULL;
DEFINE FIELD refresh_token ON access_token TYPE option<record<refresh_token>> PERMISSIONS FULL;
DEFINE FIELD expires_at ON access_token TYPE datetime PERMISSIONS FULL;
DEFINE FIELD revoked_at ON access_token TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD created_at ON access_token TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;

DEFINE INDEX idx_access_token_hash ON access_token FIELDS token_hash UNIQUE;
DEFINE INDEX idx_access_token_session ON access_token FIELDS session_id;
DEFINE INDEX idx_access_token_expires ON access_token FIELDS expires_at;

-- ------------------------------
-- TABLE: refresh_token (rotation-on-use)
-- ------------------------------

DEFINE TABLE refresh_token TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD token_hash ON refresh_token TYPE string PERMISSIONS FULL;
DEFINE FIELD client ON refresh_token TYPE record<oauth_client> PERMISSIONS FULL;
DEFINE FIELD person ON refresh_token TYPE record<person> PERMISSIONS FULL;
DEFINE FIELD scopes ON refresh_token TYPE array<string> PERMISSIONS FULL;
DEFINE FIELD session_id ON refresh_token TYPE string PERMISSIONS FULL;
DEFINE FIELD rotated_from ON refresh_token TYPE option<record<refresh_token>> PERMISSIONS FULL;
DEFINE FIELD expires_at ON refresh_token TYPE datetime PERMISSIONS FULL;
DEFINE FIELD revoked_at ON refresh_token TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD created_at ON refresh_token TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;

DEFINE INDEX idx_refresh_token_hash ON refresh_token FIELDS token_hash UNIQUE;
DEFINE INDEX idx_refresh_token_session ON refresh_token FIELDS session_id;

-- ------------------------------
-- TABLE: consent_grant (RELATION person -> oauth_client)
-- ------------------------------

DEFINE TABLE consent_grant TYPE RELATION FROM person TO oauth_client SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD scopes ON consent_grant TYPE array<string> DEFAULT [] PERMISSIONS FULL;
DEFINE FIELD granted_at ON consent_grant TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD revoked_at ON consent_grant TYPE option<datetime> PERMISSIONS FULL;

DEFINE INDEX idx_consent_grant_unique ON consent_grant FIELDS in, out UNIQUE;

-- ------------------------------
-- TABLE: security_event (SSF/CAEP/RISC outbound delivery queue)
-- ------------------------------

DEFINE TABLE security_event TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD client ON security_event TYPE record<oauth_client> PERMISSIONS FULL;
DEFINE FIELD subject ON security_event TYPE option<record<person>> PERMISSIONS FULL;
DEFINE FIELD event_type ON security_event TYPE string PERMISSIONS FULL;
DEFINE FIELD payload ON security_event TYPE object FLEXIBLE PERMISSIONS FULL;
DEFINE FIELD created_at ON security_event TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD delivered_at ON security_event TYPE option<datetime> PERMISSIONS FULL;
DEFINE FIELD attempts ON security_event TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD last_error ON security_event TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD acknowledged_at ON security_event TYPE option<datetime> PERMISSIONS FULL;

DEFINE INDEX idx_security_event_client ON security_event FIELDS client;
DEFINE INDEX idx_security_event_pending ON security_event FIELDS delivered_at;

-- ------------------------------
-- TABLE: ssf_stream (one stream per client, per SSF spec)
-- ------------------------------

DEFINE TABLE ssf_stream TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD client ON ssf_stream TYPE record<oauth_client> PERMISSIONS FULL;
DEFINE FIELD audience ON ssf_stream TYPE array<string> PERMISSIONS FULL;
DEFINE FIELD events_requested ON ssf_stream TYPE array<string> DEFAULT [] PERMISSIONS FULL;
DEFINE FIELD events_delivered ON ssf_stream TYPE array<string> DEFAULT [] PERMISSIONS FULL;
DEFINE FIELD status ON ssf_stream TYPE string DEFAULT "enabled" PERMISSIONS FULL;
DEFINE FIELD created_at ON ssf_stream TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON ssf_stream TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_ssf_stream_client ON ssf_stream FIELDS client UNIQUE;
