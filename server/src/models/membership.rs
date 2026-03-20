//! Membership model for managing organization-person relationships
//!
//! This module handles the graph relationships between people and organizations,
//! including roles, permissions, and invitation management.

use crate::{db::DB, error::Error};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{debug, error};

// ============================
// Data Structures
// ============================

/// Represents a membership relationship between a person and an organization
/// Note: role/invitation_status/permissions use String types because SurrealValue
/// derive does not work on Rust enums in SurrealDB v3.0.1 SDK.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Membership {
    pub id: RecordId,
    pub person_id: RecordId,
    pub organization_id: RecordId,
    pub role: String,
    pub permissions: Vec<String>,
    pub joined_at: DateTime<Utc>,
    pub invitation_status: String,
    pub invited_by: Option<RecordId>,
    pub invited_at: Option<DateTime<Utc>>,
    pub request_note: Option<String>,
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
    Requested,
}

impl InvitationStatus {
    pub fn as_str(&self) -> &str {
        match self {
            InvitationStatus::Pending => "pending",
            InvitationStatus::Accepted => "accepted",
            InvitationStatus::Declined => "declined",
            InvitationStatus::Requested => "requested",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Error> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(InvitationStatus::Pending),
            "accepted" => Ok(InvitationStatus::Accepted),
            "declined" => Ok(InvitationStatus::Declined),
            "requested" => Ok(InvitationStatus::Requested),
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
    pub person_id: String,       // Keep as String since it comes from routes
    pub organization_id: String, // Keep as String since it comes from routes
    pub role: MembershipRole,
    pub permissions: Vec<Permission>,
    pub invitation_status: InvitationStatus,
    pub invited_by: Option<String>, // Keep as String since it comes from routes
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
                "RELATE person:{} -> member_of -> organization:{} SET
                    role = $role,
                    permissions = $permissions,
                    invitation_status = $status,
                    invited_by = $inviter,
                    invited_at = time::now(),
                    joined_at = time::now()
                RETURN AFTER",
                data.person_id, data.organization_id
            )
        } else {
            format!(
                "RELATE person:{} -> member_of -> organization:{} SET
                    role = $role,
                    permissions = $permissions,
                    invitation_status = $status,
                    joined_at = time::now()
                RETURN AFTER",
                data.person_id, data.organization_id
            )
        };

        let permissions_strs: Vec<String> = data
            .permissions
            .iter()
            .map(|p| serde_json::to_string(p).unwrap_or_default().trim_matches('"').to_string())
            .collect();

        let inviter_rid: Option<RecordId> = data
            .invited_by
            .as_deref()
            .map(|id| RecordId::parse_simple(id))
            .transpose()
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        let result: Option<Membership> = DB
            .query(query)
            .bind(("role", data.role.as_str().to_string()))
            .bind(("permissions", permissions_strs))
            .bind(("status", data.invitation_status.as_str().to_string()))
            .bind(("inviter", inviter_rid))
            .await?
            .take(0)?;

        result.ok_or_else(|| {
            error!("Membership creation returned no record");
            Error::database("Failed to create membership - no record returned")
        })
    }

    /// Find a membership by its record ID
    pub async fn find_by_id(&self, id: &str) -> Result<Option<Membership>, Error> {
        debug!("Finding membership by ID: {}", id);

        let record_id = RecordId::parse_simple(id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        let result: Option<Membership> = DB
            .query(
                "SELECT
                    id,
                    in as person_id,
                    out as organization_id,
                    role,
                    permissions,
                    joined_at,
                    invitation_status,
                    invited_by,
                    invited_at,
                    request_note
                 FROM $id",
            )
            .bind(("id", record_id))
            .await?
            .take(0)?;

        Ok(result)
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

        let person_id: RecordId = RecordId::parse_simple(person_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;
        let org_id: RecordId = RecordId::parse_simple(org_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        let query = "SELECT
                        id,
                        in as person_id,
                        out as organization_id,
                        role,
                        permissions,
                        joined_at,
                        invitation_status,
                        invited_by,
                        invited_at,
                        request_note
                     FROM member_of
                     WHERE in = $person AND out = $org";

        let result: Option<Membership> = DB
            .query(query)
            .bind(("person", person_id))
            .bind(("org", org_id))
            .await?
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
            let permissions_json = serde_json::to_value(permissions)?;
            bindings.push(("permissions", permissions_json.to_string()));
        }

        if updates.is_empty() {
            return Err(Error::validation("No fields to update".to_string()));
        }

        let query = format!(
            "UPDATE member_of:{} SET {} RETURN AFTER",
            id,
            updates.join(", ")
        );

        let mut query_builder = DB.query(query);
        for (key, value) in bindings {
            query_builder = query_builder.bind((key, value.to_string()));
        }

        let result: Option<Membership> = query_builder.await?.take(0)?;

        result.ok_or(Error::NotFound)
    }

    /// Accept a membership invitation
    /// `id` should be the full record ID string, e.g. "member_of:xxx"
    pub async fn accept_invitation(&self, id: &str) -> Result<(), Error> {
        debug!("Accepting membership invitation: {}", id);

        let record_id = RecordId::parse_simple(id)
            .map_err(|e| Error::BadRequest(format!("Invalid membership ID '{}': {}", id, e)))?;

        let query = "UPDATE $id SET
                     invitation_status = 'accepted',
                     joined_at = time::now()";

        DB.query(query)
            .bind(("id", record_id))
            .await?;

        Ok(())
    }

    /// Decline a membership invitation
    /// `id` should be the full record ID string, e.g. "member_of:xxx"
    pub async fn decline_invitation(&self, id: &str) -> Result<(), Error> {
        debug!("Declining membership invitation: {}", id);

        let record_id = RecordId::parse_simple(id)
            .map_err(|e| Error::BadRequest(format!("Invalid membership ID '{}': {}", id, e)))?;

        let query = "UPDATE $id SET
                     invitation_status = 'declined'";

        DB.query(query).bind(("id", record_id)).await?;

        Ok(())
    }

    /// Delete a membership
    /// `id` should be the full record ID string, e.g. "member_of:xxx"
    pub async fn delete(&self, id: &str) -> Result<(), Error> {
        debug!("Deleting membership: {}", id);

        let record_id = RecordId::parse_simple(id)
            .map_err(|e| Error::BadRequest(format!("Invalid membership ID '{}': {}", id, e)))?;

        let query = "DELETE $id";

        DB.query(query).bind(("id", record_id)).await?;

        Ok(())
    }

    /// Get all memberships for an organization
    pub async fn get_organization_memberships(
        &self,
        org_id: &str,
    ) -> Result<Vec<Membership>, Error> {
        debug!("Fetching memberships for organization: {}", org_id);

        let org_record_id = RecordId::parse_simple(org_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        let query = "SELECT
                        id,
                        in as person_id,
                        out as organization_id,
                        role,
                        permissions,
                        joined_at,
                        invitation_status,
                        invited_by,
                        invited_at,
                        request_note
                     FROM member_of
                     WHERE out = $org
                     ORDER BY joined_at DESC";

        let result: Vec<Membership> = DB
            .query(query)
            .bind(("org", org_record_id))
            .await?
            .take(0)
            .unwrap_or_default();

        Ok(result)
    }

    /// Get all memberships for a person
    pub async fn get_person_memberships(&self, person_id: &str) -> Result<Vec<Membership>, Error> {
        debug!("Fetching memberships for person: {}", person_id);

        let person_record_id = RecordId::parse_simple(person_id)
            .map_err(|e| Error::BadRequest(e.to_string()))?;

        let query = "SELECT
                        id,
                        in as person_id,
                        out as organization_id,
                        role,
                        permissions,
                        joined_at,
                        invitation_status,
                        invited_by,
                        invited_at,
                        request_note
                     FROM member_of
                     WHERE in = $person
                     AND invitation_status = 'accepted'
                     ORDER BY joined_at DESC";

        let result: Vec<Membership> = DB
            .query(query)
            .bind(("person", person_record_id))
            .await?
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

        Ok(membership.map(|m| m.role == role.as_str()).unwrap_or(false))
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
            if membership.role == "owner" {
                return Ok(true);
            }

            // Check specific permissions
            let perm_str = serde_json::to_string(&permission)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            Ok(membership.permissions.contains(&perm_str))
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

