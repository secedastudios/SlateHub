-- Migration 006: Add activity event tracking table

DEFINE TABLE activity_event TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;

DEFINE FIELD person_id  ON activity_event TYPE option<record<person>> PERMISSIONS FULL;
DEFINE FIELD event_type ON activity_event TYPE string PERMISSIONS FULL;
DEFINE FIELD path       ON activity_event TYPE string PERMISSIONS FULL;
DEFINE FIELD created_at ON activity_event TYPE datetime VALUE time::now() PERMISSIONS FULL;

DEFINE INDEX idx_activity_event_time      ON activity_event FIELDS created_at;
DEFINE INDEX idx_activity_event_type_time ON activity_event FIELDS event_type, created_at;
DEFINE INDEX idx_activity_event_person    ON activity_event FIELDS person_id, created_at;
