-- Migration 004: Add join request support for organizations
-- Adds allow_join_requests flag to organizations and request_note to member_of

DEFINE FIELD allow_join_requests ON organization TYPE bool DEFAULT false PERMISSIONS FULL;
DEFINE FIELD request_note ON member_of TYPE option<string> PERMISSIONS FULL;
DEFINE FIELD OVERWRITE notification_type ON notification TYPE string ASSERT $value IN ['invitation', 'invitation_accepted', 'member_joined', 'general', 'message', 'job_application', 'application_update', 'join_request'] PERMISSIONS FULL;
