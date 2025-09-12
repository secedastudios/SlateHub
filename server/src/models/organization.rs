use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tracing::debug;

use crate::{db::DB, error::Error};

// ============================
// Data Structures
// ============================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(rename = "type")]
    pub org_type: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub logo: Option<String>,
    pub contact_email: Option<String>,
    pub phone: Option<String>,
    pub services: Vec<String>,
    pub founded_year: Option<i32>,
    pub employees_count: Option<i32>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub id: String,
    pub person_id: String,
    pub person_username: String,
    pub person_name: Option<String>,
    pub role: String,
    pub joined_at: String,
    pub invitation_status: String,
}

#[derive(Debug)]
pub struct CreateOrganizationData {
    pub name: String,
    pub slug: String,
    pub org_type: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub contact_email: Option<String>,
    pub phone: Option<String>,
    pub services: Vec<String>,
    pub founded_year: Option<i32>,
    pub created_by: String,
}

#[derive(Debug)]
pub struct UpdateOrganizationData {
    pub name: String,
    pub org_type: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub contact_email: Option<String>,
    pub phone: Option<String>,
    pub services: Vec<String>,
    pub founded_year: Option<i32>,
    pub employees_count: Option<i32>,
}

// ============================
// Model Implementation
// ============================

pub struct OrganizationModel;

impl OrganizationModel {
    pub fn new() -> Self {
        Self
    }

    /// Create a new organization
    pub async fn create(&self, data: CreateOrganizationData) -> Result<Organization, Error> {
        debug!("Creating organization with slug: {}", data.slug);

        // Check if slug is available
        let (available, reason) = self.check_slug_availability(&data.slug).await?;
        if !available {
            return Err(Error::validation(
                reason.unwrap_or("Slug not available".to_string()),
            ));
        }

        // Create the organization
        let result: Option<Organization> = DB
            .query(
                "CREATE organization SET
                    name = $name,
                    slug = $slug,
                    `type` = $org_type,
                    description = $description,
                    location = $location,
                    website = $website,
                    contact_email = $contact_email,
                    phone = $phone,
                    services = $services,
                    founded_year = $founded_year,
                    created_by = type::thing('person', $created_by),
                    created_at = time::now(),
                    updated_at = time::now()
                RETURN *",
            )
            .bind(("name", data.name.clone()))
            .bind(("slug", data.slug.clone()))
            .bind(("org_type", data.org_type.clone()))
            .bind(("description", data.description.clone()))
            .bind(("location", data.location.clone()))
            .bind(("website", data.website.clone()))
            .bind(("contact_email", data.contact_email.clone()))
            .bind(("phone", data.phone.clone()))
            .bind(("services", data.services.clone()))
            .bind(("founded_year", data.founded_year))
            .bind(("created_by", data.created_by.clone()))
            .await
            .map_err(|e| Error::database(format!("Failed to create organization: {}", e)))?
            .take(0)?;

        result.ok_or(Error::database("Failed to create organization"))
    }

    /// Get organization by slug
    pub async fn get_by_slug(&self, slug: &str) -> Result<Organization, Error> {
        debug!("Fetching organization by slug: {}", slug);

        let result: Option<Organization> = DB
            .query("SELECT * FROM organization WHERE slug = $slug")
            .bind(("slug", slug.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to fetch organization: {}", e)))?
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

        let mut sql = "SELECT * FROM organization".to_string();
        let mut conditions = Vec::new();

        if let Some(q) = query {
            conditions.push(format!("(name ~ '{}' OR description ~ '{}')", q, q));
        }

        if let Some(ot) = org_type {
            conditions.push(format!("`type` = '{}'", ot));
        }

        if let Some(loc) = location {
            conditions.push(format!("location ~ '{}'", loc));
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT 50");

        let organizations: Vec<Organization> = DB
            .query(&sql)
            .await
            .map_err(|e| Error::database(format!("Failed to search organizations: {}", e)))?
            .take(0)
            .unwrap_or_default();

        Ok(organizations)
    }

    /// Update an existing organization
    pub async fn update(&self, id: &str, data: UpdateOrganizationData) -> Result<(), Error> {
        debug!("Updating organization: {}", id);

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
                    updated_at = time::now()
                RETURN *",
            )
            .bind(("id", id.to_string()))
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
            .await
            .map_err(|e| Error::database(format!("Failed to update organization: {}", e)))?
            .take(0)?;

        Ok(())
    }

    /// Delete an organization and all its relationships
    pub async fn delete(&self, id: &str) -> Result<(), Error> {
        debug!("Deleting organization: {}", id);

        // Delete all memberships first
        let _: Vec<()> = DB
            .query("DELETE organization_members WHERE out = type::thing('organization', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to delete memberships: {}", e)))?
            .take(0)
            .unwrap_or_default();

        // Delete the organization
        let _: Vec<()> = DB
            .query("DELETE type::thing('organization', $id)")
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to delete organization: {}", e)))?
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

        let invitation_status = if role == "owner" {
            "accepted"
        } else if invited_by.is_some() {
            "pending"
        } else {
            "accepted"
        };

        let query = if let Some(inviter) = invited_by {
            DB
                .query(
                    "RELATE type::thing('person', $person)->organization_members->type::thing('organization', $org) SET
                        role = $role,
                        joined_at = time::now(),
                        invitation_status = $status,
                        invited_by = type::thing('person', $inviter),
                        invited_at = time::now()"
                )
                .bind(("inviter", inviter.to_string()))
        } else {
            DB
                .query(
                    "RELATE type::thing('person', $person)->organization_members->type::thing('organization', $org) SET
                        role = $role,
                        joined_at = time::now(),
                        invitation_status = $status"
                )
        };

        let _: Option<()> = query
            .bind(("person", person_id.to_string()))
            .bind(("org", org_id.to_string()))
            .bind(("role", role.to_string()))
            .bind(("status", invitation_status))
            .await
            .map_err(|e| Error::database(format!("Failed to add member: {}", e)))?
            .take(0)?;

        Ok(())
    }

    /// Remove a member from an organization
    pub async fn remove_member(&self, membership_id: &str) -> Result<(), Error> {
        debug!("Removing membership: {}", membership_id);

        let _: Vec<()> = DB
            .query("DELETE type::thing('organization_members', $id)")
            .bind(("id", membership_id.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to remove member: {}", e)))?
            .take(0)
            .unwrap_or_default();

        Ok(())
    }

    /// Get all members of an organization
    pub async fn get_members(&self, org_id: &str) -> Result<Vec<OrganizationMember>, Error> {
        debug!("Fetching members for organization: {}", org_id);

        let members_query: Vec<(
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            String,
        )> = DB
            .query(
                "SELECT
                    id,
                    in.id as person_id,
                    in.username as person_username,
                    in.profile.name as person_name,
                    role,
                    joined_at,
                    invitation_status
                FROM organization_members
                WHERE out = type::thing('organization', $org_id)
                ORDER BY
                    CASE role
                        WHEN 'owner' THEN 1
                        WHEN 'admin' THEN 2
                        ELSE 3
                    END,
                    joined_at DESC",
            )
            .bind(("org_id", org_id.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to fetch members: {}", e)))?
            .take(0)
            .unwrap_or_default();

        let members: Vec<OrganizationMember> = members_query
            .into_iter()
            .map(
                |(
                    id,
                    person_id,
                    person_username,
                    person_name,
                    role,
                    joined_at,
                    invitation_status,
                )| {
                    OrganizationMember {
                        id,
                        person_id,
                        person_username,
                        person_name,
                        role,
                        joined_at,
                        invitation_status,
                    }
                },
            )
            .collect();

        Ok(members)
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

        let result: Vec<(String,)> = DB
            .query(
                "SELECT role FROM organization_members
                WHERE in = type::thing('person', $person)
                AND out = type::thing('organization', $org)
                AND invitation_status = 'accepted'",
            )
            .bind(("person", person_id.to_string()))
            .bind(("org", org_id.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to check membership: {}", e)))?
            .take(0)
            .unwrap_or_default();

        Ok(result.first().map(|(role,)| role.clone()))
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

        let _: Option<()> = DB
            .query(
                "UPDATE type::thing('organization_members', $id) SET
                    role = $role",
            )
            .bind(("id", membership_id.to_string()))
            .bind(("role", new_role.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to update member role: {}", e)))?
            .take(0)?;

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
            .await
            .map_err(|e| Error::database(format!("Failed to check slug: {}", e)))?
            .take(0)
            .unwrap_or_default();

        if !org_check.is_empty() {
            return Ok((false, Some("This name is already taken".to_string())));
        }

        // Check against reserved names
        let reserved_check: Vec<(String,)> = DB
            .query("SELECT name FROM reserved_names WHERE name = $name")
            .bind(("name", slug.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to check reserved names: {}", e)))?
            .take(0)
            .unwrap_or_default();

        if !reserved_check.is_empty() {
            return Ok((false, Some("This name is reserved".to_string())));
        }

        Ok((true, None))
    }

    /// Get all organization types with ID and name
    pub async fn get_organization_types(&self) -> Result<Vec<(String, String)>, Error> {
        debug!("Fetching organization types");

        let types: Vec<(String, String)> = DB
            .query("SELECT id, name FROM organization_type ORDER BY name")
            .await
            .map_err(|e| Error::database(format!("Failed to fetch organization types: {}", e)))?
            .take(0)
            .unwrap_or_default();

        debug!("Fetched {} organization types: {:?}", types.len(), types);

        Ok(types)
    }

    /// Find a user by username or email
    pub async fn find_user_by_username_or_email(&self, identifier: &str) -> Result<String, Error> {
        debug!("Finding user by identifier: {}", identifier);

        let result: Vec<(Thing,)> = DB
            .query(
                "SELECT id FROM person WHERE username = $identifier OR email = $identifier LIMIT 1",
            )
            .bind(("identifier", identifier.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to find user: {}", e)))?
            .take(0)
            .unwrap_or_default();

        result
            .first()
            .map(|(id,)| id.id.to_string())
            .ok_or(Error::NotFound)
    }

    /// Get all organizations a user is a member of
    pub async fn get_user_organizations(
        &self,
        user_id: &str,
    ) -> Result<Vec<(Organization, String, String)>, Error> {
        debug!("Fetching organizations for user: {}", user_id);

        let result: Vec<(Organization, String, String)> = DB
            .query(
                "SELECT
                    out.* as organization,
                    role,
                    joined_at
                FROM organization_members
                WHERE in = type::thing('person', $user_id)
                AND invitation_status = 'accepted'
                ORDER BY joined_at DESC",
            )
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| Error::database(format!("Failed to fetch user organizations: {}", e)))?
            .take(0)
            .unwrap_or_default();

        Ok(result)
    }
}
