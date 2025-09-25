-- Fix Membership Permissions Script
-- This script fixes incorrect permission values in existing membership records
-- Run this against the SurrealDB database to fix serialization errors

-- Fix any memberships with the old/incorrect permission values
UPDATE membership
SET permissions = [
    "update_organization",
    "delete_organization",
    "invite_members",
    "remove_members",
    "update_member_roles",
    "create_projects",
    "update_projects",
    "delete_projects",
    "manage_content",
    "publish_content"
]
WHERE role = 'owner';

-- Fix any memberships with typos like "creeate"
UPDATE membership
SET permissions = array::distinct(
    array::map(permissions, |$p| {
        IF $p = "create" OR $p = "creeate" THEN
            "create_projects"
        ELSE IF $p = "Update" THEN
            "update_organization"
        ELSE IF $p = "Delete" THEN
            "delete_organization"
        ELSE IF $p = "InviteMembers" THEN
            "invite_members"
        ELSE IF $p = "RemoveMembers" THEN
            "remove_members"
        ELSE IF $p = "UpdateMemberRoles" THEN
            "update_member_roles"
        ELSE IF $p = "Publish" THEN
            "publish_content"
        ELSE
            $p
        END
    })
)
WHERE permissions CONTAINS "create"
   OR permissions CONTAINS "creeate"
   OR permissions CONTAINS "Update"
   OR permissions CONTAINS "Delete"
   OR permissions CONTAINS "InviteMembers"
   OR permissions CONTAINS "RemoveMembers"
   OR permissions CONTAINS "UpdateMemberRoles"
   OR permissions CONTAINS "Publish";

-- Set default permissions for admin role (if any exist)
UPDATE membership
SET permissions = [
    "update_organization",
    "invite_members",
    "remove_members",
    "update_member_roles",
    "create_projects",
    "update_projects",
    "delete_projects",
    "manage_content",
    "publish_content"
]
WHERE role = 'admin' AND (
    permissions = [] OR
    permissions = NONE OR
    array::len(permissions) = 0
);

-- Set default permissions for member role (if any exist)
UPDATE membership
SET permissions = [
    "create_projects",
    "update_projects",
    "manage_content"
]
WHERE role = 'member' AND (
    permissions = [] OR
    permissions = NONE OR
    array::len(permissions) = 0
);

-- Verify the fix by selecting all unique permission values
SELECT array::distinct(array::flatten(
    (SELECT permissions FROM membership)
)) AS all_permissions;

-- Show count of fixed records
SELECT
    role,
    count() as total,
    array::distinct(array::flatten(permissions)) as unique_permissions
FROM membership
GROUP BY role;
