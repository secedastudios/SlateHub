-- Migration 0002: Add feedback table for user feedback submissions
-- Safe to re-run: all definitions use IF NOT EXISTS.

DEFINE TABLE IF NOT EXISTS feedback TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS username ON feedback TYPE string PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS page_url ON feedback TYPE string PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS message ON feedback TYPE string PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS created_at ON feedback TYPE datetime DEFAULT time::now() PERMISSIONS FULL;
DEFINE INDEX IF NOT EXISTS idx_feedback_created ON feedback FIELDS created_at;
