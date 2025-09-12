-- Test script to verify schema changes from 'value' to 'name' field
-- Run this against SurrealDB to ensure the changes work correctly

-- Test selecting from organization_type table
SELECT id, name FROM organization_type ORDER BY name LIMIT 5;

-- Test selecting from production_type table
SELECT id, name FROM production_type ORDER BY name LIMIT 5;

-- Test selecting from production_status table
SELECT id, name FROM production_status ORDER BY name LIMIT 5;

-- Test selecting from role table with department
SELECT id, name, department FROM role ORDER BY name LIMIT 5;

-- Test selecting from department table
SELECT id, name FROM department ORDER BY name LIMIT 5;

-- Test selecting from phase table
SELECT id, name FROM phase ORDER BY name LIMIT 5;

-- Test selecting from credit_type table
SELECT id, name FROM credit_type ORDER BY name LIMIT 5;

-- Test selecting from ownership_type table
SELECT id, name FROM ownership_type ORDER BY name LIMIT 5;

-- Test selecting from gender table
SELECT id, name FROM gender ORDER BY name LIMIT 5;

-- Test selecting from ethnicity table
SELECT id, name FROM ethnicity ORDER BY name LIMIT 5;

-- Test selecting from hair_color table
SELECT id, name FROM hair_color ORDER BY name LIMIT 5;

-- Test selecting from eye_color table
SELECT id, name FROM eye_color ORDER BY name LIMIT 5;

-- Test selecting from body_type table
SELECT id, name FROM body_type ORDER BY name LIMIT 5;

-- Test selecting from union table
SELECT id, name FROM union ORDER BY name LIMIT 5;

-- Test that we can still create organizations with the type field
-- (using the ID from organization_type table)
SELECT id, name FROM organization_type WHERE name = 'production_company';

-- Verify the old 'value' field no longer exists (this should fail)
-- Uncomment to test: SELECT value FROM organization_type LIMIT 1;

-- Test joining organization with organization_type to get the type name
-- This shows how to get the human-readable name when needed
SELECT
    o.name as org_name,
    o.type as type_id,
    ot.name as type_name
FROM organization o
LEFT JOIN organization_type ot ON ot.id = o.type
LIMIT 5;
