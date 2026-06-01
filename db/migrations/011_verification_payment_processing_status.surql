-- Migration 011: add 'processing' to verification_payment.status ASSERT.
--
-- Migration 010 was applied to deployments before we added support for the
-- `identity.verification_session.processing` Stripe webhook. This migration
-- re-DEFINEs just the status field with the expanded ASSERT list.
--
-- Uses `OVERWRITE` so it's safe to run on environments that:
--   - have 010 applied (the common case — updates the ASSERT in place)
--   - or were initialised fresh from schema.surql (no-op-ish; same result)

DEFINE FIELD OVERWRITE status ON verification_payment TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'paid', 'processing', 'verified', 'failed', 'refunded'] PERMISSIONS FULL;
