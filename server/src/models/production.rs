use crate::db::DB;
use crate::error::Error;
use crate::record_id_ext::RecordIdExt;
use crate::services::embedding::build_production_embedding_text;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::debug;

/// A production photo (gallery item)
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct ProductionPhoto {
    pub url: String,
    pub thumbnail_url: String,
    #[serde(default)]
    pub caption: String,
}

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
    // Photos
    #[serde(default)]
    #[surreal(default)]
    pub header_photo: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub poster_photo: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub photos: Vec<ProductionPhoto>,
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
    // Classification
    #[serde(default)]
    #[surreal(default)]
    pub budget_level: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub production_tier: Option<String>,
}

impl Production {
    /// Get the effective poster URL: custom poster_photo takes priority over TMDB poster_url
    pub fn effective_poster_url(&self) -> Option<&str> {
        self.poster_photo.as_deref().or(self.poster_url.as_deref())
    }
}

fn default_source() -> String {
    "manual".to_string()
}

/// Parse a date string from an HTML date input into a proper DateTime<Utc>.
/// HTML date inputs produce "2026-03-17"; we parse to a full DateTime for SurrealDB.
fn parse_datetime(s: Option<String>) -> Option<DateTime<Utc>> {
    s.and_then(|v| {
        let iso = if v.len() == 10 && !v.contains('T') {
            format!("{}T00:00:00Z", v)
        } else {
            v
        };
        iso.parse::<DateTime<Utc>>().ok()
    })
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
    pub budget_level: Option<String>,
    pub production_tier: Option<String>,
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
    pub budget_level: Option<String>,
    pub production_tier: Option<String>,
}

/// Member information for production members
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct ProductionMember {
    pub id: String,
    pub name: String,
    pub username: Option<String>,        // For persons
    pub slug: Option<String>,            // For organizations
    pub avatar: Option<String>,          // Profile avatar URL
    pub role: String,                    // owner, admin, member (permission level)
    #[serde(default)]
    #[surreal(default)]
    pub production_roles: Option<Vec<String>>, // e.g. ["Director", "Producer"]
    pub member_type: String,             // person or organization
    pub invitation_status: String,       // pending, accepted, declined
    #[serde(default)]
    #[surreal(default)]
    pub is_verified: bool,               // Whether org is verified (gold checkmark)
}

/// Production membership info (for "my productions" listing)
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct ProductionMembership {
    pub production_id: String,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub production_type: String,
    #[serde(default)]
    #[surreal(default)]
    pub poster_photo: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub poster_url: Option<String>,
    #[serde(default)]
    #[surreal(default)]
    pub location: Option<String>,
    pub role: String,
    #[serde(default)]
    #[surreal(default)]
    pub production_roles: Option<Vec<String>>,
    pub invitation_status: String,
    pub created_at: DateTime<Utc>,
}

/// Validate that a string looks like a safe RecordId ("table:key") and parse it.
/// This prevents SQL injection when the ID must be formatted into a query string
/// (e.g. for RELATE or WHERE in/out comparisons where bind params don't work with RecordIds).
fn validate_record_id_str(id: &str) -> Result<RecordId, Error> {
    let parts: Vec<&str> = id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(Error::BadRequest("Invalid record ID format".to_string()));
    }
    let table = parts[0];
    let key = parts[1];
    if table.is_empty()
        || key.is_empty()
        || !table
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::BadRequest("Invalid record ID format".to_string()));
    }
    Ok(RecordId::new(table, key))
}

/// Production model for database operations
pub struct ProductionModel;

impl ProductionModel {
    /// Create a new production and establish ownership
    pub async fn create(
        data: CreateProductionData,
        creator_id: &str,
        creator_type: &str, // "person" or "organization"
        owner_production_roles: Option<Vec<String>>,
    ) -> Result<Production, Error> {
        // Validate creator_id to prevent SQL injection in RELATE query
        let creator_rid = validate_record_id_str(creator_id)?;

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
                location: $location,
                budget_level: $budget_level,
                production_tier: $production_tier
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
            .bind(("start_date", parse_datetime(data.start_date)))
            .bind(("end_date", parse_datetime(data.end_date)))
            .bind(("description", data.description))
            .bind(("location", data.location))
            .bind(("budget_level", data.budget_level))
            .bind(("production_tier", data.production_tier))
            .await
            .map_err(|e| Error::Database(format!("Failed to create production: {}", e)))?;

        let production: Option<Production> = result.take(0)?;
        let production = production.ok_or_else(|| {
            Error::Database("Failed to create production - no result returned".to_string())
        })?;

        // Fire-and-forget embedding update
        crate::services::embedding::spawn_embedding_update(production.id.clone(), embedding_text);

        // Create ownership relation — format IDs directly into query
        // because RELATE needs RecordIds, not strings
        let roles = owner_production_roles
            .as_ref()
            .filter(|r| !r.is_empty());

        let ownership_query = format!(
            "RELATE {}->member_of->{} SET role = 'owner', invitation_status = 'accepted', production_roles = $production_roles;",
            creator_rid.display(),
            production.id.display(),
        );

        DB.query(&ownership_query)
            .bind(("production_roles", roles.cloned()))
            .await
            .map_err(|e| Error::Database(format!("Failed to create ownership relation: {}", e)))?;

        // Also create involvement (credit) edges for each owner production role
        if let Some(ref roles) = owner_production_roles {
            use crate::models::involvement::InvolvementModel;
            for role in roles.iter().filter(|r| !r.is_empty()) {
                let _ = InvolvementModel::create(
                    creator_id,
                    &production.id,
                    "crew",
                    Some(role),
                    None,
                    None,
                    "manual",
                )
                .await;
            }
        }

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
            .query(&format!("SELECT * FROM {}", production_id.display()))
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
        query_embedding: Option<Vec<f32>>,
        sort: Option<&str>,
        offset: usize,
    ) -> Result<Vec<Production>, Error> {
        debug!(
            "Listing productions - status: {:?}, type: {:?}, filter: {:?}, sort: {:?}",
            status_filter, type_filter, filter, sort
        );

        let has_embedding = query_embedding.is_some();
        let empty_emb: Vec<f32> = vec![];

        let mut query = String::from("SELECT *");

        if filter.is_some() || has_embedding {
            query.push_str(
                ", <float> (
                    (IF string::lowercase(title ?? '') CONTAINS string::lowercase($filter ?? '') THEN 50 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS string::lowercase($filter ?? '') THEN 20 ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS string::lowercase($filter ?? '') THEN 20 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS _score"
            );
        }

        query.push_str(" FROM production WHERE 1=1");

        if status_filter.is_some() {
            query.push_str(" AND status = $status");
        }

        if type_filter.is_some() {
            query.push_str(" AND type = $type");
        }

        if filter.is_some() || has_embedding {
            let mut text_or_vector = Vec::new();
            if filter.is_some() {
                text_or_vector.push("string::lowercase(title) CONTAINS string::lowercase($filter)".to_string());
                text_or_vector.push("string::lowercase(description ?? '') CONTAINS string::lowercase($filter)".to_string());
                text_or_vector.push("string::lowercase(location ?? '') CONTAINS string::lowercase($filter)".to_string());
            }
            if has_embedding {
                text_or_vector.push(format!("(embedding IS NOT NONE AND $has_embedding = true AND vector::similarity::cosine(embedding, $query_embedding) > {})", crate::config::search_weights().vector_threshold));
            }
            query.push_str(&format!(" AND ({})", text_or_vector.join(" OR ")));
        }

        if filter.is_some() || has_embedding {
            query.push_str(" ORDER BY _score DESC, created_at DESC");
        } else {
            let order_clause = match sort {
                Some("title") => " ORDER BY title ASC",
                Some("status") => " ORDER BY status ASC, created_at DESC",
                _ => " ORDER BY created_at DESC",
            };
            query.push_str(order_clause);
        }

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if offset > 0 {
            query.push_str(&format!(" START {}", offset));
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

        db_query = db_query.bind(("has_embedding", has_embedding));
        db_query = db_query.bind(("query_embedding", query_embedding.unwrap_or(empty_emb)));

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
        if data.budget_level.is_some() {
            update_fields.push("budget_level = $budget_level");
        }
        if data.production_tier.is_some() {
            update_fields.push("production_tier = $production_tier");
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
            "UPDATE {} SET {} RETURN *",
            production_id.display(),
            update_fields.join(", ")
        );

        let mut db_query = DB
            .query(&query);

        if let Some(title) = data.title {
            db_query = db_query.bind(("title", title));
        }
        if let Some(prod_type) = data.production_type {
            db_query = db_query.bind(("type", prod_type));
        }
        if let Some(status) = data.status {
            db_query = db_query.bind(("status", status));
        }
        if let Some(start_date) = parse_datetime(data.start_date) {
            db_query = db_query.bind(("start_date", start_date));
        }
        if let Some(end_date) = parse_datetime(data.end_date) {
            db_query = db_query.bind(("end_date", end_date));
        }
        if let Some(description) = data.description {
            db_query = db_query.bind(("description", description));
        }
        if let Some(location) = data.location {
            db_query = db_query.bind(("location", location));
        }
        if let Some(budget_level) = data.budget_level {
            db_query = db_query.bind(("budget_level", budget_level));
        }
        if let Some(production_tier) = data.production_tier {
            db_query = db_query.bind(("production_tier", production_tier));
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
        DB.query(&format!("DELETE member_of WHERE out = {}", production_id.display()))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete member relations: {}", e)))?;

        // Delete all involvement relations to this production
        DB.query(&format!("DELETE involvement WHERE out = {}", production_id.display()))
            .await
            .map_err(|e| {
                Error::Database(format!("Failed to delete involvement relations: {}", e))
            })?;

        // Delete the production
        DB.query(&format!("DELETE {}", production_id.display()))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete production: {}", e)))?;

        // Commit transaction
        DB.query("COMMIT TRANSACTION")
            .await
            .map_err(|e| Error::Database(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// Get productions for a user or organization, with their role info
    pub async fn get_member_productions(member_id: &str) -> Result<Vec<ProductionMembership>, Error> {
        let member_rid = validate_record_id_str(member_id)?;
        debug!("Fetching productions for member: {}", member_id);

        let query = format!(
            "SELECT
                <string> out.id AS production_id,
                out.title AS title,
                out.slug AS slug,
                out.status AS status,
                out.`type` AS production_type,
                out.poster_photo AS poster_photo,
                out.poster_url AS poster_url,
                out.location AS location,
                role,
                production_roles,
                invitation_status,
                out.created_at AS created_at
            FROM member_of
            WHERE in = {}
            AND <string> type::table(out) = 'production'
            ORDER BY created_at DESC",
            member_rid.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch member productions: {}", e)))?;

        let productions: Vec<ProductionMembership> = result.take(0)?;
        Ok(productions)
    }

    /// Get members of a production
    pub async fn get_members(production_id: &RecordId) -> Result<Vec<ProductionMember>, Error> {
        debug!("Fetching members for production: {}", production_id.display());

        let query = format!(
            "SELECT
                <string> in.id as id,
                in.name as name,
                in.username as username,
                in.slug as slug,
                IF <string> type::table(in) = 'person' THEN in.profile.avatar ELSE in.logo END as avatar,
                role,
                production_roles,
                <string> type::table(in) as member_type,
                invitation_status,
                in.verified ?? false as is_verified
            FROM member_of
            WHERE out = {}
            ORDER BY role ASC, in.name ASC",
            production_id.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production members: {}", e)))?;

        let members: Vec<ProductionMember> = result.take(0)?;
        Ok(members)
    }

    /// Check if a user or organization is a member of a production
    pub async fn is_member(production_id: &RecordId, member_id: &str) -> Result<bool, Error> {
        let member_rid = validate_record_id_str(member_id)?;
        debug!(
            "Checking membership for {} in production {}",
            member_id, production_id.display()
        );

        let query = format!(
            "SELECT count() as count FROM member_of WHERE in = {} AND out = {}",
            member_rid.display(),
            production_id.display()
        );

        let mut result = DB
            .query(&query)
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

    /// Check if a user can edit a production (owner or admin).
    /// Also grants access if the user is owner/admin of an organization that is
    /// itself owner/admin of the production.
    pub async fn can_edit(production_id: &RecordId, member_id: &str) -> Result<bool, Error> {
        let member_rid = validate_record_id_str(member_id)?;
        debug!(
            "Checking edit permission for {} in production {}",
            member_id, production_id.display()
        );

        // Direct membership check
        let query = format!(
            "SELECT role FROM member_of WHERE in = {} AND out = {}",
            member_rid.display(),
            production_id.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(format!("Failed to check edit permission: {}", e)))?;

        let member: Option<serde_json::Value> = result.take(0)?;
        if let Some(member_obj) = member {
            if let Some(role) = member_obj.get("role").and_then(|r| r.as_str()) {
                if role == "owner" || role == "admin" {
                    return Ok(true);
                }
            }
        }

        // Indirect check: person is owner/admin of an org that is owner/admin of this production
        let org_query = format!(
            "SELECT VALUE out FROM member_of \
             WHERE in = {} \
             AND <string> type::table(out) = 'organization' \
             AND role IN ['owner', 'admin'] \
             AND invitation_status = 'accepted'",
            member_rid.display()
        );

        let mut org_result = DB
            .query(&org_query)
            .await
            .map_err(|e| Error::Database(format!("Failed to check org memberships: {}", e)))?;

        let org_ids: Vec<surrealdb::types::RecordId> = org_result.take(0).unwrap_or_default();

        for org_id in org_ids {
            let prod_query = format!(
                "SELECT role FROM member_of WHERE in = {} AND out = {}",
                org_id.display(),
                production_id.display()
            );

            let mut prod_result = DB
                .query(&prod_query)
                .await
                .map_err(|e| Error::Database(format!("Failed to check org production role: {}", e)))?;

            let org_member: Option<serde_json::Value> = prod_result.take(0)?;
            if let Some(obj) = org_member {
                if let Some(role) = obj.get("role").and_then(|r| r.as_str()) {
                    if role == "owner" || role == "admin" {
                        debug!(
                            "User {} has edit access via org {} (role: {})",
                            member_id, org_id.display(), role
                        );
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Add a member to a production with invitation (pending status)
    pub async fn add_member(
        production_id: &RecordId,
        member_id: &str,
        role: &str,
        production_roles: Option<Vec<String>>,
        invited_by: Option<&str>,
    ) -> Result<(), Error> {
        let member_rid = validate_record_id_str(member_id)?;
        let invited_by_rid = invited_by.map(validate_record_id_str).transpose()?;
        debug!(
            "Adding member {} to production {} with role {} / production_roles {:?}",
            member_id, production_id.display(), role, production_roles
        );

        let invited_by_clause = if let Some(ref inviter_rid) = invited_by_rid {
            format!(", invited_by = {}", inviter_rid.display())
        } else {
            String::new()
        };

        let query = format!(
            "RELATE {}->member_of->{} SET role = $role, invitation_status = 'pending', production_roles = $production_roles{}",
            member_rid.display(),
            production_id.display(),
            invited_by_clause,
        );

        let roles = production_roles.filter(|r| !r.is_empty());

        DB.query(&query)
            .bind(("role", role.to_string()))
            .bind(("production_roles", roles))
            .await
            .map_err(|e| Error::Database(format!("Failed to add member: {}", e)))?;

        Ok(())
    }

    /// Add a member to a production with accepted status (e.g. owner/creator)
    pub async fn add_member_accepted(
        production_id: &RecordId,
        member_id: &str,
        role: &str,
        production_roles: Option<Vec<String>>,
    ) -> Result<(), Error> {
        let member_rid = validate_record_id_str(member_id)?;
        debug!(
            "Adding accepted member {} to production {} with role {}",
            member_id, production_id.display(), role
        );

        let roles = production_roles.filter(|r| !r.is_empty());

        let query = format!(
            "RELATE {}->member_of->{} SET role = $role, invitation_status = 'accepted', production_roles = $production_roles",
            member_rid.display(),
            production_id.display(),
        );

        DB.query(&query)
            .bind(("role", role.to_string()))
            .bind(("production_roles", roles.clone()))
            .await
            .map_err(|e| Error::Database(format!("Failed to add member: {}", e)))?;

        // Also create involvement (credit) edges so the production appears on the person's profile
        if let Some(ref roles) = roles {
            use crate::models::involvement::InvolvementModel;
            for prod_role in roles.iter().filter(|r| !r.is_empty()) {
                if !InvolvementModel::exists(member_id, production_id, Some(prod_role)).await? {
                    InvolvementModel::create(
                        member_id,
                        production_id,
                        "crew",
                        Some(prod_role),
                        None,
                        None,
                        "invited",
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    /// Remove a member from a production
    pub async fn remove_member(production_id: &RecordId, member_id: &str) -> Result<(), Error> {
        let member_rid = validate_record_id_str(member_id)?;
        debug!(
            "Removing member {} from production {}",
            member_id, production_id.display()
        );

        let query = format!(
            "DELETE FROM member_of WHERE in = {} AND out = {}",
            member_rid.display(),
            production_id.display()
        );

        DB.query(&query)
            .await
            .map_err(|e| Error::Database(format!("Failed to remove member: {}", e)))?;

        Ok(())
    }

    /// Update production roles for an existing member
    pub async fn update_member_roles(
        production_id: &RecordId,
        member_id: &str,
        new_roles: Vec<String>,
    ) -> Result<(), Error> {
        let member_rid = validate_record_id_str(member_id)?;
        debug!(
            "Updating roles for member {} in production {} to {:?}",
            member_id, production_id.display(), new_roles
        );

        // Get current roles from member_of
        let get_query = format!(
            "SELECT VALUE production_roles FROM member_of WHERE in = {} AND out = {} LIMIT 1",
            member_rid.display(),
            production_id.display()
        );
        let mut result = DB
            .query(&get_query)
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch member roles: {}", e)))?;
        let current: Option<Vec<String>> = result.take(0)?;
        let old_roles: Vec<String> = current.unwrap_or_default();

        let roles_to_store = if new_roles.is_empty() {
            None
        } else {
            Some(new_roles.clone())
        };

        // Update the member_of edge
        let update_query = format!(
            "UPDATE member_of SET production_roles = $production_roles WHERE in = {} AND out = {}",
            member_rid.display(),
            production_id.display()
        );
        DB.query(&update_query)
            .bind(("production_roles", roles_to_store))
            .await
            .map_err(|e| Error::Database(format!("Failed to update member roles: {}", e)))?;

        // Sync involvement edges: delete removed roles, add new ones
        use crate::models::involvement::InvolvementModel;

        // Delete involvements for roles that were removed
        for old_role in &old_roles {
            if !new_roles.contains(old_role) {
                InvolvementModel::delete_by_person_production_role(member_id, production_id, old_role).await?;
            }
        }

        // Create involvements for newly added roles
        // Use "manual" source since this is set by a production editor (owner/admin)
        for new_role in &new_roles {
            if !new_role.is_empty() && !old_roles.contains(new_role) {
                if !InvolvementModel::exists(member_id, production_id, Some(new_role)).await? {
                    InvolvementModel::create(
                        member_id,
                        production_id,
                        "crew",
                        Some(new_role),
                        None,
                        None,
                        "manual",
                    )
                    .await?;
                }
            }
        }

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
            .query("SELECT name, position FROM production_status ORDER BY position")
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production statuses: {}", e)))?;

        let statuses: Vec<serde_json::Value> = result.take(0)?;
        Ok(statuses
            .into_iter()
            .filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
    }

    /// Get budget levels from the database
    pub async fn get_budget_levels() -> Result<Vec<String>, Error> {
        debug!("Fetching budget levels");

        let mut result = DB
            .query("SELECT name, position FROM budget_level ORDER BY position")
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch budget levels: {}", e)))?;

        let levels: Vec<serde_json::Value> = result.take(0)?;
        Ok(levels
            .into_iter()
            .filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
    }

    /// Get production tiers from the database
    pub async fn get_production_tiers() -> Result<Vec<String>, Error> {
        debug!("Fetching production tiers");

        let mut result = DB
            .query("SELECT name, position FROM production_tier ORDER BY position")
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch production tiers: {}", e)))?;

        let tiers: Vec<serde_json::Value> = result.take(0)?;
        Ok(tiers
            .into_iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
    }

    /// Get production roles from the role table (for dropdown)
    pub async fn get_roles() -> Result<Vec<String>, Error> {
        debug!("Fetching production roles");

        let mut result = DB
            .query("SELECT name FROM role ORDER BY name")
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch roles: {}", e)))?;

        let roles: Vec<serde_json::Value> = result.take(0)?;
        Ok(roles
            .into_iter()
            .filter_map(|r| r.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
    }

    /// Get roles filtered by type: "individual", "organization", or "both"
    pub async fn get_roles_by_type(role_type: &str) -> Result<Vec<String>, Error> {
        debug!("Fetching roles for type: {}", role_type);

        let mut result = DB
            .query("SELECT name FROM role WHERE role_type = $role_type OR role_type = 'both' ORDER BY name")
            .bind(("role_type", role_type.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch roles by type: {}", e)))?;

        let roles: Vec<serde_json::Value> = result.take(0)?;
        Ok(roles
            .into_iter()
            .filter_map(|r| r.get("name").and_then(|n| n.as_str()).map(String::from))
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

    /// Create a production from scraped data (no TMDB match available)
    pub async fn create_from_scraped_data(
        title: &str,
        production_type: &str,
        year: Option<&str>,
        source: &str,
    ) -> Result<Production, Error> {
        debug!(
            "Creating production from scraped data: {} (year={:?}, source={})",
            title, year, source
        );

        let slug = generate_slug(title);
        let release_date = year.map(|y| format!("{}-01-01", y));

        let embedding_text = build_production_embedding_text(
            title,
            production_type,
            "Released",
            None,
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
                release_date: $release_date,
                source: $source,
                source_overrides: []
            } RETURN *;
        "#;

        let mut result = DB
            .query(query)
            .bind(("title", title.to_string()))
            .bind(("slug", slug))
            .bind(("type", production_type.to_string()))
            .bind(("release_date", release_date))
            .bind(("source", source.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to create scraped production: {}", e)))?;

        let production: Option<Production> = result.take(0)?;
        let production = production.ok_or_else(|| {
            Error::Database("Failed to create scraped production - no result returned".to_string())
        })?;

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
        let query = format!(
            "SELECT count() AS count FROM member_of WHERE out = {} AND role = 'owner'",
            production_id.display()
        );

        let mut result = DB
            .query(&query)
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
        let claimer_rid = validate_record_id_str(claimer_id)?;
        debug!(
            "Claiming production {} by {}",
            production_id.display(),
            claimer_id
        );

        // Create owner edge
        let query = format!(
            "RELATE {}->member_of->{} SET role = 'owner', joined_at = time::now(), invitation_status = 'accepted'",
            claimer_rid.display(),
            production_id.display()
        );

        DB.query(&query)
            .await
            .map_err(|e| Error::Database(format!("Failed to claim production: {}", e)))?;

        // Promote self-asserted involvements to pending_verification
        let promote_query = format!(
            "UPDATE involvement SET verification_status = 'pending_verification' WHERE out = {} AND verification_status = 'self_asserted'",
            production_id.display()
        );

        DB.query(&promote_query)
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
