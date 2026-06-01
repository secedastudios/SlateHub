-- Migration 010: paid identity-verification flow (Stripe) + feature flags.
--
-- Two tables introduced together for the paid /get-verified feature:
--   * verification_payment — one row per Stripe Checkout + Identity attempt
--   * feature_flag         — runtime on/off (+ audience) switches, code-registered
--
-- ===========================================================================
-- TABLE: verification_payment
-- ===========================================================================
--
-- Records one payment per attempt. A person can have multiple payments over
-- time (e.g., refunded after failure, paid again). The Stripe Checkout
-- Session is the source of truth for payment state; the Identity Session
-- (created after successful checkout) is the source of truth for verification.
--
-- Status transitions:
--   pending   → Checkout Session created, payment not yet captured
--   paid      → checkout.session.completed received; Identity Session may
--               or may not exist yet
--   verified  → identity.verification_session.verified received; the
--               person.verification_status is set to 'identity' atomically
--   failed    → identity terminally failed or user canceled; usually
--               followed by refund
--   refunded  → Stripe refund issued (either after fail or after stale
--               24h timeout from the background job)

DEFINE TABLE verification_payment TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD person ON verification_payment TYPE record<person> PERMISSIONS FULL;
DEFINE FIELD stripe_checkout_session_id ON verification_payment TYPE string PERMISSIONS FULL;
DEFINE FIELD stripe_payment_intent_id ON verification_payment TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD stripe_identity_session_id ON verification_payment TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD amount_minor ON verification_payment TYPE int PERMISSIONS FULL;  -- amount in minor units (cents, pence, etc.)
DEFINE FIELD currency ON verification_payment TYPE string PERMISSIONS FULL;   -- ISO 4217 lowercase (e.g. "eur", "usd")
DEFINE FIELD status ON verification_payment TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'paid', 'verified', 'failed', 'refunded'] PERMISSIONS FULL;
DEFINE FIELD failure_reason ON verification_payment TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD refund_id ON verification_payment TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON verification_payment TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;
DEFINE FIELD updated_at ON verification_payment TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_verification_payment_person ON verification_payment FIELDS person;
DEFINE INDEX idx_verification_payment_checkout ON verification_payment FIELDS stripe_checkout_session_id UNIQUE;
DEFINE INDEX idx_verification_payment_identity ON verification_payment FIELDS stripe_identity_session_id;
DEFINE INDEX idx_verification_payment_status_created ON verification_payment FIELDS status, created_at;

-- ===========================================================================
-- TABLE: feature_flag
-- ===========================================================================
--
-- One row per flag, identified by a string `key`. State is one of:
--   off         — feature is disabled for everyone
--   admin_only  — only users with is_admin = true see it
--   verified    — only users with verification_status = 'identity' see it
--   all         — visible to everyone (anonymous included, depending on route)
--
-- Flags are registered in code (server/src/services/feature_flag.rs). Rows
-- in this table are seeded by the server at boot for any flag not yet
-- present, defaulting to 'off'. The admin UI mutates the `state` field.

DEFINE TABLE feature_flag TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD key ON feature_flag TYPE string PERMISSIONS FULL;
DEFINE FIELD name ON feature_flag TYPE string PERMISSIONS FULL;
DEFINE FIELD description ON feature_flag TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD state ON feature_flag TYPE string DEFAULT 'off'
    ASSERT $value IN ['off', 'admin_only', 'verified', 'all'] PERMISSIONS FULL;
DEFINE FIELD updated_at ON feature_flag TYPE datetime VALUE time::now() PERMISSIONS FULL;
DEFINE FIELD updated_by ON feature_flag TYPE option<record<person>> PERMISSIONS FULL;

DEFINE INDEX idx_feature_flag_key ON feature_flag FIELDS key UNIQUE;
