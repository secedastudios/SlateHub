-- Migration 013: landing-page campaign analytics.
--
-- Dedicated funnel tracking for the on-disk `/a/{campaign}` ad landing pages,
-- kept separate from the site-wide `activity_event` time-series so per-campaign
-- conversion funnels stay clean and cheap to query.
--
-- Funnel stages (event_type):
--   view             -> landing page rendered (anonymous, keyed by visitor_id cookie)
--   cta_click        -> reserved for explicit CTA instrumentation
--   signup_started   -> visitor reached /signup carrying the campaign
--   signup_completed -> CONVERSION. Emitted at EMAIL VERIFICATION (not account
--                       creation), because "email verified" is the conversion.
--                       Attribution survives the user clicking the verify link
--                       in a later session via `person.signup_campaign`.

DEFINE TABLE landing_event TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD campaign   ON landing_event TYPE string PERMISSIONS FULL;             -- campaign id, e.g. "not-on-set"
DEFINE FIELD event_type ON landing_event TYPE string
    ASSERT $value IN ['view', 'cta_click', 'signup_started', 'signup_completed'] PERMISSIONS FULL;
DEFINE FIELD person     ON landing_event TYPE option<record<person>> PERMISSIONS FULL;  -- set once the person is known
DEFINE FIELD role       ON landing_event TYPE option<string> PERMISSIONS FULL;     -- selected role chip — analytics only, never applied to the account
DEFINE FIELD visitor_id ON landing_event TYPE option<string> PERMISSIONS FULL;     -- anonymous cookie id, correlates a visitor's funnel steps
DEFINE FIELD referrer   ON landing_event TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD path       ON landing_event TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD created_at ON landing_event TYPE datetime VALUE $value OR time::now() PERMISSIONS FULL;

DEFINE INDEX idx_landing_event_campaign_type_time ON landing_event FIELDS campaign, event_type, created_at;
DEFINE INDEX idx_landing_event_time               ON landing_event FIELDS created_at;
DEFINE INDEX idx_landing_event_person             ON landing_event FIELDS person;

-- Attribution carrier: which landing-page campaign (if any) a person signed up
-- through. Written at account creation; read at email-verification to emit the
-- `signup_completed` conversion. OVERWRITE makes re-running idempotent.
DEFINE FIELD OVERWRITE signup_campaign ON person TYPE option<string> PERMISSIONS FULL;
DEFINE INDEX IF NOT EXISTS idx_person_signup_campaign ON person FIELDS signup_campaign;
