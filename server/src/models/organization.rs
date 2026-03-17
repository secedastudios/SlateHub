use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{debug, error, warn};

use crate::{
    db::DB,
    error::Error,
    models::membership::{MembershipModel, MembershipRole},
    record_id_ext::RecordIdExt,
    services::embedding::build_organization_embedding_text,
};

// ============================
// Data Structures
// ============================

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct SocialLink {
    pub platform: String,
    pub url: String,
}

/// Represents an organization type with its full RecordId
/// The id field contains the complete reference (e.g., "organization_type:abc123")
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct OrganizationType {
    pub id: RecordId,
    pub name: String,
}

/// Organization entity with all RecordId references properly typed
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Organization {
    pub id: RecordId, // Full RecordId (e.g., "organization:xyz789")
    pub name: String,
    pub slug: String,
    #[serde(rename = "type")]
    #[surreal(rename = "type")]
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
    #[serde(default)]
    #[surreal(default)]
    pub verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct OrganizationMember {
    pub id: RecordId,
    pub person_id: RecordId,
    pub person_username: String,
    pub person_name: Option<String>,
    pub person_avatar: Option<String>,
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
        debug!("Validating organization type: {}", org_type_id.display());
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

        let org_type_id: RecordId = RecordId::parse_simple(&data.org_type).map_err(|e| Error::BadRequest(e.to_string()))?;
        let owner_id: RecordId = RecordId::parse_simple(created_by).map_err(|e| Error::BadRequest(e.to_string()))?;

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
        debug!("Organization type '{}' is valid", org_type_id.display());

        debug!(
            "Creating organization with data: name={}, slug={}, type={}",
            data.name, data.slug, org_type_id.display()
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

        // Transaction creates the org and owner membership atomically
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

            COMMIT TRANSACTION;
            "#;

        debug!("Executing transaction to create organization and owner membership");

        let response = DB
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

        // Check the transaction response for errors.
        // Use debug format to inspect all statement results for constraint violations.
        let response_debug = format!("{:?}", response);
        debug!("Create organization response: {}", response_debug);

        if response_debug.contains("already contains") {
            return Err(Error::conflict("This slug is already taken"));
        }
        if response_debug.contains("cancelled transaction") {
            return Err(Error::database(
                "Organization creation transaction failed",
            ));
        }

        // Fetch the created org with type details in a separate query
        let org: Option<Organization> = DB
            .query("SELECT *, type.* FROM organization WHERE slug = $slug")
            .bind(("slug", data.slug.clone()))
            .await?
            .take(0)?;

        let org = org.ok_or_else(|| {
            error!("Organization creation returned no record");
            Error::database("Failed to create organization - no record returned")
        })?;

        debug!(
            "Successfully created organization '{}' with ID: {} and owner membership",
            data.slug,
            org.id.display()
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

        let id: RecordId = RecordId::parse_simple(id).map_err(|e| Error::BadRequest(e.to_string()))?;

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
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Organization>, Error> {
        debug!("Searching organizations with filters");

        let mut sql = "SELECT *, type.* FROM organization".to_string();
        let mut conditions = Vec::new();

        if query.is_some() {
            conditions.push("(string::lowercase(name) CONTAINS string::lowercase($query) OR string::lowercase(description ?? '') CONTAINS string::lowercase($query))".to_string());
        }

        if org_type.is_some() {
            conditions.push("type.name = $org_type".to_string());
        }

        if location.is_some() {
            conditions.push("string::lowercase(location ?? '') CONTAINS string::lowercase($location)".to_string());
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", limit));
        if offset > 0 {
            sql.push_str(&format!(" START {}", offset));
        }

        let mut result = DB.query(&sql);
        if let Some(q) = query {
            result = result.bind(("query", q.to_string()));
        }
        if let Some(ot) = org_type {
            result = result.bind(("org_type", ot.to_string()));
        }
        if let Some(loc) = location {
            result = result.bind(("location", loc.to_string()));
        }

        let organizations: Vec<Organization> = result.await?.take(0).unwrap_or_default();

        Ok(organizations)
    }

    /// Update an existing organization
    pub async fn update(&self, id: &str, data: UpdateOrganizationData) -> Result<(), Error> {
        debug!("Updating organization: {}", id);
        let id: RecordId = RecordId::parse_simple(id).map_err(|e| Error::BadRequest(e.to_string()))?;
        let org_type_id: RecordId = RecordId::parse_simple(&data.org_type)
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        // Build embedding text for background update
        let embedding_text = build_organization_embedding_text(
            &data.name,
            &data.org_type,
            data.description.as_deref(),
            &data.services,
            data.location.as_deref(),
            data.founded_year,
            data.employees_count,
        );

        DB.query(
                "UPDATE $id SET
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
            .bind(("id", id.clone()))
            .bind(("name", data.name))
            .bind(("org_type", org_type_id))
            .bind(("description", data.description))
            .bind(("location", data.location))
            .bind(("website", data.website))
            .bind(("contact_email", data.contact_email))
            .bind(("phone", data.phone))
            .bind(("services", data.services))
            .bind(("founded_year", data.founded_year))
            .bind(("employees_count", data.employees_count))
            .bind(("public", data.public))
            .await?;

        // Fire-and-forget embedding update
        crate::services::embedding::spawn_embedding_update(id, embedding_text);

        Ok(())
    }

    /// Delete an organization and all its relationships
    pub async fn delete(&self, id: &str) -> Result<(), Error> {
        debug!("Deleting organization: {}", id);

        let id: RecordId = RecordId::parse_simple(id).map_err(|e| Error::BadRequest(e.to_string()))?;

        // Delete all memberships first
        let _: Vec<()> = DB
            .query("DELETE member_of WHERE out = $id")
            .bind(("id", id.clone()))
            .await?
            .take(0)
            .unwrap_or_default();

        // Delete the organization
        let _: Vec<()> = DB
            .query("DELETE $id")
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

        let person_id: RecordId = RecordId::parse_simple(person_id).map_err(|e| Error::BadRequest(e.to_string()))?;
        let org_id: RecordId = RecordId::parse_simple(org_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        let invitation_status = if role == "owner" {
            "accepted"
        } else if invited_by.is_some() {
            "pending"
        } else {
            "accepted"
        };

        let query = if let Some(inviter) = invited_by {
            let inviter_rid = RecordId::parse_simple(inviter)
                .map_err(|e| Error::BadRequest(e.to_string()))?;
            DB.query(
                "RELATE $person->member_of->$org SET
                        role = $role,
                        invitation_status = $status,
                        invited_by = $inviter",
            )
            .bind(("inviter", inviter_rid))
        } else {
            DB.query(
                "RELATE $person->member_of->$org SET
                        role = $role,
                        invitation_status = $status",
            )
        };

        query
            .bind(("org", org_id))
            .bind(("person", person_id))
            .bind(("role", role.to_string()))
            .bind(("status", invitation_status))
            .await?;

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

        let org_record_id = RecordId::parse_simple(org_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        let result: Vec<OrganizationMember> = DB
            .query(
                "SELECT
                    id,
                    in as person_id,
                    in.username as person_username,
                    in.profile.name as person_name,
                    in.profile.avatar as person_avatar,
                    role,
                    joined_at,
                    invitation_status
                FROM member_of
                WHERE out = $org_id
                ORDER BY
                    role DESC,
                    in.profile.name ASC",
            )
            .bind(("org_id", org_record_id))
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
            .filter(|m| m.invitation_status == "accepted")
            .map(|m| m.role.clone()))
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
        let org_check: Vec<serde_json::Value> = DB
            .query("SELECT slug FROM organization WHERE slug = $slug")
            .bind(("slug", slug.to_string()))
            .await?
            .take(0)
            .unwrap_or_default();

        if !org_check.is_empty() {
            return Ok((false, Some("This name is already taken".to_string())));
        }

        // Check against reserved names
        let reserved_check: Vec<serde_json::Value> = DB
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
        #[derive(Debug, Deserialize, SurrealValue)]
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
            .map(|record| (record.id.to_raw_string(), record.name))
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

        #[derive(Debug, serde::Deserialize, SurrealValue)]
        struct PersonId {
            id: RecordId,
        }

        let result: Option<PersonId> = DB
            .query(
                "SELECT id FROM person WHERE username = $identifier OR email = $identifier LIMIT 1",
            )
            .bind(("identifier", identifier.to_string()))
            .await?
            .take(0)?;

        result
            .map(|p| p.id.to_raw_string())
            .ok_or(Error::NotFound)
    }

    /// Get all organizations a user is a member of
    pub async fn get_user_organizations(
        &self,
        user_id: &str,
    ) -> Result<Vec<(Organization, String, String)>, Error> {
        debug!("=== Starting get_user_organizations ===");
        debug!("Fetching organizations for user_id: '{}'", user_id);

        let user_id: RecordId = RecordId::parse_simple(user_id).map_err(|e| Error::BadRequest(e.to_string()))?;

        // First get the organization relationships
        // user_id should already be a full record ID like "person:xyz"
        #[derive(Debug, Deserialize, SurrealValue)]
        struct MemberRel {
            org_id: RecordId,
            role: String,
            joined_at: DateTime<Utc>,
        }

        let query = "
            SELECT
                out as org_id,
                role,
                joined_at
            FROM member_of
            WHERE in = $user_id
            AND <string> type::table(out) = 'organization'
            AND invitation_status = 'accepted'
            ORDER BY joined_at DESC";

        debug!("Executing relationship query with user_id: '{}'", user_id.display());

        let relationships: Vec<MemberRel> = DB
            .query(query)
            .bind(("user_id", user_id.clone()))
            .await
            .map_err(|e| {
                error!("Failed to query member_of: {:?}", e);
                e
            })?
            .take(0)
            .unwrap_or_default();

        debug!(
            "Query returned {} organization relationships for user '{}'",
            relationships.len(),
            user_id.display()
        );

        if relationships.is_empty() {
            debug!("No organization memberships found. Possible causes:");
            debug!("  1. User has no organization memberships");
            debug!(
                "  2. User ID mismatch (check if '{}' exists in DB)",
                user_id.display()
            );
            debug!("  3. invitation_status is not 'accepted'");
        } else {
            debug!("Found relationships:");
            for rel in &relationships {
                debug!("  - Org: {}, Role: {}, Joined: {}", rel.org_id.display(), rel.role, rel.joined_at);
            }
        }

        // Now fetch each organization
        let mut result = Vec::new();
        debug!(
            "Fetching organization details for {} relationships",
            relationships.len()
        );

        for rel in relationships {
            debug!("Fetching organization: {}", rel.org_id.display());
            let org_query = "SELECT *, type.* FROM organization WHERE id = $id";

            let org: Option<Organization> = DB
                .query(org_query)
                .bind(("id", rel.org_id.clone()))
                .await
                .map_err(|e| {
                    error!("Failed to fetch organization {}: {:?}", rel.org_id.display(), e);
                    e
                })?
                .take(0)?;

            if let Some(org) = org {
                debug!(
                    "Successfully fetched organization: {} ({})",
                    org.name, org.slug
                );
                result.push((org, rel.role, rel.joined_at.to_rfc3339()));
            } else {
                warn!("Organization {} not found in database", rel.org_id.display());
            }
        }

        result.sort_by(|(a, _, _), (b, _, _)| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        debug!(
            "=== Completed get_user_organizations: returning {} organizations ===",
            result.len()
        );
        Ok(result)
    }

    /// Get person IDs of all owners of an organization
    pub async fn get_org_owners(&self, org_id: &str) -> Result<Vec<String>, Error> {
        let org_rid = surrealdb::types::RecordId::parse_simple(org_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        #[derive(Debug, serde::Deserialize, surrealdb::types::SurrealValue)]
        struct OwnerId {
            person_id: String,
        }

        let results: Vec<OwnerId> = DB
            .query("SELECT <string> in AS person_id FROM member_of WHERE out = $org_id AND role = 'owner'")
            .bind(("org_id", org_rid))
            .await?
            .take(0)
            .unwrap_or_default();

        Ok(results.into_iter().map(|o| o.person_id).collect())
    }
}
