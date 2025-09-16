//! Membership model for managing organization-person relationships
//!
//! This module handles the graph relationships between people and organizations,
//! including roles, permissions, and invitation management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use tracing::{debug, error};

use crate::{db::DB, error::Error};

// ============================
// Data Structures
// ============================

/// Represents a membership relationship between a person and an organization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Membership {
    pub id: String,
    pub person_id: String,
    pub organization_id: String,
    pub role: MembershipRole,
    pub permissions: Vec<Permission>,
    pub joined_at: DateTime<Utc>,
    pub invitation_status: InvitationStatus,
    pub invited_by: Option<String>,
    pub invited_at: Option<DateTime<Utc>>,
}

/// Membership roles within an organization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MembershipRole {
    Owner,
    Admin,
    Member,
}

impl MembershipRole {
    pub fn as_str(&self) -> &str {
        match self {
            MembershipRole::Owner => "owner",
            MembershipRole::Admin => "admin",
            MembershipRole::Member => "member",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Error> {
        match s.to_lowercase().as_str() {
            "owner" => Ok(MembershipRole::Owner),
            "admin" => Ok(MembershipRole::Admin),
            "member" => Ok(MembershipRole::Member),
            _ => Err(Error::validation(format!("Invalid role: {}", s))),
        }
    }
}

/// Permissions that can be granted to members
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
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

/// Status of a membership invitation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Declined,
}

impl InvitationStatus {
    pub fn as_str(&self) -> &str {
        match self {
            InvitationStatus::Pending => "pending",
            InvitationStatus::Accepted => "accepted",
            InvitationStatus::Declined => "declined",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Error> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(InvitationStatus::Pending),
            "accepted" => Ok(InvitationStatus::Accepted),
            "declined" => Ok(InvitationStatus::Declined),
            _ => Err(Error::validation(format!(
                "Invalid invitation status: {}",
                s
            ))),
        }
    }
}

/// Data for creating a new membership
#[derive(Debug)]
pub struct CreateMembershipData {
    pub person_id: String,
    pub organization_id: String,
    pub role: MembershipRole,
    pub permissions: Vec<Permission>,
    pub invitation_status: InvitationStatus,
    pub invited_by: Option<String>,
}

/// Data for updating a membership
#[derive(Debug)]
pub struct UpdateMembershipData {
    pub role: Option<MembershipRole>,
    pub permissions: Option<Vec<Permission>>,
}

// ============================
// Model Implementation
// ============================

pub struct MembershipModel;

impl MembershipModel {
    pub fn new() -> Self {
        Self
    }

    /// Create a new membership relationship
    pub async fn create(&self, data: CreateMembershipData) -> Result<Membership, Error> {
        debug!(
            "Creating membership between person {} and organization {} with role {:?}",
            data.person_id, data.organization_id, data.role
        );

        // Check if membership already exists
        let existing = self
            .find_by_person_and_org(&data.person_id, &data.organization_id)
            .await?;

        if existing.is_some() {
            return Err(Error::Conflict(
                "Membership already exists for this person and organization".to_string(),
            ));
        }

        // Create the relationship using RELATE
        let query = if let Some(ref _inviter) = data.invited_by {
            format!(
                "RELATE organization:{} -> organization_members -> person:{} SET
                    role = $role,
                    permissions = $permissions,
                    invitation_status = $status,
                    invited_by = $inviter,
                    invited_at = time::now(),
                    joined_at = time::now()
                RETURN AFTER",
                data.organization_id, data.person_id
            )
        } else {
            format!(
                "RELATE organization:{} -> organization_members -> person:{} SET
                    role = $role,
                    permissions = $permissions,
                    invitation_status = $status,
                    joined_at = time::now()
                RETURN AFTER",
                data.organization_id, data.person_id
            )
        };

        let result: Option<Membership> = DB
            .query(query)
            .bind(("role", data.role.as_str().to_string()))
            .bind(("permissions", data.permissions.clone()))
            .bind(("status", data.invitation_status.as_str().to_string()))
            .bind(("inviter", data.invited_by.clone()))
            .await
            .map_err(|e| {
                error!("Failed to create membership: {}", e);
                Error::database(format!("Failed to create membership: {}", e))
            })?
            .take("AFTER")?;

        result.ok_or_else(|| {
            error!("Membership creation returned no record");
            Error::database("Failed to create membership - no record returned")
        })
    }

    /// Find a membership by person and organization
    pub async fn find_by_person_and_org(
        &self,
        person_id: &str,
        org_id: &str,
    ) -> Result<Option<Membership>, Error> {
        debug!(
            "Finding membership for person {} in organization {}",
            person_id, org_id
        );

        let query = "SELECT * FROM organization_members
                     WHERE in = organization:$org AND out = person:$person";

        let result: Option<Membership> = DB
            .query(query)
            .bind(("org", org_id.to_string()))
            .bind(("person", person_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to find membership: {}", e);
                Error::database(format!("Failed to find membership: {}", e))
            })?
            .take(0)?;

        Ok(result)
    }

    /// Update a membership
    pub async fn update(&self, id: &str, data: UpdateMembershipData) -> Result<Membership, Error> {
        debug!("Updating membership: {}", id);

        let mut updates = Vec::new();
        let mut bindings: Vec<(&str, String)> = Vec::new();

        if let Some(role) = data.role {
            updates.push("role = $role");
            bindings.push(("role", role.as_str().to_string()));
        }

        if let Some(permissions) = data.permissions {
            updates.push("permissions = $permissions");
            let permissions_json = serde_json::to_value(permissions)
                .map_err(|e| Error::database(format!("Failed to serialize permissions: {}", e)))?;
            bindings.push(("permissions", permissions_json.to_string()));
        }

        if updates.is_empty() {
            return Err(Error::validation("No fields to update".to_string()));
        }

        let query = format!(
            "UPDATE organization_members:{} SET {} RETURN AFTER",
            id,
            updates.join(", ")
        );

        let mut query_builder = DB.query(query);
        for (key, value) in bindings {
            query_builder = query_builder.bind((key, value.to_string()));
        }

        let result: Option<Membership> = query_builder
            .await
            .map_err(|e| {
                error!("Failed to update membership: {}", e);
                Error::database(format!("Failed to update membership: {}", e))
            })?
            .take("AFTER")?;

        result.ok_or(Error::NotFound)
    }

    /// Accept a membership invitation
    pub async fn accept_invitation(&self, id: &str) -> Result<Membership, Error> {
        debug!("Accepting membership invitation: {}", id);

        let query = "UPDATE organization_members:$id SET
                     invitation_status = 'accepted',
                     joined_at = time::now()
                     RETURN AFTER";

        let result: Option<Membership> = DB
            .query(query)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to accept invitation: {}", e);
                Error::database(format!("Failed to accept invitation: {}", e))
            })?
            .take("AFTER")?;

        result.ok_or(Error::NotFound)
    }

    /// Decline a membership invitation
    pub async fn decline_invitation(&self, id: &str) -> Result<(), Error> {
        debug!("Declining membership invitation: {}", id);

        let query = "UPDATE organization_members:$id SET
                     invitation_status = 'declined'";

        DB.query(query)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to decline invitation: {}", e);
                Error::database(format!("Failed to decline invitation: {}", e))
            })?;

        Ok(())
    }

    /// Delete a membership
    pub async fn delete(&self, id: &str) -> Result<(), Error> {
        debug!("Deleting membership: {}", id);

        let query = "DELETE organization_members:$id";

        DB.query(query)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to delete membership: {}", e);
                Error::database(format!("Failed to delete membership: {}", e))
            })?;

        Ok(())
    }

    /// Get all memberships for an organization
    pub async fn get_organization_memberships(
        &self,
        org_id: &str,
    ) -> Result<Vec<Membership>, Error> {
        debug!("Fetching memberships for organization: {}", org_id);

        let query = "SELECT * FROM organization_members
                     WHERE in = organization:$org
                     ORDER BY joined_at DESC";

        let result: Vec<Membership> = DB
            .query(query)
            .bind(("org", org_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to fetch organization memberships: {}", e);
                Error::database(format!("Failed to fetch organization memberships: {}", e))
            })?
            .take(0)
            .unwrap_or_default();

        Ok(result)
    }

    /// Get all memberships for a person
    pub async fn get_person_memberships(&self, person_id: &str) -> Result<Vec<Membership>, Error> {
        debug!("Fetching memberships for person: {}", person_id);

        let query = "SELECT * FROM organization_members
                     WHERE out = person:$person
                     AND invitation_status = 'accepted'
                     ORDER BY joined_at DESC";

        let result: Vec<Membership> = DB
            .query(query)
            .bind(("person", person_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to fetch person memberships: {}", e);
                Error::database(format!("Failed to fetch person memberships: {}", e))
            })?
            .take(0)
            .unwrap_or_default();

        Ok(result)
    }

    /// Check if a person has a specific role in an organization
    pub async fn has_role(
        &self,
        person_id: &str,
        org_id: &str,
        role: MembershipRole,
    ) -> Result<bool, Error> {
        let membership = self.find_by_person_and_org(person_id, org_id).await?;

        Ok(membership.map(|m| m.role == role).unwrap_or(false))
    }

    /// Check if a person has a specific permission in an organization
    pub async fn has_permission(
        &self,
        person_id: &str,
        org_id: &str,
        permission: Permission,
    ) -> Result<bool, Error> {
        let membership = self.find_by_person_and_org(person_id, org_id).await?;

        if let Some(membership) = membership {
            // Owners have all permissions
            if membership.role == MembershipRole::Owner {
                return Ok(true);
            }

            // Check specific permissions
            Ok(membership.permissions.contains(&permission))
        } else {
            Ok(false)
        }
    }

    /// Get default permissions for a role
    pub fn get_default_permissions(role: &MembershipRole) -> Vec<Permission> {
        match role {
            MembershipRole::Owner => vec![
                Permission::UpdateOrganization,
                Permission::DeleteOrganization,
                Permission::InviteMembers,
                Permission::RemoveMembers,
                Permission::UpdateMemberRoles,
                Permission::CreateProjects,
                Permission::UpdateProjects,
                Permission::DeleteProjects,
                Permission::ManageContent,
                Permission::PublishContent,
            ],
            MembershipRole::Admin => vec![
                Permission::UpdateOrganization,
                Permission::InviteMembers,
                Permission::RemoveMembers,
                Permission::CreateProjects,
                Permission::UpdateProjects,
                Permission::DeleteProjects,
                Permission::ManageContent,
                Permission::PublishContent,
            ],
            MembershipRole::Member => vec![
                Permission::CreateProjects,
                Permission::UpdateProjects,
                Permission::ManageContent,
            ],
        }
    }
}

// ============================
// Tests
// ============================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_conversion() {
        assert_eq!(
            MembershipRole::from_str("owner").unwrap(),
            MembershipRole::Owner
        );
        assert_eq!(
            MembershipRole::from_str("admin").unwrap(),
            MembershipRole::Admin
        );
        assert_eq!(
            MembershipRole::from_str("member").unwrap(),
            MembershipRole::Member
        );
        assert!(MembershipRole::from_str("invalid").is_err());
    }

    #[test]
    fn test_invitation_status_conversion() {
        assert_eq!(
            InvitationStatus::from_str("pending").unwrap(),
            InvitationStatus::Pending
        );
        assert_eq!(
            InvitationStatus::from_str("accepted").unwrap(),
            InvitationStatus::Accepted
        );
        assert_eq!(
            InvitationStatus::from_str("declined").unwrap(),
            InvitationStatus::Declined
        );
        assert!(InvitationStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_default_permissions() {
        let owner_perms = MembershipModel::get_default_permissions(&MembershipRole::Owner);
        assert!(owner_perms.contains(&Permission::DeleteOrganization));

        let admin_perms = MembershipModel::get_default_permissions(&MembershipRole::Admin);
        assert!(admin_perms.contains(&Permission::InviteMembers));
        assert!(!admin_perms.contains(&Permission::DeleteOrganization));

        let member_perms = MembershipModel::get_default_permissions(&MembershipRole::Member);
        assert!(member_perms.contains(&Permission::CreateProjects));
        assert!(!member_perms.contains(&Permission::InviteMembers));
    }
}
