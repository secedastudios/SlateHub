# Migration Summary: Renaming 'value' to 'name' Field

## Date: 2024

## Issue
The field name `value` was being used in multiple enum tables in the SurrealDB schema, but `value` is a reserved word in SurrealDB. This needed to be renamed to `name` to avoid conflicts.

## Affected Tables
The following enum tables had their `value` field renamed to `name`:
- organization_type
- production_type
- production_status
- involvement_relation_type
- role
- department
- phase
- credit_type
- ownership_type
- gender
- ethnicity
- hair_color
- eye_color
- body_type
- union

## Changes Made

### 1. Database Schema (`/db/schema.surql`)
- **Field Definitions**: Changed all `DEFINE FIELD value` to `DEFINE FIELD name`
- **Index Definitions**: Updated all indexes from `idx_*_value` to `idx_*_name`
- **Insert Statements**: Changed all `INSERT INTO table (value)` to `INSERT INTO table (name)`
- **Comment Update**: Updated schema comment from "add a 'value' field" to "add a 'name' field"

### 2. Backend Models (`/server/src/models/organization.rs`)
- **get_organization_types() method**: 
  - Changed return type from `Vec<String>` to `Vec<(String, String)>` to return both ID and name
  - Updated query from `SELECT value FROM organization_type` to `SELECT id, name FROM organization_type`
  - Now returns tuples of (id, name) for proper dropdown population

### 3. Route Handlers (`/server/src/routes/organizations.rs`)
- **Added OrgType struct**: New struct with `id` and `name` fields for template data
- **Updated template structs**: Changed `org_types: Vec<String>` to `org_types: Vec<OrgType>` in:
  - OrganizationsListTemplate
  - NewOrganizationTemplate
  - EditOrganizationTemplate
- **Updated route handlers**: Modified list_organizations, new_organization_page, and edit_organization_page to map database results to OrgType structs

### 4. HTML Templates
- **organizations/new.html**: Updated dropdown to use `org_type.id` for value and `org_type.name` for display
- **organizations/list.html**: Updated filter dropdown to use `org_type.id` for value and `org_type.name` for display
- **organizations/edit.html**: 
  - Updated dropdown to use `org_type.id` for value and `org_type.name` for display
  - Fixed duplicate closing `</select>` tag
  - Corrected conditional syntax for selected option

## Implementation Details

### Dropdown Behavior
As per requirements, all dropdowns that reference these enum tables now:
- Use the record ID for the `value` attribute
- Use the name field for the display text
- Example: `<option value="{{ org_type.id }}">{{ org_type.name }}</option>`

### Data Storage
- Organizations continue to store the type as a string in their `type` field
- The stored value is now the record ID from the organization_type table (e.g., "organization_type:production_company")
- This maintains referential integrity while keeping data denormalized for performance

### Backward Compatibility
- The change from storing the name to storing the ID in the organization's type field may require data migration for existing records
- New organizations created after this change will use the ID-based system

## Testing
Created test script (`test_schema_changes.sql`) to verify:
- All enum tables can be queried with the new `name` field
- The `value` field no longer exists
- Organization type lookups work correctly
- Joins between organizations and organization_type tables function properly

## Benefits
1. **Compliance**: Avoids using SurrealDB reserved word `value`
2. **Consistency**: All enum tables now use the same field naming convention
3. **Flexibility**: Using IDs allows enum display names to be changed without affecting stored references
4. **Best Practice**: Follows database design best practices for lookup tables

## Potential Follow-up Tasks
1. Run data migration script to update existing organization records if needed
2. Apply similar pattern to other enum-like fields in the system
3. Add validation to ensure only valid enum IDs are accepted during form submission
4. Consider adding caching for enum table lookups to improve performance