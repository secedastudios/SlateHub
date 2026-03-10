use crate::db::DB;
use crate::error::Error;
use crate::record_id_ext::RecordIdExt;
use crate::services::embedding::build_production_embedding_text;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

/// Production entity from the database
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Production {
    pub id: RecordId,
    pub title: String,
    pub slug: String,
    #[serde(rename = "type")]
    #[surreal(rename = "type")]
    pub production_type: String,
    pub status: String,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // TMDB / external source
    #[serde(default)]
    #[surreal(default)]
    pub tmdb_id: Option<i64>,
    #[serde(default)]
    #[surreal(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub poster_url: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub tmdb_url: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub overview: Option<String>,
    // Source tracking
    #[serde(default = "default_source")]
    #[surreal(default)]
    pub source: String,
    #[serde(default)]
    #[surreal(default)]
    pub source_overrides: Vec<String>,
}

fn default_source() -> String {
    "manual".to_string()
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
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
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

        // Create the production (embedding generated in background)
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

        let embedding_text = build_production_embedding_text(
            &data.title,
            &data.production_type,
            &data.status,
            data.description.as_deref(),
            data.location.as_deref(),
            data.start_date.as_deref(),
            data.end_date.as_deref(),
        );

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

        // Fire-and-forget embedding update
        crate::services::embedding::spawn_embedding_update(production.id.clone(), embedding_text);

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

        debug!("Successfully created production: {}", production.id.display());
        Ok(production)
    }

    /// Get a production by ID
    pub async fn get(production_id: &RecordId) -> Result<Production, Error> {
        debug!("Fetching production: {}", production_id.display());

        let mut result = DB
            .query("SELECT * FROM $production_id")
            .bind(("production_id", production_id.to_raw_string()))
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
        filter: Option<&str>,
        sort: Option<&str>,
    ) -> Result<Vec<Production>, Error> {
        debug!(
            "Listing productions - status: {:?}, type: {:?}, filter: {:?}, sort: {:?}",
            status_filter, type_filter, filter, sort
        );

        let mut query = String::from("SELECT * FROM production WHERE 1=1");

        if status_filter.is_some() {
            query.push_str(" AND status = $status");
        }

        if type_filter.is_some() {
            query.push_str(" AND type = $type");
        }

        if filter.is_some() {
            query.push_str(
                " AND (string::lowercase(title) CONTAINS string::lowercase($filter) \
                 OR string::lowercase(description ?? '') CONTAINS string::lowercase($filter) \
                 OR string::lowercase(location ?? '') CONTAINS string::lowercase($filter))",
            );
        }

        let order_clause = match sort {
            Some("title") => " ORDER BY title ASC",
            Some("status") => " ORDER BY status ASC, created_at DESC",
            _ => " ORDER BY created_at DESC",
        };
        query.push_str(order_clause);

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

        if let Some(filter) = filter {
            db_query = db_query.bind(("filter", filter.to_string()));
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
        debug!("Updating production: {}", production_id.display());

        // Fetch current production to merge with updates for embedding
        let current = Self::get(production_id).await?;

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

        // Generate embedding with merged data
        let title = data.title.as_ref().unwrap_or(&current.title);
        let production_type = data
            .production_type
            .as_ref()
            .unwrap_or(&current.production_type);
        let status = data.status.as_ref().unwrap_or(&current.status);
        let description = data.description.as_ref().or(current.description.as_ref());
        let location = data.location.as_ref().or(current.location.as_ref());
        let current_start_str = current.start_date.map(|d| d.to_string());
        let current_end_str = current.end_date.map(|d| d.to_string());
        let start_date = data.start_date.as_ref().or(current_start_str.as_ref());
        let end_date = data.end_date.as_ref().or(current_end_str.as_ref());

        let embedding_text = build_production_embedding_text(
            title,
            production_type,
            status,
            description.map(|s| s.as_str()),
            location.map(|s| s.as_str()),
            start_date.map(|s| s.as_str()),
            end_date.map(|s| s.as_str()),
        );

        let query = format!(
            "UPDATE $production_id SET {} RETURN *",
            update_fields.join(", ")
        );

        let mut db_query = DB
            .query(&query)
            .bind(("production_id", production_id.to_raw_string()));

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
        let production = production.ok_or_else(|| Error::NotFound)?;

        // Fire-and-forget embedding update
        crate::services::embedding::spawn_embedding_update(production.id.clone(), embedding_text);

        Ok(production)
    }

    /// Delete a production
    pub async fn delete(production_id: &RecordId) -> Result<(), Error> {
        debug!("Deleting production: {}", production_id.display());

        // Start transaction
        DB.query("BEGIN TRANSACTION")
            .await
            .map_err(|e| Error::Database(format!("Failed to start transaction: {}", e)))?;

        // Delete all member_of relations to this production
        DB.query("DELETE member_of WHERE out = $production_id")
            .bind(("production_id", production_id.to_raw_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete member relations: {}", e)))?;

        // Delete all involvement relations to this production
        DB.query("DELETE involvement WHERE out = $production_id")
            .bind(("production_id", production_id.to_raw_string()))
            .await
            .map_err(|e| {
                Error::Database(format!("Failed to delete involvement relations: {}", e))
            })?;

        // Delete the production
        DB.query("DELETE $production_id")
            .bind(("production_id", production_id.to_raw_string()))
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
        debug!("Fetching members for production: {}", production_id.display());

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
            .bind(("production_id", production_id.to_raw_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production members: {}", e)))?;

        let members: Vec<ProductionMember> = result.take(0)?;
        Ok(members)
    }

    /// Check if a user or organization is a member of a production
    pub async fn is_member(production_id: &RecordId, member_id: &str) -> Result<bool, Error> {
        debug!(
            "Checking membership for {} in production {}",
            member_id, production_id.display()
        );

        let query = r#"
            SELECT count() as count FROM member_of
            WHERE in = $member_id AND out = $production_id
        "#;

        let mut result = DB
            .query(query)
            .bind(("member_id", member_id.to_string()))
            .bind(("production_id", production_id.to_raw_string()))
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
            member_id, production_id.display()
        );

        let query = r#"
            SELECT role FROM member_of
            WHERE in = $member_id AND out = $production_id
        "#;

        let mut result = DB
            .query(query)
            .bind(("member_id", member_id.to_string()))
            .bind(("production_id", production_id.to_raw_string()))
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
            member_id, production_id.display(), role
        );

        let query = r#"
            RELATE $member->member_of->$production SET role = $role
        "#;

        DB.query(query)
            .bind(("member", member_id.to_string()))
            .bind(("production", production_id.to_raw_string()))
            .bind(("role", role.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to add member: {}", e)))?;

        Ok(())
    }

    /// Remove a member from a production
    pub async fn remove_member(production_id: &RecordId, member_id: &str) -> Result<(), Error> {
        debug!(
            "Removing member {} from production {}",
            member_id, production_id.display()
        );

        let query = r#"
            DELETE FROM member_of WHERE in = $member AND out = $production
        "#;

        DB.query(query)
            .bind(("member", member_id.to_string()))
            .bind(("production", production_id.to_raw_string()))
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

    /// Find a production by TMDB ID
    pub async fn find_by_tmdb_id(tmdb_id: i64) -> Result<Option<Production>, Error> {
        debug!("Finding production by tmdb_id: {}", tmdb_id);

        let query = "SELECT * FROM production WHERE tmdb_id = $tmdb_id LIMIT 1";
        let mut result = DB
            .query(query)
            .bind(("tmdb_id", tmdb_id))
            .await
            .map_err(|e| Error::Database(format!("Failed to find production by tmdb_id: {}", e)))?;

        let productions: Vec<Production> = result.take(0)?;
        Ok(productions.into_iter().next())
    }

    /// Find a production by TMDB ID, or create it from TMDB data
    pub async fn find_or_create_from_tmdb(
        tmdb_id: i64,
        title: String,
        media_type: String,
        poster_url: Option<String>,
        tmdb_url: String,
        release_date: Option<String>,
        overview: Option<String>,
    ) -> Result<Production, Error> {
        // Try to find existing
        if let Some(existing) = Self::find_by_tmdb_id(tmdb_id).await? {
            return Ok(existing);
        }

        debug!("Creating production from TMDB: {} (tmdb_id={})", title, tmdb_id);

        // Map TMDB media_type to production_type
        let production_type = match media_type.as_str() {
            "movie" => "Film",
            "tv" => "TV Series",
            _ => "Other",
        };

        // Generate slug from title
        let slug = generate_slug(&title);

        // Build embedding text for background update
        let embedding_text = build_production_embedding_text(
            &title,
            production_type,
            "Released",
            overview.as_deref(),
            None,
            release_date.as_deref(),
            None,
        );

        let query = r#"
            CREATE production CONTENT {
                title: $title,
                slug: $slug,
                type: $type,
                status: 'Released',
                description: $overview,
                tmdb_id: $tmdb_id,
                media_type: $media_type,
                poster_url: $poster_url,
                tmdb_url: $tmdb_url,
                release_date: $release_date,
                overview: $overview,
                source: 'tmdb',
                source_overrides: []
            } RETURN *;
        "#;

        let mut result = DB
            .query(query)
            .bind(("title", title))
            .bind(("slug", slug))
            .bind(("type", production_type.to_string()))
            .bind(("overview", overview))
            .bind(("tmdb_id", tmdb_id))
            .bind(("media_type", media_type))
            .bind(("poster_url", poster_url))
            .bind(("tmdb_url", tmdb_url))
            .bind(("release_date", release_date))
            .await
            .map_err(|e| Error::Database(format!("Failed to create TMDB production: {}", e)))?;

        let production: Option<Production> = result.take(0)?;
        let production = production.ok_or_else(|| {
            Error::Database("Failed to create TMDB production - no result returned".to_string())
        })?;

        // Fire-and-forget embedding update
        crate::services::embedding::spawn_embedding_update(production.id.clone(), embedding_text);

        Ok(production)
    }

    /// Search productions by title for dedup autocomplete
    pub async fn search_by_title(query: &str, limit: usize) -> Result<Vec<Production>, Error> {
        debug!("Searching productions by title: {}", query);

        let sql = r#"
            SELECT * FROM production
            WHERE string::lowercase(title) CONTAINS string::lowercase($query)
            ORDER BY release_date DESC, created_at DESC
            LIMIT $limit
        "#;

        let mut result = DB
            .query(sql)
            .bind(("query", query.to_string()))
            .bind(("limit", limit))
            .await
            .map_err(|e| Error::Database(format!("Failed to search productions: {}", e)))?;

        let productions: Vec<Production> = result.take(0)?;
        Ok(productions)
    }

    /// Check if a production is claimed (has an owner via member_of edge)
    pub async fn is_claimed(production_id: &RecordId) -> Result<bool, Error> {
        let query = r#"
            SELECT count() AS count FROM member_of
            WHERE out = $production_id AND role = 'owner'
        "#;

        let mut result = DB
            .query(query)
            .bind(("production_id", production_id.to_raw_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to check claim status: {}", e)))?;

        let count: Option<serde_json::Value> = result.take(0)?;
        if let Some(obj) = count {
            if let Some(c) = obj.get("count") {
                return Ok(c.as_u64().unwrap_or(0) > 0);
            }
        }
        Ok(false)
    }

    /// Claim an unclaimed production — creates owner member_of edge and promotes self-asserted credits
    pub async fn claim(production_id: &RecordId, claimer_id: &str) -> Result<(), Error> {
        debug!(
            "Claiming production {} by {}",
            production_id.display(),
            claimer_id
        );

        // Create owner edge
        let query = r#"
            RELATE $person->member_of->$production SET
                role = 'owner',
                joined_at = time::now(),
                invitation_status = 'accepted'
        "#;

        DB.query(query)
            .bind(("person", claimer_id.to_string()))
            .bind(("production", production_id.to_raw_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to claim production: {}", e)))?;

        // Promote self-asserted involvements to pending_verification
        let promote_query = r#"
            UPDATE involvement SET verification_status = 'pending_verification'
            WHERE out = $production_id AND verification_status = 'self_asserted'
        "#;

        DB.query(promote_query)
            .bind(("production_id", production_id.to_raw_string()))
            .await
            .map_err(|e| {
                Error::Database(format!("Failed to promote credits on claim: {}", e))
            })?;

        Ok(())
    }
}

/// Generate a URL-friendly slug from a title
fn generate_slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
