-- Test script for verifying membership queries work correctly
-- Run this with: surreal sql --conn http://localhost:8000 --user root --pass root --ns slatehub --db slatehub < test_scripts/test_membership.surql

-- Clean up any existing test data
DELETE person:testperson1;
DELETE person:testperson2;
DELETE organization:testorg1;
DELETE member_of WHERE in = person:testperson1;
DELETE member_of WHERE in = person:testperson2;
DELETE member_of WHERE out = organization:testorg1;

-- Create test persons
CREATE person:testperson1 SET
    username = "testuser1",
    email = "test1@example.com",
    password_hash = "dummy_hash",
    profile = {
        name: "Test User One",
        bio: "Test bio",
        location: "Test City"
    },
    created_at = time::now(),
    updated_at = time::now();

CREATE person:testperson2 SET
    username = "testuser2",
    email = "test2@example.com",
    password_hash = "dummy_hash",
    profile = {
        name: "Test User Two",
        bio: "Test bio",
        location: "Test City"
    },
    created_at = time::now(),
    updated_at = time::now();

-- Create test organization
CREATE organization:testorg1 SET
    name = "Test Organization",
    slug = "test-org",
    description = "Test organization for membership testing",
    org_type = organization_type:0gykpc4o7fq0402dr6ie,
    location = "Test Location",
    website = "https://test.org",
    public = false,
    created_at = time::now(),
    updated_at = time::now();

-- Create member_of relationships
RELATE person:testperson1->member_of->organization:testorg1 SET
    role = "owner",
    permissions = ["UpdateOrganization", "DeleteOrganization", "InviteMembers", "RemoveMembers", "UpdateMemberRoles", "CreateProjects", "UpdateProjects", "DeleteProjects", "ManageContent", "PublishContent"],
    invitation_status = "accepted",
    joined_at = time::now();

RELATE person:testperson2->member_of->organization:testorg1 SET
    role = "member",
    permissions = ["CreateProjects", "UpdateProjects", "ManageContent"],
    invitation_status = "accepted",
    invited_by = person:testperson1,
    invited_at = time::now() - 1d,
    joined_at = time::now();

-- Test Query 1: Find membership by person and organization (mimics find_by_person_and_org)
SELECT
    id,
    in as person_id,
    out as organization_id,
    role,
    permissions,
    joined_at,
    invitation_status,
    invited_by,
    invited_at
FROM member_of
WHERE in = person:testperson1 AND out = organization:testorg1;

-- Test Query 2: Get all memberships for an organization (mimics get_organization_memberships)
SELECT
    id,
    in as person_id,
    out as organization_id,
    role,
    permissions,
    joined_at,
    invitation_status,
    invited_by,
    invited_at
FROM member_of
WHERE out = organization:testorg1
ORDER BY joined_at DESC;

-- Test Query 3: Get all memberships for a person (mimics get_person_memberships)
SELECT
    id,
    in as person_id,
    out as organization_id,
    role,
    permissions,
    joined_at,
    invitation_status,
    invited_by,
    invited_at
FROM member_of
WHERE in = person:testperson1
AND invitation_status = 'accepted'
ORDER BY joined_at DESC;

-- Test Query 4: Organization get_members query
SELECT
    id,
    in as person_id,
    in.username as person_username,
    in.profile.name as person_name,
    role,
    joined_at,
    invitation_status
FROM member_of
WHERE out = type::thing('organization', 'testorg1')
ORDER BY
    role DESC,
    person_name ASC;

-- Test Query 5: Get user organizations (from organization model)
SELECT
    out as org_id,
    role,
    joined_at
FROM member_of
WHERE in = person:testperson1
AND invitation_status = 'accepted'
ORDER BY joined_at DESC;

-- Clean up test data
DELETE person:testperson1;
DELETE person:testperson2;
DELETE organization:testorg1;
DELETE member_of WHERE in = person:testperson1;
DELETE member_of WHERE in = person:testperson2;
DELETE member_of WHERE out = organization:testorg1;

-- Output success message
RETURN "All membership query tests completed successfully!";
