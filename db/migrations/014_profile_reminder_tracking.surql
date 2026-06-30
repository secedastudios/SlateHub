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
-- OVERWRITE makes re-running idempotent.

DEFINE FIELD OVERWRITE profile_reminders_sent   ON person TYPE int DEFAULT 0 PERMISSIONS FULL;
DEFINE FIELD OVERWRITE last_profile_reminder_at ON person TYPE option<datetime> PERMISSIONS FULL;
