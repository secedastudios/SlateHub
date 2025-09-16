# Organization Membership Refactoring

## Overview

This document describes the refactoring of the organization creation process and membership management system in SlateHub. The changes move from an embedded person record approach to a pure graph relationship model using SurrealDB's RELATE functionality.

## Changes Made

### 1. Database Schema Updates

#### Organization Table
- Removed `created_by` field entirely from the organization table
- Creator information is now tracked exclusively through the membership relation
- Maintains referential integrity through the graph relationship model

#### Organization Members Relation
- Relation direction: `FROM organization TO person` (organization HAS members)
- Added `permissions` field as `array<string>` for granular permission control
- Maintains existing fields: `role`, `joined_at`, `invitation_status`, `invited_by`, `invited_at`
- Unique index on `(in, out)` prevents duplicate memberships between same org and person

### 2. Organization Model Refactoring

#### Create Method with Transaction Support
Organization creation and membership assignment are now wrapped in a database transaction for atomicity:

```rust
// Transaction ensures both operations succeed or both fail
DB.query("BEGIN TRANSACTION").await?;

// Create the organization
let org_query = r#"
    CREATE organization CONTENT {
        name: $name,
        slug: $slug,
        type: $org_type,
        // ... other fields
    } RETURN AFTER
"#;

// Create the membership relation
let membership_query = format!(r#"
    RELATE organization:{} -> organization_members -> person:{} SET
        role = 'owner',
        permissions: $permissions,
        invitation_status = 'accepted',
        joined_at = time::now()
"#, org.id, created_by);

// Commit on success or rollback on error
match result {
    Ok(org) => DB.query("COMMIT TRANSACTION").await?,
    Err(e) => DB.query("CANCEL TRANSACTION").await
}
```

#### Transaction Benefits
- **Atomicity**: Organization and membership are created together or not at all
- **Consistency**: No orphaned organizations without owners
- **Automatic Rollback**: Failures automatically roll back all changes
- **Data Integrity**: Guaranteed creator-owner relationship

### 3. New Membership Model

Created a dedicated `membership.rs` model to handle organization-person relationships:

#### Key Components

**Data Structures:**
- `Membership` - Represents a membership relationship
- `MembershipRole` - Enum for Owner, Admin, Member
- `Permission` - Enum for granular permissions
- `InvitationStatus` - Enum for Pending, Accepted, Declined

**Core Methods:**
- `create()` - Creates a new membership using RELATE (organization -> person)
- `find_by_person_and_org()` - Finds existing membership
- `update()` - Updates role and permissions
- `accept_invitation()` / `decline_invitation()` - Manages invitations
- `has_permission()` - Checks if a person has a specific permission
- `has_role()` - Checks if a person has a specific role

The relation prevents duplicate memberships through a unique index on the relation fields.

#### Permission System

The new permission system provides granular control:

```rust
pub enum Permission {
    // Organization management
    UpdateOrganization,
    DeleteOrganization,
    
    // Member management
    InviteMembers,
    RemoveMembers,
    UpdateMemberRoles,
    
    // Project management
    CreateProjects,
    UpdateProjects,
    DeleteProjects,
    
    // Content management
    ManageContent,
    PublishContent,
}
```

Default permissions are automatically assigned based on role:
- **Owner**: All permissions
- **Admin**: All except DeleteOrganization
- **Member**: CreateProjects, UpdateProjects, ManageContent

### 4. Graph Relationship Benefits

The pure graph model provides several advantages:

1. **Flexible Relationships**: Easy to add new edge attributes without modifying the organization or person tables
2. **Efficient Queries**: Can traverse relationships directly using SurrealDB's graph capabilities
3. **Data Integrity**: Relationships are managed at the database level with unique constraints
4. **Scalability**: Better performance for complex permission checks and membership queries
5. **Clean Design**: No redundant data storage - creator tracked only through membership relation
6. **Transactional Safety**: Organization creation and membership assignment are atomic operations

## Usage Examples

### Creating an Organization

```rust
let create_data = CreateOrganizationData {
    name: "My Production Company".to_string(),
    slug: "my-production-company".to_string(),
    org_type: "production_company".to_string(),
    // ... other fields
};

// Pass the creator's user ID as a separate parameter
// The create method handles both organization and membership in a transaction
let org = OrganizationModel::new().create(create_data, user_id).await?;
// Organization is created with owner membership atomically
```

### Checking Permissions

```rust
let membership_model = MembershipModel::new();

// Check if user can delete organization
let can_delete = membership_model
    .has_permission(user_id, org_id, Permission::DeleteOrganization)
    .await?;

// Check if user is an admin
let is_admin = membership_model
    .has_role(user_id, org_id, MembershipRole::Admin)
    .await?;
```

### Adding a New Member

```rust
let membership_data = CreateMembershipData {
    person_id: new_member_id.to_string(),
    organization_id: org_id.to_string(),
    role: MembershipRole::Member,
    permissions: MembershipModel::get_default_permissions(&MembershipRole::Member),
    invitation_status: InvitationStatus::Pending,
    invited_by: Some(inviter_id.to_string()),
};

let membership = MembershipModel::new().create(membership_data).await?;
```

## Migration Notes

For existing organizations:
1. The `created_by` field should be removed from organization records
2. Ensure a corresponding membership record exists with role "owner" for the creator
3. Update relation direction: organization_members should be FROM organization TO person
4. The unique index prevents duplicate memberships automatically

## Testing

Integration tests are included but require a test database connection:
- `test_create_organization_with_membership` - Verifies organization creation with automatic owner membership
- Tests verify permission assignment and role-based access control

## Future Enhancements

1. **Custom Permissions**: Allow organizations to define custom permissions
2. **Permission Templates**: Create reusable permission sets for common roles
3. **Audit Trail**: Track permission changes and membership modifications
4. **Bulk Operations**: Add/remove multiple members at once
5. **Delegation**: Allow owners to delegate specific permissions temporarily

## Code Quality

The refactoring follows Rust best practices:
- Simple, idiomatic Rust code
- Clear separation of concerns
- Proper error handling with descriptive messages
- Comprehensive logging for debugging
- Type-safe enums for roles and permissions
- Transactional integrity for critical operations
- Automatic rollback on failures