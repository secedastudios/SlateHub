-- Migration 001: Convert production_role (string) to production_roles (array<string>)
-- Affects: member_of, pending_invitation
-- Also backfills involvement records for accepted members missing them
-- Date: 2026-03-17

BEGIN TRANSACTION;

UPDATE member_of SET production_roles = IF production_role IS NOT NONE AND production_role != NONE THEN [production_role] ELSE production_roles END;

UPDATE member_of UNSET production_role;

UPDATE pending_invitation SET production_roles = IF production_role IS NOT NONE AND production_role != NONE THEN [production_role] ELSE production_roles END;

UPDATE pending_invitation UNSET production_role;

DEFINE FIELD OVERWRITE production_roles ON member_of TYPE option<array<string>> PERMISSIONS FULL;

DEFINE FIELD OVERWRITE production_roles ON pending_invitation TYPE option<array<string>> PERMISSIONS FULL;

REMOVE FIELD IF EXISTS production_role ON TABLE member_of;

REMOVE FIELD IF EXISTS production_role ON TABLE pending_invitation;

LET $members = (SELECT in, out, production_roles FROM member_of WHERE invitation_status = 'accepted' AND production_roles IS NOT NONE);

FOR $m IN $members { LET $person = $m.in; LET $production = $m.out; FOR $role IN $m.production_roles { LET $exists = (SELECT count() AS c FROM involvement WHERE in = $person AND out = $production AND role = $role); IF $exists[0].c == 0 { RELATE $person->involvement->$production SET relation_type = 'crew', role = $role, source = 'invited', verification_status = 'pending_verification', timestamp = time::now(); }; }; };

COMMIT TRANSACTION;
