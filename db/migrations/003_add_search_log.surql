-- Migration 003: Add search_log table for search analytics
-- Date: 2026-03-18

BEGIN TRANSACTION;

DEFINE TABLE search_log TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;
DEFINE FIELD query ON search_log TYPE string PERMISSIONS FULL;
DEFINE FIELD source ON search_log TYPE string PERMISSIONS FULL;         -- "web", "mcp", "api"
DEFINE FIELD category ON search_log TYPE string PERMISSIONS FULL;       -- "all", "people", "productions", "organizations", "locations", "jobs"
DEFINE FIELD result_count ON search_log TYPE option<int> PERMISSIONS FULL;
DEFINE FIELD created_at ON search_log TYPE datetime VALUE time::now() PERMISSIONS FULL;
DEFINE INDEX idx_search_log_created ON search_log FIELDS created_at;
DEFINE INDEX idx_search_log_source ON search_log FIELDS source;
DEFINE INDEX idx_search_log_category ON search_log FIELDS category;

COMMIT TRANSACTION;
