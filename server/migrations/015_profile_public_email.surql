-- Migration 015: opt-in public contact email on person profiles.
--
-- Until now the profile page showed the account's SIGN-IN email whenever
-- `profile.is_public` was on. That address is a credential, not contact
-- info, so it is no longer displayed anywhere. Instead a person can enter a
-- separate email on the edit form, labelled "(displayed publicly)":
--
--   profile.public_email -> option<string>, must be a valid address when set
--
-- `option<>` on the FLEXIBLE profile object means no backfill is needed:
-- existing rows simply read as NONE ("no public email") and every later
-- UPDATE still validates (unlike a non-optional field — see the DEFAULT
-- ALWAYS note in db/schema.surql for profile_reminders_sent).
-- OVERWRITE makes re-running idempotent; fresh installs already carry the
-- field from db/schema.surql.
--
-- Non-destructive: adds one optional field, touches no rows.

DEFINE FIELD OVERWRITE profile.public_email ON person TYPE option<string> ASSERT $value = NONE OR string::is_email($value) PERMISSIONS FULL;
