use crate::db::DB;
use crate::error::Error;
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

/// Involvement with production details (for profile display via graph traversal)
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct InvolvementWithProduction {
    pub id: RecordId,
    pub role: Option<String>,
    pub relation_type: String,
    pub department: Option<String>,
    pub credit_type: Option<String>,
    pub verification_status: String,
    pub source: String,
    // Production fields from out.*
    pub production_id: RecordId,
    pub production_title: String,
    pub production_slug: String,
    pub production_type: String,
    pub poster_url: Option<String>,
    pub tmdb_url: Option<String>,
    pub tmdb_id: Option<i64>,
    pub media_type: Option<String>,
    pub release_date: Option<String>,
    pub is_claimed: bool,
    // Computed sort field (not used in display)
    pub has_no_date: Option<bool>,
}

/// Involvement with person details (for production detail via reverse graph traversal)
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct InvolvementWithPerson {
    pub id: RecordId,
    pub role: Option<String>,
    pub relation_type: String,
    pub department: Option<String>,
    pub credit_type: Option<String>,
    pub verification_status: String,
    pub person_id: RecordId,
    pub person_name: Option<String>,
    pub person_username: String,
    pub person_avatar: Option<String>,
}

pub struct InvolvementModel;

/// Parse a "table:key" string into a RecordId
fn to_record_id(id: &str) -> RecordId {
    if id.contains(':') {
        let parts: Vec<&str> = id.splitn(2, ':').collect();
        RecordId::new(parts[0], parts[1])
    } else {
        RecordId::new("person", id)
    }
}

impl InvolvementModel {
    /// Create an involvement edge: person->involvement->production
    pub async fn create(
        person_id: &str,
        production_id: &RecordId,
        relation_type: &str,
        role: Option<&str>,
        department: Option<&str>,
        credit_type: Option<&str>,
        source: &str,
    ) -> Result<(), Error> {
        // Determine verification_status from source
        let verification_status = match source {
            "tmdb_import" => "externally_sourced",
            "claimed" => "verified",
            _ => "self_asserted",
        };

        debug!(
            "Creating involvement: {} -> {:?} (role={:?}, source={}, status={})",
            person_id,
            production_id,
            role,
            source,
            verification_status
        );

        let person_rid = to_record_id(person_id);

        let query = r#"
            RELATE $person->involvement->$production SET
                relation_type = $relation_type,
                role = $role,
                department = $department,
                credit_type = $credit_type,
                source = $source,
                verification_status = $verification_status,
                timestamp = time::now()
        "#;

        DB.query(query)
            .bind(("person", person_rid))
            .bind(("production", production_id.clone()))
            .bind(("relation_type", relation_type.to_string()))
            .bind(("role", role.map(|s| s.to_string())))
            .bind(("department", department.map(|s| s.to_string())))
            .bind(("credit_type", credit_type.map(|s| s.to_string())))
            .bind(("source", source.to_string()))
            .bind(("verification_status", verification_status.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to create involvement: {}", e)))?;

        Ok(())
    }

    /// Get all involvements for a person with production details (graph traversal using out.*)
    pub async fn get_for_person(person_id: &str) -> Result<Vec<InvolvementWithProduction>, Error> {
        debug!("Fetching involvements for person: {}", person_id);

        let person_rid = to_record_id(person_id);

        let query = r#"
            SELECT
                id,
                role,
                relation_type,
                department,
                credit_type,
                verification_status,
                source,
                out.id AS production_id,
                out.title AS production_title,
                out.slug AS production_slug,
                out.`type` AS production_type,
                out.poster_url AS poster_url,
                out.tmdb_url AS tmdb_url,
                out.tmdb_id AS tmdb_id,
                out.media_type AS media_type,
                out.release_date AS release_date,
                count(out<-member_of[WHERE role = 'owner']) > 0 AS is_claimed,
                out.release_date IS NONE AS has_no_date
            FROM involvement
            WHERE in = $person
                AND verification_status != 'rejected'
            ORDER BY has_no_date DESC, release_date DESC, production_title ASC
        "#;

        let mut result = DB
            .query(query)
            .bind(("person", person_rid))
            .await
            .map_err(|e| {
                Error::Database(format!("Failed to fetch person involvements: {}", e))
            })?;

        let involvements: Vec<InvolvementWithProduction> = result.take(0)?;
        debug!("Found {} involvements for person", involvements.len());
        Ok(involvements)
    }

    /// Get all involvements for a production with person details (reverse graph traversal using in.*)
    pub async fn get_for_production(
        production_id: &RecordId,
    ) -> Result<Vec<InvolvementWithPerson>, Error> {
        debug!("Fetching involvements for production: {:?}", production_id);

        let query = r#"
            SELECT
                id,
                role,
                relation_type,
                department,
                credit_type,
                verification_status,
                in.id AS person_id,
                in.name AS person_name,
                in.username AS person_username,
                in.profile.avatar AS person_avatar
            FROM involvement
            WHERE out = $production_id
                AND verification_status != 'rejected'
            ORDER BY relation_type ASC, role ASC
        "#;

        let mut result = DB
            .query(query)
            .bind(("production_id", production_id.clone()))
            .await
            .map_err(|e| {
                Error::Database(format!("Failed to fetch production involvements: {}", e))
            })?;

        let involvements: Vec<InvolvementWithPerson> = result.take(0)?;
        Ok(involvements)
    }

    /// Get pending/self-asserted credits for a production (for owner review)
    pub async fn get_pending_for_production(
        production_id: &RecordId,
    ) -> Result<Vec<InvolvementWithPerson>, Error> {
        debug!(
            "Fetching pending involvements for production: {:?}",
            production_id
        );

        let query = r#"
            SELECT
                id,
                role,
                relation_type,
                department,
                credit_type,
                verification_status,
                in.id AS person_id,
                in.name AS person_name,
                in.username AS person_username,
                in.profile.avatar AS person_avatar
            FROM involvement
            WHERE out = $production_id
                AND verification_status IN ['self_asserted', 'pending_verification']
            ORDER BY role ASC
        "#;

        let mut result = DB
            .query(query)
            .bind(("production_id", production_id.clone()))
            .await
            .map_err(|e| {
                Error::Database(format!("Failed to fetch pending involvements: {}", e))
            })?;

        let involvements: Vec<InvolvementWithPerson> = result.take(0)?;
        Ok(involvements)
    }

    /// Check if an involvement already exists (dedup)
    pub async fn exists(
        person_id: &str,
        production_id: &RecordId,
        role: Option<&str>,
    ) -> Result<bool, Error> {
        let person_rid = to_record_id(person_id);

        let query = r#"
            SELECT count() AS count FROM involvement
            WHERE in = $person
                AND out = $production_id
                AND role = $role
        "#;

        let mut result = DB
            .query(query)
            .bind(("person", person_rid))
            .bind(("production_id", production_id.clone()))
            .bind(("role", role.map(|s| s.to_string())))
            .await
            .map_err(|e| Error::Database(format!("Failed to check involvement exists: {}", e)))?;

        let count: Option<serde_json::Value> = result.take(0)?;
        if let Some(obj) = count {
            if let Some(c) = obj.get("count") {
                return Ok(c.as_u64().unwrap_or(0) > 0);
            }
        }
        Ok(false)
    }

    /// Delete an involvement edge
    pub async fn delete(involvement_id: &str) -> Result<(), Error> {
        debug!("Deleting involvement: {}", involvement_id);

        let rid = to_record_id(involvement_id);

        DB.query("DELETE $rid")
            .bind(("rid", rid))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete involvement: {}", e)))?;

        Ok(())
    }

    /// Verify a credit (set verification_status = "verified")
    pub async fn verify(involvement_id: &str, verified_by: &str) -> Result<(), Error> {
        debug!(
            "Verifying involvement {} by {}",
            involvement_id, verified_by
        );

        let inv_rid = to_record_id(involvement_id);
        let verifier_rid = to_record_id(verified_by);

        let query = r#"
            UPDATE $rid SET
                verification_status = 'verified',
                verified_by = $verifier,
                verified_at = time::now()
        "#;

        DB.query(query)
            .bind(("rid", inv_rid))
            .bind(("verifier", verifier_rid))
            .await
            .map_err(|e| Error::Database(format!("Failed to verify involvement: {}", e)))?;

        Ok(())
    }

    /// Reject a credit (set verification_status = "rejected")
    pub async fn reject(involvement_id: &str, rejected_by: &str) -> Result<(), Error> {
        debug!(
            "Rejecting involvement {} by {}",
            involvement_id, rejected_by
        );

        let inv_rid = to_record_id(involvement_id);
        let verifier_rid = to_record_id(rejected_by);

        let query = r#"
            UPDATE $rid SET
                verification_status = 'rejected',
                verified_by = $verifier,
                verified_at = time::now()
        "#;

        DB.query(query)
            .bind(("rid", inv_rid))
            .bind(("verifier", verifier_rid))
            .await
            .map_err(|e| Error::Database(format!("Failed to reject involvement: {}", e)))?;

        Ok(())
    }

    /// Get the production ID for an involvement (for auth checks)
    pub async fn get_production_id(involvement_id: &str) -> Result<Option<RecordId>, Error> {
        let inv_rid = to_record_id(involvement_id);

        let query = "SELECT out FROM $rid";

        let mut result = DB
            .query(query)
            .bind(("rid", inv_rid))
            .await
            .map_err(|e| Error::Database(format!("Failed to get involvement production: {}", e)))?;

        let row: Option<serde_json::Value> = result.take(0)?;
        if let Some(obj) = row {
            if let Some(out) = obj.get("out").and_then(|v| v.as_str()) {
                let parts: Vec<&str> = out.splitn(2, ':').collect();
                if parts.len() == 2 {
                    return Ok(Some(RecordId::new(parts[0], parts[1])));
                }
            }
        }
        Ok(None)
    }
}
