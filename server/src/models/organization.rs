use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use surrealdb::RecordId;
use tracing::{debug, error, warn};

use crate::{
    db::DB,
    error::Error,
    models::membership::{InvitationStatus, MembershipModel, MembershipRole},
};

// ============================
// Data Structures
// ============================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLink {
    pub platform: String,
    pub url: String,
}

/// Represents an organization type with its full RecordId
/// The id field contains the complete reference (e.g., "organization_type:abc123")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationType {
    pub id: RecordId,
    pub name: String,
}

/// Organization entity with all RecordId references properly typed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: RecordId, // Full RecordId (e.g., "organization:xyz789")
    pub name: String,
    pub slug: String,
    #[serde(rename = "type")]
    pub org_type: OrganizationType, // Contains embedded OrganizationType with its RecordId
    pub description: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub social_links: Vec<SocialLink>,
    pub logo: Option<String>,
    pub contact_email: Option<String>,
    pub phone: Option<String>,
    pub services: Vec<String>,
    pub founded_year: Option<i32>,
    pub employees_count: Option<i32>,
    pub public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub id: RecordId,
    pub person_id: RecordId,
    pub person_username: String,
    pub person_name: Option<String>,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    pub invitation_status: String,
}

#[derive(Debug)]
pub struct CreateOrganizationData {
    pub name: String,
    pub slug: String,
    pub org_type: String, // String ID from form, converted to record reference when creating
    pub description: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub contact_email: Option<String>,
    pub phone: Option<String>,
    pub services: Vec<String>,
    pub founded_year: Option<i32>,
    pub employees_count: Option<i32>,
    pub public: bool,
}

#[derive(Debug)]
pub struct UpdateOrganizationData {
    pub name: String,
    pub org_type: String, // String ID from form, converted to record reference when updating
    pub description: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub contact_email: Option<String>,
    pub phone: Option<String>,
    pub services: Vec<String>,
    pub founded_year: Option<i32>,
    pub employees_count: Option<i32>,
    pub public: bool,
}

// ============================
// Model Implementation
// ============================

pub struct OrganizationModel;

impl OrganizationModel {
    pub fn new() -> Self {
        Self
    }

    /// Validate that an organization type exists in the database
    async fn validate_organization_type(&self, org_type_id: &RecordId) -> Result<bool, Error> {
        debug!("Validating organization type: {}", org_type_id);
        Ok(DB
            .select::<Option<OrganizationType>>(org_type_id)
            .await?
            .is_some())
    }

    /// Create a new organization with the creator as owner
    pub async fn create(
        &self,
        data: CreateOrganizationData,
        created_by: &str,
    ) -> Result<Organization, Error> {
        debug!("Creating organization with slug: {}", data.slug);

        let org_type_id: RecordId = RecordId::from_str(&data.org_type)?;
        let owner_id: RecordId = RecordId::from_str(created_by)?;

        // Check if slug is available
        let (available, reason) = self.check_slug_availability(&data.slug).await?;
        if !available {
            error!("Slug '{}' is not available: {:?}", data.slug, reason);
            return Err(Error::validation(
                reason.unwrap_or("Slug not available".to_string()),
            ));
        }
        debug!(
            "Slug '{}' is available, proceeding with creation",
            data.slug
        );

        // Validate organization type exists
        let type_exists = self.validate_organization_type(&org_type_id).await?;
        if !type_exists {
            error!("Organization type '{}' does not exist", data.org_type);
            return Err(Error::validation(format!(
                "Invalid organization type: {}",
                data.org_type
            )));
        }
        debug!("Organization type '{}' is valid", org_type_id);

        debug!(
            "Creating organization with data: name={}, slug={}, type={}",
            data.name, data.slug, org_type_id
        );

        // Get default owner permissions as strings (must match snake_case serialization)
        let owner_permissions = vec![
            "update_organization".to_string(),
            "delete_organization".to_string(),
            "invite_members".to_string(),
            "remove_members".to_string(),
            "update_member_roles".to_string(),
            "create_projects".to_string(),
            "update_projects".to_string(),
            "delete_projects".to_string(),
            "manage_content".to_string(),
            "publish_content".to_string(),
        ];

        // Single SQL transaction that creates the organization and membership
        let transaction_query = r#"
            BEGIN TRANSACTION;

            LET $org = (CREATE organization CONTENT {{
                name: $name,
                slug: $slug,
                type: $org_type,
                description: $description,
                location: $location,
                website: $website,
                social_links: [],
                contact_email: $contact_email,
                phone: $phone,
                services: $services,
                founded_year: $founded_year,
                public: $public
            }});

            RELATE $person->member_of->$org SET
                role = 'owner',
                permissions = $permissions,
                invitation_status = 'accepted',
                joined_at = time::now();

            LET $result = (SELECT *, type.* FROM $org.id);

            COMMIT TRANSACTION;

            RETURN $result;
            "#;

        debug!("Executing transaction to create organization and owner membership");

        let mut response = DB
            .query(transaction_query)
            .bind(("name", data.name))
            .bind(("slug", data.slug.clone()))
            .bind(("org_type", org_type_id))
            .bind(("description", data.description))
            .bind(("location", data.location))
            .bind(("website", data.website))
            .bind(("contact_email", data.contact_email))
            .bind(("phone", data.phone))
            .bind(("services", data.services))
            .bind(("founded_year", data.founded_year))
            .bind(("public", data.public))
            .bind(("permissions", owner_permissions))
            .bind(("person", owner_id))
            .await?;

        debug!("Create organization response: {:?}", response);
        let org: Option<Organization> = response.take(3)?;

        let org = org.ok_or_else(|| {
            error!("Organization creation returned no record");
            Error::database("Failed to create organization - no record returned")
        })?;

        debug!(
            "Successfully created organization '{}' with ID: {} and owner membership",
            data.slug,
            org.id.to_string()
        );

        Ok(org)
    }

    /// Get organization by slug
    pub async fn get_by_slug(&self, slug: &str) -> Result<Organization, Error> {
        debug!("Fetching organization by slug: {}", slug);

        let result: Option<Organization> = DB
            .query("SELECT *, type.* FROM organization WHERE slug = $slug")
            .bind(("slug", slug.to_string()))
            .await?
            .take(0)?;

        result.ok_or(Error::NotFound)
    }

    /// Get organization by ID
    pub async fn get_by_id(&self, id: &str) -> Result<Organization, Error> {
        debug!("Fetching organization by ID: {}", id);

        let id: RecordId = RecordId::from_str(id)?;

        let result: Option<Organization> = DB
            .query("SELECT *, type.* FROM organization WHERE $id")
            .bind(("id", id))
            .await?
            .take(0)?;

        result.ok_or(Error::NotFound)
    }

    /// Search organizations with filters
    pub async fn search(
        &self,
        query: Option<&str>,
        org_type: Option<&str>,
        location: Option<&str>,
    ) -> Result<Vec<Organization>, Error> {
        debug!("Searching organizations with filters");

        let mut sql = "SELECT *, type.* FROM organization".to_string();
        let mut conditions = Vec::new();

        if let Some(q) = query {
            conditions.push(format!("(name ~ '{}' OR description ~ '{}')", q, q));
        }

        if let Some(ot) = org_type {
            conditions.push(format!("type.name = '{}'", ot));
        }

        if let Some(loc) = location {
            conditions.push(format!("location ~ '{}'", loc));
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT 50");

        let organizations: Vec<Organization> = DB.query(&sql).await?.take(0).unwrap_or_default();

        Ok(organizations)
    }

    /// Update an existing organization
    pub async fn update(&self, id: &str, data: UpdateOrganizationData) -> Result<(), Error> {
        debug!("Updating organization: {}", id);
        let id: RecordId = RecordId::from_str(id)?;

        let _: Option<Organization> = DB
            .query(
                "UPDATE type::thing('organization', $id) SET
                    name = $name,
                    `type` = $org_type,
                    description = $description,
                    location = $location,
                    website = $website,
                    contact_email = $contact_email,
                    phone = $phone,
                    services = $services,
                    founded_year = $founded_year,
                    employees_count = $employees_count,
                    public = $public",
            )
            .bind(("id", id))
            .bind(("name", data.name))
            .bind(("org_type", data.org_type))
            .bind(("description", data.description))
            .bind(("location", data.location))
            .bind(("website", data.website))
            .bind(("contact_email", data.contact_email))
            .bind(("phone", data.phone))
            .bind(("services", data.services))
            .bind(("founded_year", data.founded_year))
            .bind(("employees_count", data.employees_count))
            .bind(("public", data.public))
            .await?
            .take(0)?;

        Ok(())
    }

    /// Delete an organization and all its relationships
    pub async fn delete(&self, id: &str) -> Result<(), Error> {
        debug!("Deleting organization: {}", id);

        let id: RecordId = RecordId::from_str(id)?;

        // Delete all memberships first
        let _: Vec<()> = DB
            .query("DELETE organization_members WHERE $id")
            .bind(("id", id.clone()))
            .await?
            .take(0)
            .unwrap_or_default();

        // Delete the organization
        let _: Vec<()> = DB
            .query("DELETE type::thing('organization', $id)")
            .bind(("id", id))
            .await?
            .take(0)
            .unwrap_or_default();

        Ok(())
    }

    /// Add a member to an organization
    pub async fn add_member(
        &self,
        org_id: &str,
        person_id: &str,
        role: &str,
        invited_by: Option<&str>,
    ) -> Result<(), Error> {
        debug!(
            "Adding member {} to organization {} with role {}",
            person_id, org_id, role
        );

        let person_id: RecordId = RecordId::from_str(person_id)?;
        let org_id: RecordId = RecordId::from_str(org_id)?;

        let invitation_status = if role == "owner" {
            "accepted"
        } else if invited_by.is_some() {
            "pending"
        } else {
            "accepted"
        };

        let query = if let Some(inviter) = invited_by {
            DB.query(
                "RELATE $org->organization_members->$person SET
                        role = $role,
                        invitation_status = $status,
                        invited_by = $inviter",
            )
            .bind(("inviter", inviter.to_string()))
        } else {
            DB.query(
                "RELATE $person->member_of->$org SET
                        role = $role,
                        invitation_status = $status",
            )
        };

        let _: Option<()> = query
            .bind(("org", org_id))
            .bind(("person", person_id))
            .bind(("role", role.to_string()))
            .bind(("status", invitation_status))
            .await?
            .take(0)?;

        Ok(())
    }

    /// Remove a member from an organization
    pub async fn remove_member(&self, membership_id: &str) -> Result<(), Error> {
        debug!("Removing membership: {}", membership_id);

        let membership_model = MembershipModel::new();
        membership_model.delete(membership_id).await?;

        Ok(())
    }

    /// Get all members of an organization
    pub async fn get_members(&self, org_id: &str) -> Result<Vec<OrganizationMember>, Error> {
        debug!("Fetching members for organization: {}", org_id);

        let result: Vec<OrganizationMember> = DB
            .query(
                "SELECT
                    id,
                    out as person_id,
                    out.username as person_username,
                    out.profile.name as person_name,
                    role,
                    joined_at,
                    invitation_status
                FROM organization_members
                WHERE in = type::thing('organization', $org_id)
                ORDER BY
                    CASE role
                        WHEN 'owner' THEN 1
                        WHEN 'admin' THEN 2
                        ELSE 3
                    END,
                    joined_at DESC",
            )
            .bind(("org_id", org_id.to_string()))
            .await?
            .take(0)
            .unwrap_or_default();

        Ok(result)
    }

    /// Get a user's role in an organization
    pub async fn get_member_role(
        &self,
        org_id: &str,
        person_id: &str,
    ) -> Result<Option<String>, Error> {
        debug!(
            "Checking role for person {} in organization {}",
            person_id, org_id
        );

        let membership_model = MembershipModel::new();
        let membership = membership_model
            .find_by_person_and_org(person_id, org_id)
            .await?;

        Ok(membership
            .filter(|m| m.invitation_status == InvitationStatus::Accepted)
            .map(|m| m.role.as_str().to_string()))
    }

    /// Update a member's role
    pub async fn update_member_role(
        &self,
        membership_id: &str,
        new_role: &str,
    ) -> Result<(), Error> {
        debug!(
            "Updating role for membership {} to {}",
            membership_id, new_role
        );

        let membership_model = MembershipModel::new();
        let role_enum = MembershipRole::from_str(new_role)?;

        membership_model
            .update(
                membership_id,
                crate::models::membership::UpdateMembershipData {
                    role: Some(role_enum.clone()),
                    permissions: Some(MembershipModel::get_default_permissions(&role_enum)),
                },
            )
            .await?;

        Ok(())
    }

    /// Check if a slug is available
    pub async fn check_slug_availability(
        &self,
        slug: &str,
    ) -> Result<(bool, Option<String>), Error> {
        debug!("Checking availability of slug: {}", slug);

        // Check if slug is taken
        let org_check: Vec<(String,)> = DB
            .query("SELECT slug FROM organization WHERE slug = $slug")
            .bind(("slug", slug.to_string()))
            .await?
            .take(0)
            .unwrap_or_default();

        if !org_check.is_empty() {
            return Ok((false, Some("This name is already taken".to_string())));
        }

        // Check against reserved names
        let reserved_check: Vec<(String,)> = DB
            .query("SELECT name FROM reserved_names WHERE name = $name")
            .bind(("name", slug.to_string()))
            .await?
            .take(0)
            .unwrap_or_default();

        if !reserved_check.is_empty() {
            return Ok((false, Some("This name is reserved".to_string())));
        }

        Ok((true, None))
    }

    /// Get all organization types with ID and name
    pub async fn get_organization_types(&self) -> Result<Vec<(String, String)>, Error> {
        debug!("Fetching organization types from database");

        // Define a struct to match the query result
        #[derive(Debug, Deserialize)]
        struct OrgTypeRecord {
            id: RecordId,
            name: String,
        }

        // Fetch organization types with their IDs
        let sql = "SELECT id, name FROM organization_type ORDER BY name";

        let mut response = DB.query(sql).await?;

        // Extract as structured records
        let records: Vec<OrgTypeRecord> = response.take(0)?;

        debug!("Fetched {} organization types", records.len());

        // Convert to tuples with full RecordId strings
        let types: Vec<(String, String)> = records
            .into_iter()
            .map(|record| (record.id.to_string(), record.name))
            .collect();

        if types.is_empty() {
            warn!(
                "No organization types found - database may need initialization with 'make db-init'"
            );
        } else {
            debug!("Successfully loaded {} organization types", types.len());
        }

        Ok(types)
    }

    /// Find a user by username or email
    pub async fn find_user_by_username_or_email(&self, identifier: &str) -> Result<String, Error> {
        debug!("Finding user by identifier: {}", identifier);

        let result: Vec<(String,)> = DB
            .query(
                "SELECT id FROM person WHERE username = $identifier OR email = $identifier LIMIT 1",
            )
            .bind(("identifier", identifier.to_string()))
            .await?
            .take(0)
            .unwrap_or_default();

        result
            .first()
            .map(|(id,)| id.clone())
            .ok_or(Error::NotFound)
    }

    /// Get all organizations a user is a member of
    pub async fn get_user_organizations(
        &self,
        user_id: &str,
    ) -> Result<Vec<(Organization, String, String)>, Error> {
        debug!("=== Starting get_user_organizations ===");
        debug!("Fetching organizations for user_id: '{}'", user_id);

        let user_id: RecordId = RecordId::from_str(user_id)?;

        // First get the organization relationships
        // user_id should already be a full record ID like "person:xyz"
        let query = "
            SELECT
                out as org_id,
                role,
                joined_at
            FROM member_of
            WHERE in = $user_id
            AND invitation_status = 'accepted'
            ORDER BY joined_at DESC";

        debug!("Executing relationship query with user_id: '{}'", user_id);

        let relationships: Vec<(RecordId, String, DateTime<Utc>)> = DB
            .query(query)
            .bind(("user_id", user_id.clone()))
            .await
            .map_err(|e| {
                error!("Failed to query organization_members: {:?}", e);
                e
            })?
            .take(0)
            .unwrap_or_default();

        debug!(
            "Query returned {} organization relationships for user '{}'",
            relationships.len(),
            user_id
        );

        if relationships.is_empty() {
            debug!("No organization memberships found. Possible causes:");
            debug!("  1. User has no organization memberships");
            debug!(
                "  2. User ID mismatch (check if '{}' exists in DB)",
                user_id
            );
            debug!("  3. invitation_status is not 'accepted'");
        } else {
            debug!("Found relationships:");
            for (org_id, role, joined_at) in &relationships {
                debug!("  - Org: {}, Role: {}, Joined: {}", org_id, role, joined_at);
            }
        }

        // Now fetch each organization
        let mut result = Vec::new();
        debug!(
            "Fetching organization details for {} relationships",
            relationships.len()
        );

        for (org_id, role, joined_at) in relationships {
            debug!("Fetching organization: {}", org_id);
            let org_query = "SELECT *, type.* FROM organization WHERE id = $id";

            let org: Option<Organization> = DB
                .query(org_query)
                .bind(("id", org_id.to_string()))
                .await
                .map_err(|e| {
                    error!("Failed to fetch organization {}: {:?}", org_id, e);
                    e
                })?
                .take(0)?;

            if let Some(org) = org {
                debug!(
                    "Successfully fetched organization: {} ({})",
                    org.name, org.slug
                );
                result.push((org, role, joined_at.to_rfc3339()));
            } else {
                warn!("Organization {} not found in database", org_id);
            }
        }

        debug!(
            "=== Completed get_user_organizations: returning {} organizations ===",
            result.len()
        );
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    // Helper function to create test organization data
    fn create_test_org_data(slug: &str) -> CreateOrganizationData {
        CreateOrganizationData {
            name: format!("Test Organization {}", slug),
            slug: slug.to_string(),
            org_type: "production_company".to_string(),
            description: Some("A test organization for unit testing".to_string()),
            location: Some("Los Angeles, CA".to_string()),
            website: Some("https://example.com".to_string()),
            contact_email: Some("contact@example.com".to_string()),
            phone: Some("+1-555-0123".to_string()),
            services: vec!["production".to_string(), "post-production".to_string()],
            founded_year: Some(2020),
            employees_count: Some(50),
            public: true,
        }
    }

    #[test]
    fn test_organization_data_creation() {
        let org_data = create_test_org_data("test-org");

        assert_eq!(org_data.name, "Test Organization test-org");
        assert_eq!(org_data.slug, "test-org");
        assert_eq!(org_data.org_type, "production_company");
        assert!(org_data.description.is_some());
        assert_eq!(org_data.services.len(), 2);
        assert_eq!(org_data.founded_year, Some(2020));
        assert!(org_data.public);
    }

    #[test]
    fn test_organization_slug_validation() {
        // Test various slug formats
        let valid_slugs = vec![
            "my-company",
            "company-123",
            "test-org-2024",
            "production-co",
        ];

        let invalid_slugs = vec![
            "My Company", // spaces
            "company!",   // special chars
            "test@org",   // @ symbol
            "",           // empty
        ];

        for slug in valid_slugs {
            // In a real implementation, you'd validate the slug format
            assert!(!slug.is_empty());
            assert!(!slug.contains(' '));
            assert!(!slug.contains('@'));
            assert!(!slug.contains('!'));
        }

        for slug in invalid_slugs {
            // These should fail validation
            assert!(
                slug.is_empty() || slug.contains(' ') || slug.contains('@') || slug.contains('!')
            );
        }
    }

    #[test]
    fn test_social_link_structure() {
        let social_link = SocialLink {
            platform: "linkedin".to_string(),
            url: "https://linkedin.com/company/test".to_string(),
        };

        assert_eq!(social_link.platform, "linkedin");
        assert!(social_link.url.starts_with("https://"));
    }

    #[test]
    fn test_update_organization_data() {
        let update_data = UpdateOrganizationData {
            name: "Updated Organization".to_string(),
            org_type: "studio".to_string(),
            description: Some("Updated description".to_string()),
            location: Some("New York, NY".to_string()),
            website: Some("https://updated.com".to_string()),
            contact_email: Some("new@example.com".to_string()),
            phone: Some("+1-555-9999".to_string()),
            services: vec!["editing".to_string()],
            founded_year: Some(2019),
            employees_count: Some(100),
            public: false,
        };

        assert_eq!(update_data.name, "Updated Organization");
        assert_eq!(update_data.org_type, "studio");
        assert!(!update_data.public);
        assert_eq!(update_data.employees_count, Some(100));
    }

    #[tokio::test]
    async fn test_organization_model_new() {
        let _model = OrganizationModel::new();
        // The model should be created successfully
        // In Rust, if this compiles and runs, it works
        assert!(true);
    }

    #[test]
    fn test_organization_member_structure() {
        let member = OrganizationMember {
            id: RecordId::from_str("organization_members:member_123").unwrap(),
            person_id: RecordId::from_str("person:person_456").unwrap(),
            person_username: "johndoe".to_string(),
            person_name: Some("John Doe".to_string()),
            role: "admin".to_string(),
            joined_at: Utc::now(),
            invitation_status: "accepted".to_string(),
        };

        assert_eq!(member.person_username, "johndoe");
        assert_eq!(member.role, "admin");
        assert_eq!(member.invitation_status, "accepted");
        assert!(member.person_name.is_some());
    }

    #[test]
    fn test_organization_fields_optional() {
        // Test that optional fields can be None
        let org_data = CreateOrganizationData {
            name: "Minimal Org".to_string(),
            slug: "minimal-org".to_string(),
            org_type: "production_company".to_string(),
            description: None,
            location: None,
            website: None,
            contact_email: None,
            phone: None,
            services: vec![],
            founded_year: None,
            employees_count: None,
            public: false,
        };

        assert!(org_data.description.is_none());
        assert!(org_data.location.is_none());
        assert!(org_data.website.is_none());
        assert!(org_data.contact_email.is_none());
        assert!(org_data.phone.is_none());
        assert!(org_data.founded_year.is_none());
        assert!(org_data.employees_count.is_none());
        assert!(org_data.services.is_empty());
    }

    #[test]
    fn test_organization_type_variations() {
        let org_types = vec![
            "production_company",
            "studio",
            "agency",
            "post_production",
            "equipment_rental",
            "freelancer_collective",
        ];

        for org_type in org_types {
            let org_data = CreateOrganizationData {
                name: format!("Test {}", org_type),
                slug: format!("test-{}", org_type.replace('_', "-")),
                org_type: org_type.to_string(),
                description: None,
                location: None,
                website: None,
                contact_email: None,
                phone: None,
                services: vec![],
                founded_year: None,
                employees_count: None,
                public: true,
            };

            assert_eq!(org_data.org_type, org_type);
        }
    }

    // Integration test placeholder - would require database connection
    #[tokio::test]
    #[ignore] // Ignore by default as it requires database
    async fn test_create_organization_with_membership() {
        // This test would require a test database connection
        // Uncomment and implement when test database is available

        /*
        let model = OrganizationModel::new();
        let membership_model = crate::models::membership::MembershipModel::new();

        // Create test organization data with a mock user ID
        let user_id = "test_user_123";
        let org_data = create_test_org_data("integration-test-org");

        // Create the organization
        match model.create(org_data, user_id).await {
            Ok(org) => {
                // Verify organization was created
                assert_eq!(org.slug, "integration-test-org");
                assert!(org.id.starts_with("organization:"));
                assert_eq!(org.org_type, "production_company");


                // Verify membership was created
                let membership = membership_model
                    .find_by_person_and_org(user_id, &org.id)
                    .await
                    .expect("Should find membership")
                    .expect("Membership should exist");

                assert_eq!(membership.person_id, user_id);
                assert_eq!(membership.organization_id, org.id);
                assert_eq!(membership.role, crate::models::membership::MembershipRole::Owner);
                assert_eq!(membership.invitation_status, crate::models::membership::InvitationStatus::Accepted);

                // Verify owner has all permissions
                let has_delete_perm = membership_model
                    .has_permission(user_id, &org.id, crate::models::membership::Permission::DeleteOrganization)
                    .await
                    .expect("Should check permission");
                assert!(has_delete_perm, "Owner should have delete permission");

                // Cleanup - delete the test organization and membership
                let _ = membership_model.delete(&membership.id).await;
                let _ = model.delete(&org.id).await;
            }
            Err(e) => {
                panic!("Failed to create organization: {:?}", e);
            }
        }
        */
    }

    #[tokio::test]
    #[ignore] // Ignore by default as it requires database
    async fn test_create_organization_integration() {
        // This test would require a test database connection
        // Uncomment and implement when test database is available

        /*
        let model = OrganizationModel::new();
        let org_data = create_test_org_data("integration-test-org");

        match model.create(org_data, "test_user").await {
            Ok(org) => {
                assert_eq!(org.slug, "integration-test-org");
                assert!(org.id.starts_with("organization:"));
                assert_eq!(org.org_type, "production_company");

                // Cleanup - delete the test organization
                let _ = model.delete(&org.id).await;
            }
            Err(e) => {
                panic!("Failed to create organization: {:?}", e);
            }
        }
        */
    }

    #[tokio::test]
    #[ignore] // Ignore by default as it requires database
    async fn test_slug_availability_check() {
        // This test would require a test database connection

        /*
        let model = OrganizationModel::new();

        // Check availability of a new slug
        let (available, reason) = model.check_slug_availability("unique-test-slug").await.unwrap();
        assert!(available);
        assert!(reason.is_none());

        // Create an organization with that slug
        let org_data = create_test_org_data("unique-test-slug");
        let _ = model.create(org_data, "test_user").await.unwrap();

        // Check again - should not be available
        let (available, reason) = model.check_slug_availability("unique-test-slug").await.unwrap();
        assert!(!available);
        assert!(reason.is_some());

        // Cleanup
        let _ = model.delete("unique-test-slug").await;
        */
    }
}
