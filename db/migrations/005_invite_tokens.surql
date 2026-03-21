-- Migration 005: Add invite link tokens and make email optional

DEFINE FIELD OVERWRITE token ON pending_invitation TYPE option<string> PERMISSIONS FULL;
DEFINE INDEX OVERWRITE idx_pending_invitation_token ON pending_invitation FIELDS token UNIQUE;
DEFINE FIELD OVERWRITE email ON pending_invitation TYPE option<string> PERMISSIONS FULL;
