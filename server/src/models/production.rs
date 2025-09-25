use crate::db::DB;
use crate::error::Error;
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use tracing::debug;

/// Production entity from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Production {
    pub id: RecordId,
    pub title: String,
    pub slug: String,
    #[serde(rename = "type")]
    pub production_type: String,
    pub status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Data required to create a new production
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProductionData {
    pub title: String,
    pub production_type: String,
    pub status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
}

/// Data for updating an existing production
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProductionData {
    pub title: Option<String>,
    pub production_type: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
}

/// Member information for production members
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionMember {
    pub id: String,
    pub name: String,
    pub username: Option<String>, // For persons
    pub slug: Option<String>,     // For organizations
    pub role: String,             // owner, admin, member
    pub member_type: String,      // person or organization
}

/// Production model for database operations
pub struct ProductionModel;

impl ProductionModel {
    /// Create a new production and establish ownership
    pub async fn create(
        data: CreateProductionData,
        creator_id: &str,
        creator_type: &str, // "person" or "organization"
    ) -> Result<Production, Error> {
        debug!(
            "Creating production: {} by {} ({})",
            data.title, creator_id, creator_type
        );

        // Generate slug from title
        let slug = data
            .title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");

        // Start a transaction
        let _response = DB
            .query("BEGIN TRANSACTION")
            .await
            .map_err(|e| Error::Database(format!("Failed to start transaction: {}", e)))?;

        // Create the production
        let query = r#"
            CREATE production CONTENT {
                title: $title,
                slug: $slug,
                type: $type,
                status: $status,
                start_date: $start_date,
                end_date: $end_date,
                description: $description,
                location: $location
            } RETURN *;
        "#;

        let mut result = DB
            .query(query)
            .bind(("title", data.title))
            .bind(("slug", slug))
            .bind(("type", data.production_type))
            .bind(("status", data.status))
            .bind(("start_date", data.start_date))
            .bind(("end_date", data.end_date))
            .bind(("description", data.description))
            .bind(("location", data.location))
            .await
            .map_err(|e| Error::Database(format!("Failed to create production: {}", e)))?;

        let production: Option<Production> = result.take(0)?;
        let production = production.ok_or_else(|| {
            Error::Database("Failed to create production - no result returned".to_string())
        })?;

        // Create ownership relation
        let ownership_query = r#"
            RELATE $creator->member_of->$production SET role = 'owner';
        "#;

        let prod_id = production.id.clone();
        DB.query(ownership_query)
            .bind(("creator", creator_id.to_string()))
            .bind(("production", prod_id.clone()))
            .await
            .map_err(|e| Error::Database(format!("Failed to create ownership relation: {}", e)))?;

        // Commit transaction
        DB.query("COMMIT TRANSACTION")
            .await
            .map_err(|e| Error::Database(format!("Failed to commit transaction: {}", e)))?;

        debug!("Successfully created production: {}", production.id);
        Ok(production)
    }

    /// Get a production by ID
    pub async fn get(production_id: &RecordId) -> Result<Production, Error> {
        debug!("Fetching production: {}", production_id);

        let mut result = DB
            .query("SELECT * FROM $production_id")
            .bind(("production_id", production_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production: {}", e)))?;

        let production: Option<Production> = result.take(0)?;
        production.ok_or_else(|| Error::NotFound)
    }

    /// Get a production by slug
    pub async fn get_by_slug(slug: &str) -> Result<Production, Error> {
        debug!("Fetching production by slug: {}", slug);

        let query = "SELECT * FROM production WHERE slug = $slug";
        let mut result = DB
            .query(query)
            .bind(("slug", slug.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production: {}", e)))?;

        let productions: Vec<Production> = result.take(0)?;
        productions
            .into_iter()
            .next()
            .ok_or_else(|| Error::NotFound)
    }

    /// List all productions with optional filters
    pub async fn list(
        limit: Option<usize>,
        status_filter: Option<&str>,
        type_filter: Option<&str>,
    ) -> Result<Vec<Production>, Error> {
        debug!(
            "Listing productions with filters - status: {:?}, type: {:?}",
            status_filter, type_filter
        );

        let mut query = String::from("SELECT * FROM production WHERE 1=1");

        if status_filter.is_some() {
            query.push_str(" AND status = $status");
        }

        if type_filter.is_some() {
            query.push_str(" AND type = $type");
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let mut db_query = DB.query(&query);

        if let Some(status) = status_filter {
            db_query = db_query.bind(("status", status.to_string()));
        }

        if let Some(prod_type) = type_filter {
            db_query = db_query.bind(("type", prod_type.to_string()));
        }

        let mut result = db_query
            .await
            .map_err(|e| Error::Database(format!("Failed to list productions: {}", e)))?;

        let productions: Vec<Production> = result.take(0)?;
        Ok(productions)
    }

    /// Update a production
    pub async fn update(
        production_id: &RecordId,
        data: UpdateProductionData,
    ) -> Result<Production, Error> {
        debug!("Updating production: {}", production_id);

        let mut update_fields = Vec::new();

        if data.title.is_some() {
            update_fields.push("title = $title");
        }
        if data.production_type.is_some() {
            update_fields.push("type = $type");
        }
        if data.status.is_some() {
            update_fields.push("status = $status");
        }
        if data.start_date.is_some() {
            update_fields.push("start_date = $start_date");
        }
        if data.end_date.is_some() {
            update_fields.push("end_date = $end_date");
        }
        if data.description.is_some() {
            update_fields.push("description = $description");
        }
        if data.location.is_some() {
            update_fields.push("location = $location");
        }

        update_fields.push("updated_at = time::now()");

        let query = format!(
            "UPDATE $production_id SET {} RETURN *",
            update_fields.join(", ")
        );

        let mut db_query = DB
            .query(&query)
            .bind(("production_id", production_id.to_string()));

        if let Some(title) = data.title {
            db_query = db_query.bind(("title", title));
        }
        if let Some(prod_type) = data.production_type {
            db_query = db_query.bind(("type", prod_type));
        }
        if let Some(status) = data.status {
            db_query = db_query.bind(("status", status));
        }
        if let Some(start_date) = data.start_date {
            db_query = db_query.bind(("start_date", start_date));
        }
        if let Some(end_date) = data.end_date {
            db_query = db_query.bind(("end_date", end_date));
        }
        if let Some(description) = data.description {
            db_query = db_query.bind(("description", description));
        }
        if let Some(location) = data.location {
            db_query = db_query.bind(("location", location));
        }

        let mut result = db_query
            .await
            .map_err(|e| Error::Database(format!("Failed to update production: {}", e)))?;

        let production: Option<Production> = result.take(0)?;
        production.ok_or_else(|| Error::NotFound)
    }

    /// Delete a production
    pub async fn delete(production_id: &RecordId) -> Result<(), Error> {
        debug!("Deleting production: {}", production_id);

        // Start transaction
        DB.query("BEGIN TRANSACTION")
            .await
            .map_err(|e| Error::Database(format!("Failed to start transaction: {}", e)))?;

        // Delete all member_of relations to this production
        DB.query("DELETE member_of WHERE out = $production_id")
            .bind(("production_id", production_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete member relations: {}", e)))?;

        // Delete all involvement relations to this production
        DB.query("DELETE involvement WHERE out = $production_id")
            .bind(("production_id", production_id.to_string()))
            .await
            .map_err(|e| {
                Error::Database(format!("Failed to delete involvement relations: {}", e))
            })?;

        // Delete the production
        DB.query("DELETE $production_id")
            .bind(("production_id", production_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete production: {}", e)))?;

        // Commit transaction
        DB.query("COMMIT TRANSACTION")
            .await
            .map_err(|e| Error::Database(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// Get productions for a user or organization
    pub async fn get_member_productions(member_id: &str) -> Result<Vec<Production>, Error> {
        debug!("Fetching productions for member: {}", member_id);

        let query = r#"
            SELECT out.* FROM member_of
            WHERE in = $member_id
            AND type::table(out) = 'production'
            ORDER BY out.created_at DESC
        "#;

        let mut result = DB
            .query(query)
            .bind(("member_id", member_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch member productions: {}", e)))?;

        let productions: Vec<Production> = result.take(0)?;
        Ok(productions)
    }

    /// Get members of a production
    pub async fn get_members(production_id: &RecordId) -> Result<Vec<ProductionMember>, Error> {
        debug!("Fetching members for production: {}", production_id);

        let query = r#"
            SELECT
                in.id as id,
                in.name as name,
                in.username as username,
                in.slug as slug,
                role,
                type::table(in) as member_type
            FROM member_of
            WHERE out = $production_id
            ORDER BY role ASC, in.name ASC
        "#;

        let mut result = DB
            .query(query)
            .bind(("production_id", production_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production members: {}", e)))?;

        let members: Vec<ProductionMember> = result.take(0)?;
        Ok(members)
    }

    /// Check if a user or organization is a member of a production
    pub async fn is_member(production_id: &RecordId, member_id: &str) -> Result<bool, Error> {
        debug!(
            "Checking membership for {} in production {}",
            member_id, production_id
        );

        let query = r#"
            SELECT count() as count FROM member_of
            WHERE in = $member_id AND out = $production_id
        "#;

        let mut result = DB
            .query(query)
            .bind(("member_id", member_id.to_string()))
            .bind(("production_id", production_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to check membership: {}", e)))?;

        let count: Option<serde_json::Value> = result.take(0)?;
        if let Some(count_obj) = count {
            if let Some(count_val) = count_obj.get("count") {
                return Ok(count_val.as_u64().unwrap_or(0) > 0);
            }
        }
        Ok(false)
    }

    /// Check if a user or organization can edit a production (owner or admin)
    pub async fn can_edit(production_id: &RecordId, member_id: &str) -> Result<bool, Error> {
        debug!(
            "Checking edit permission for {} in production {}",
            member_id, production_id
        );

        let query = r#"
            SELECT role FROM member_of
            WHERE in = $member_id AND out = $production_id
        "#;

        let mut result = DB
            .query(query)
            .bind(("member_id", member_id.to_string()))
            .bind(("production_id", production_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to check edit permission: {}", e)))?;

        let member: Option<serde_json::Value> = result.take(0)?;
        if let Some(member_obj) = member {
            if let Some(role) = member_obj.get("role").and_then(|r| r.as_str()) {
                return Ok(role == "owner" || role == "admin");
            }
        }
        Ok(false)
    }

    /// Add a member to a production
    pub async fn add_member(
        production_id: &RecordId,
        member_id: &str,
        role: &str,
    ) -> Result<(), Error> {
        debug!(
            "Adding member {} to production {} with role {}",
            member_id, production_id, role
        );

        let query = r#"
            RELATE $member->member_of->$production SET role = $role
        "#;

        DB.query(query)
            .bind(("member", member_id.to_string()))
            .bind(("production", production_id.to_string()))
            .bind(("role", role.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to add member: {}", e)))?;

        Ok(())
    }

    /// Remove a member from a production
    pub async fn remove_member(production_id: &RecordId, member_id: &str) -> Result<(), Error> {
        debug!(
            "Removing member {} from production {}",
            member_id, production_id
        );

        let query = r#"
            DELETE FROM member_of WHERE in = $member AND out = $production
        "#;

        DB.query(query)
            .bind(("member", member_id.to_string()))
            .bind(("production", production_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to remove member: {}", e)))?;

        Ok(())
    }

    /// Get production types from the database
    pub async fn get_production_types() -> Result<Vec<String>, Error> {
        debug!("Fetching production types");

        let mut result = DB
            .query("SELECT name FROM production_type ORDER BY name")
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production types: {}", e)))?;

        let types: Vec<serde_json::Value> = result.take(0)?;
        Ok(types
            .into_iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
    }

    /// Get production statuses from the database
    pub async fn get_production_statuses() -> Result<Vec<String>, Error> {
        debug!("Fetching production statuses");

        let mut result = DB
            .query("SELECT name FROM production_status ORDER BY name")
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production statuses: {}", e)))?;

        let statuses: Vec<serde_json::Value> = result.take(0)?;
        Ok(statuses
            .into_iter()
            .filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
    }
}
