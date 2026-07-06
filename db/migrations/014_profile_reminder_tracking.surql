-- Migration 014: profile-completion reminder tracking.
--
-- Bookkeeping for the profile-reminder sweep (services::profile_reminders),
-- which nudges verified-but-empty accounts to fill in their profile and,
-- after a final reminder + grace period, removes them so the directory stays
-- a real listing of working talent and crew.
--
--   profile_reminders_sent    -> how many reminders this account has received (0..3)
--   last_profile_reminder_at  -> when the most recent reminder went out (for spacing)
--
-- profile_reminders_sent is a non-optional int on a table with existing rows,
-- so it MUST be DEFAULT ALWAYS + backfilled: SCHEMAFULL re-validates the whole
-- row on every UPDATE, and a legacy NULL in a plain `TYPE int DEFAULT 0` field
-- fails coercion ("Expected `int` but found `NONE`"), breaking ALL updates to
-- pre-migration person rows (admin verification changes, profile edits, email
-- verification). DEFAULT ALWAYS fills NONE on update without overwriting real
-- values; the backfill fixes rows that are never written again.
--
-- OVERWRITE makes re-running idempotent.

DEFINE FIELD OVERWRITE profile_reminders_sent   ON person TYPE int DEFAULT ALWAYS 0 PERMISSIONS FULL;
DEFINE FIELD OVERWRITE last_profile_reminder_at ON person TYPE option<datetime> PERMISSIONS FULL;

UPDATE person SET profile_reminders_sent = 0 WHERE profile_reminders_sent IS NONE;
