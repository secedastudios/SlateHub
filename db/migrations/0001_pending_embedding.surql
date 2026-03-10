-- Migration 0001: Add pending_embedding table + migration tracking
-- Bootstraps the schema_migration table used by the migration system itself.
--
-- Safe to re-run: all definitions use IF NOT EXISTS.

-- Bootstrap migration tracking (this is the first migration, so the table may not exist yet)
DEFINE TABLE IF NOT EXISTS schema_migration TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS name ON schema_migration TYPE string PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS applied_at ON schema_migration TYPE datetime DEFAULT time::now() PERMISSIONS FULL;
DEFINE INDEX IF NOT EXISTS idx_schema_migration_name ON schema_migration FIELDS name UNIQUE;

-- Durable queue for background embedding generation
DEFINE TABLE IF NOT EXISTS pending_embedding TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD IF NOT EXISTS target ON pending_embedding TYPE record PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS embedding_text ON pending_embedding TYPE string PERMISSIONS FULL;
DEFINE FIELD IF NOT EXISTS created_at ON pending_embedding TYPE datetime DEFAULT time::now() PERMISSIONS FULL;
DEFINE INDEX IF NOT EXISTS idx_pending_embedding_target ON pending_embedding FIELDS target UNIQUE;
