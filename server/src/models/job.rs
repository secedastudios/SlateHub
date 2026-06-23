//! Job postings and applications (the jobs board).
//!
//! Owns the `job_posting` table (roles are embedded as an object array) and
//! the `application` RELATION from person to job_posting; also reads the
//! `pay_rate_type` reference table. Called by `routes/jobs.rs`.
//! Queries cast record ids and datetimes to `<string>` because the v3 SDK
//! can't deserialize RecordId values into `serde_json::Value` rows, and
//! RELATE/WHERE-on-edge clauses format validated record ids directly into the
//! query text (bind params don't match RecordId `in`/`out` fields).

use crate::db::DB;
use crate::error::Error;
use crate::record_id_ext::RecordIdExt;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;
use tracing::debug;

/// Validate that a record key contains only safe characters (alphanumeric and underscore).
/// This prevents SQL injection when a key must be formatted into a query string.
fn validate_record_key(key: &str) -> Result<(), Error> {
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(Error::BadRequest("Invalid record ID".to_string()));
    }
    Ok(())
}

/// Validate a full record ID string like "table:key" and return a RecordId.
fn parse_record_id(id: &str) -> Result<RecordId, Error> {
    let parts: Vec<&str> = id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(Error::BadRequest("Invalid record ID format".to_string()));
    }
    validate_record_key(parts[0])?;
    validate_record_key(parts[1])?;
    Ok(RecordId::new(parts[0], parts[1]))
}

/// A job posting from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPosting {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub posted_by: String,
    pub related_production: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_website: Option<String>,
    pub applications_enabled: bool,
    /// One of "open" | "closed" | "filled" (schema ASSERT on
    /// `job_posting.status`).
    pub status: String,
    pub expires_at: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Denormalized view for job listings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub poster_name: String,
    pub poster_slug: String,
    /// "person" | "organization" — derived from the `posted_by` id's table.
    pub poster_type: String,
    pub is_poster_verified: bool,
    pub role_count: i64,
    /// One of "open" | "closed" | "filled" (schema ASSERT on
    /// `job_posting.status`).
    pub status: String,
    pub expires_at: String,
    pub created_at: String,
    pub production_title: Option<String>,
    pub production_poster: Option<String>,
    pub applications_enabled: bool,
}

/// Full detail view for a single job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetailView {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub poster_name: String,
    pub poster_slug: String,
    /// "person" | "organization" — derived from the `posted_by` id's table.
    pub poster_type: String,
    pub is_poster_verified: bool,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_website: Option<String>,
    pub applications_enabled: bool,
    /// One of "open" | "closed" | "filled" (schema ASSERT on
    /// `job_posting.status`).
    pub status: String,
    pub expires_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub roles: Vec<JobRoleView>,
    pub production_title: Option<String>,
    pub production_slug: Option<String>,
    pub production_poster: Option<String>,
    pub can_edit: bool,
    pub is_expired: bool,
    pub application_count: i64,
    pub applications: Vec<ApplicationView>,
}

/// Role view (display only — embedded in job_posting)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRoleView {
    pub title: String,
    pub description: Option<String>,
    /// Pay-rate label from the `pay_rate_type` reference table; defaults to
    /// "TBD" (no schema ASSERT).
    pub rate_type: String,
    pub rate_amount: Option<String>,
    pub location_override: Option<String>,
    #[serde(default)]
    pub has_applied: bool,
}

/// Application view (for poster to see applicants)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationView {
    pub id: String,
    pub applicant_name: String,
    pub applicant_username: String,
    pub applicant_avatar: Option<String>,
    pub role_title: String,
    pub cover_letter: Option<String>,
    /// One of "submitted" | "reviewed" | "shortlisted" | "rejected" |
    /// "withdrawn" (schema ASSERT on `application.status`).
    pub status: String,
    pub applied_at: String,
}

/// User's own application view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserApplicationView {
    pub id: String,
    pub job_id: String,
    pub job_title: String,
    pub role_title: String,
    pub poster_name: String,
    pub cover_letter: Option<String>,
    /// One of "submitted" | "reviewed" | "shortlisted" | "rejected" |
    /// "withdrawn" (schema ASSERT on `application.status`).
    pub status: String,
    pub applied_at: String,
}

/// Data to create a job posting
#[derive(Debug, Clone)]
pub struct CreateJobData {
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_website: Option<String>,
    pub applications_enabled: bool,
    pub related_production: Option<String>,
    /// Form value: "1day" | "1week" | "1month" (anything else is treated as
    /// one week by `compute_expires_at`).
    pub expires_in: String,
}

/// Data for a role (embedded in job posting)
#[derive(Debug, Clone, Serialize)]
pub struct CreateJobRoleData {
    pub title: String,
    pub description: Option<String>,
    /// Pay-rate label from the `pay_rate_type` reference table; defaults to
    /// "TBD" (no schema ASSERT).
    pub rate_type: String,
    pub rate_amount: Option<String>,
    pub location_override: Option<String>,
}

/// Data to update a job posting
#[derive(Debug, Clone)]
pub struct UpdateJobData {
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_website: Option<String>,
    pub applications_enabled: bool,
    /// Form value: "1day" | "1week" | "1month" (anything else is treated as
    /// one week by `compute_expires_at`).
    pub expires_in: String,
}

/// Query/mutation surface for job postings and their applications.
pub struct JobModel;

impl JobModel {
    /// Map an `expires_in` form value ("1day" | "1week" | "1month") to an
    /// absolute expiry; anything else falls back to one week.
    fn compute_expires_at(expires_in: &str) -> DateTime<Utc> {
        let now = Utc::now();
        match expires_in {
            "1day" => now + Duration::days(1),
            "1week" => now + Duration::weeks(1),
            "1month" => now + Duration::days(30),
            _ => now + Duration::weeks(1),
        }
    }

    /// Build a JSON array string for roles to embed in the query
    fn roles_json(roles: &[CreateJobRoleData]) -> String {
        let items: Vec<String> = roles.iter().map(|r| {
            let desc = match &r.description {
                Some(d) => format!("\"{}\"", d.replace('"', "\\\"")),
                None => "NONE".to_string(),
            };
            let amount = match &r.rate_amount {
                Some(a) => format!("\"{}\"", a.replace('"', "\\\"")),
                None => "NONE".to_string(),
            };
            let loc = match &r.location_override {
                Some(l) => format!("\"{}\"", l.replace('"', "\\\"")),
                None => "NONE".to_string(),
            };
            format!(
                r#"{{ title: "{}", description: {}, rate_type: "{}", rate_amount: {}, location_override: {} }}"#,
                r.title.replace('"', "\\\""),
                desc,
                r.rate_type.replace('"', "\\\""),
                amount,
                loc,
            )
        }).collect();
        format!("[{}]", items.join(", "))
    }

    /// Create a new job posting with embedded roles
    pub async fn create(
        data: CreateJobData,
        roles: Vec<CreateJobRoleData>,
        poster_id: &str,
    ) -> Result<String, Error> {
        debug!("Creating job posting: {} by {}", data.title, poster_id);

        let poster_record = parse_record_id(poster_id)?;
        let expires_at = Self::compute_expires_at(&data.expires_in);

        let prod_clause = if let Some(ref prod_id) = data.related_production {
            if !prod_id.is_empty() {
                let prod_record = parse_record_id(prod_id)?;
                format!(", related_production = {}", prod_record.display())
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let roles_json = Self::roles_json(&roles);

        let query = format!(
            r#"CREATE job_posting SET
                title = $title,
                description = $description,
                location = $location,
                posted_by = {},
                contact_name = $contact_name,
                contact_email = $contact_email,
                contact_phone = $contact_phone,
                contact_website = $contact_website,
                applications_enabled = <bool> $applications_enabled,
                roles = {},
                expires_at = <datetime> $expires_at{}
            RETURN <string> id AS id;"#,
            poster_record.display(),
            roles_json,
            prod_clause
        );

        debug!("Job create query: {}", query);

        let mut result = DB
            .query(&query)
            .bind(("title", data.title))
            .bind(("description", data.description))
            .bind(("location", data.location))
            .bind(("contact_name", data.contact_name))
            .bind(("contact_email", data.contact_email))
            .bind(("contact_phone", data.contact_phone))
            .bind(("contact_website", data.contact_website))
            .bind((
                "applications_enabled",
                if data.applications_enabled {
                    "true"
                } else {
                    "false"
                },
            ))
            .bind(("expires_at", expires_at.to_rfc3339()))
            .await
            .map_err(|e| Error::Database(format!("Failed to create job posting: {}", e)))?;

        let row: Option<serde_json::Value> = result.take(0)?;
        debug!("Job create result: {:?}", row);
        let job_id = row
            .and_then(|r| r.get("id").and_then(|v| v.as_str()).map(String::from))
            .ok_or_else(|| Error::Database("No job ID returned".to_string()))?;

        let key = job_id.strip_prefix("job_posting:").unwrap_or(&job_id);
        debug!("Created job posting: {}", key);
        Ok(key.to_string())
    }

    /// List jobs with optional search
    pub async fn list(
        search: Option<&str>,
        query_embedding: Option<Vec<f32>>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobListItem>, Error> {
        debug!("Listing jobs, search: {:?}", search);

        let has_embedding = query_embedding.is_some();
        let empty_emb: Vec<f32> = vec![];

        let mut query = String::from(
            r#"SELECT
                <string> id AS id,
                title,
                description,
                location,
                <string> posted_by AS posted_by_id,
                array::len(roles) AS role_count,
                status,
                <string> expires_at AS expires_at,
                <string> created_at AS created_at,
                <string> related_production AS related_production_id,
                applications_enabled"#,
        );

        if search.is_some() || has_embedding {
            query.push_str(
                ", <float> (
                    (IF string::lowercase(title ?? '') CONTAINS string::lowercase($search ?? '') THEN 50 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS string::lowercase($search ?? '') THEN 20 ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS string::lowercase($search ?? '') THEN 20 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS _score"
            );
        }

        query.push_str(
            r#"
            FROM job_posting
            WHERE status = 'open'
            AND expires_at > time::now()"#,
        );

        if search.is_some() || has_embedding {
            let mut text_or_vector = Vec::new();
            if search.is_some() {
                text_or_vector.push(
                    "string::lowercase(title) CONTAINS string::lowercase($search)".to_string(),
                );
                text_or_vector.push(
                    "string::lowercase(description) CONTAINS string::lowercase($search)"
                        .to_string(),
                );
                text_or_vector.push("string::lowercase(string::join(' ', roles.*.title)) CONTAINS string::lowercase($search)".to_string());
            }
            if has_embedding {
                text_or_vector.push(format!("(embedding IS NOT NONE AND $has_embedding = true AND vector::similarity::cosine(embedding, $query_embedding) > {})", crate::config::search_weights().vector_threshold));
            }
            query.push_str(&format!(" AND ({})", text_or_vector.join(" OR ")));
        }

        if search.is_some() || has_embedding {
            query.push_str(" ORDER BY _score DESC, created_at DESC");
        } else {
            query.push_str(" ORDER BY created_at DESC");
        }
        query.push_str(&format!(" LIMIT {}", limit));
        if offset > 0 {
            query.push_str(&format!(" START {}", offset));
        }

        let mut db_query = DB.query(&query);
        if let Some(s) = search {
            db_query = db_query.bind(("search", s.to_string()));
        }
        db_query = db_query.bind(("has_embedding", has_embedding));
        db_query = db_query.bind(("query_embedding", query_embedding.unwrap_or(empty_emb)));

        let mut result = db_query
            .await
            .map_err(|e| Error::Database(format!("Failed to list jobs: {}", e)))?;

        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        let mut jobs = Vec::new();
        for row in &rows {
            let posted_by_id = row
                .get("posted_by_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let (poster_name, poster_slug, poster_type, is_poster_verified) =
                Self::fetch_poster_info(posted_by_id).await;

            let related_prod_id = row
                .get("related_production_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let (production_title, _slug, production_poster) =
                Self::fetch_production_info(related_prod_id).await;

            jobs.push(JobListItem {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                title: row
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: row
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                location: row
                    .get("location")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                poster_name,
                poster_slug,
                poster_type,
                is_poster_verified,
                role_count: row.get("role_count").and_then(|v| v.as_i64()).unwrap_or(0),
                status: row
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("open")
                    .to_string(),
                expires_at: row
                    .get("expires_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                created_at: row
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                production_title,
                production_poster,
                applications_enabled: row
                    .get("applications_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            });
        }

        // Sort verified posters first, then by created_at (already DESC from query)
        jobs.sort_by_key(|j| std::cmp::Reverse(j.is_poster_verified));

        Ok(jobs)
    }

    /// Get the full detail view of one posting by its bare record key.
    ///
    /// Joins poster and production info, marks which roles the viewer has
    /// already applied to, counts live applications (`GROUP ALL` so the
    /// aggregate returns a single row), and loads the applicant list when the
    /// viewer can edit the posting.
    ///
    /// # Errors
    /// `Error::BadRequest` for an unsafe key, `Error::NotFound` if the
    /// posting doesn't exist, `Error::Database` on query failure.
    pub async fn get(
        job_id: &str,
        current_person_id: Option<&str>,
    ) -> Result<JobDetailView, Error> {
        debug!("Fetching job: {}", job_id);

        validate_record_key(job_id)?;
        let job_rid = RecordId::new("job_posting", job_id);

        let basic_query = format!(
            r#"SELECT
                <string> id AS id,
                title,
                description,
                location,
                <string> posted_by AS posted_by_id,
                <string> related_production AS related_production_id,
                contact_name,
                contact_email,
                contact_phone,
                contact_website,
                applications_enabled,
                status,
                roles,
                <string> expires_at AS expires_at,
                <string> created_at AS created_at,
                <string> updated_at AS updated_at
            FROM ONLY {}"#,
            job_rid.display()
        );

        let mut result = DB
            .query(&basic_query)
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch job: {}", e)))?;

        let job: Option<serde_json::Value> = result.take(0)?;
        let job = job.ok_or(Error::NotFound)?;

        let posted_by_id = job
            .get("posted_by_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (poster_name, poster_slug, poster_type, is_poster_verified) =
            Self::fetch_poster_info(posted_by_id).await;

        let related_prod_id = job
            .get("related_production_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (production_title, production_slug, production_poster) =
            Self::fetch_production_info(related_prod_id).await;

        // Parse embedded roles array
        let mut roles: Vec<JobRoleView> = job
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|r| JobRoleView {
                        title: r
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        description: r
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        rate_type: r
                            .get("rate_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("TBD")
                            .to_string(),
                        rate_amount: r
                            .get("rate_amount")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        location_override: r
                            .get("location_override")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        has_applied: false,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Check per-role application state for current user
        if let Some(uid) = current_person_id {
            let uid_record = parse_record_id(uid)?;
            let app_query = format!(
                "SELECT role_title FROM application WHERE in = {} AND out = {} AND status != 'withdrawn'",
                uid_record.display(),
                job_rid.display()
            );
            if let Ok(mut ar) = DB.query(&app_query).await {
                let app_rows: Vec<serde_json::Value> = ar.take(0).unwrap_or_default();
                let applied_titles: Vec<String> = app_rows
                    .iter()
                    .filter_map(|r| {
                        r.get("role_title")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    })
                    .collect();
                for role in &mut roles {
                    role.has_applied = applied_titles.contains(&role.title);
                }
            }
        }

        // Total application count
        let application_count = {
            let count_query = format!(
                "SELECT count() AS count FROM application WHERE out = {} AND status != 'withdrawn' GROUP ALL",
                job_rid.display()
            );
            if let Ok(mut cr) = DB.query(&count_query).await {
                let v: Option<serde_json::Value> = cr.take(0).unwrap_or(None);
                v.and_then(|o| o.get("count").and_then(|c| c.as_i64()))
                    .unwrap_or(0)
            } else {
                0
            }
        };

        let can_edit = if let Some(uid) = current_person_id {
            Self::can_edit(job_id, uid).await.unwrap_or(false)
        } else {
            false
        };

        // Fetch applications if user can edit this job
        let applications = if can_edit {
            Self::get_applications(job_id).await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let expires_str = job.get("expires_at").and_then(|v| v.as_str()).unwrap_or("");
        let is_expired = DateTime::parse_from_rfc3339(expires_str)
            .map(|dt| dt < Utc::now())
            .unwrap_or(false);

        Ok(JobDetailView {
            id: job
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            title: job
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            description: job
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            location: job
                .get("location")
                .and_then(|v| v.as_str())
                .map(String::from),
            poster_name,
            poster_slug,
            poster_type,
            is_poster_verified,
            contact_name: job
                .get("contact_name")
                .and_then(|v| v.as_str())
                .map(String::from),
            contact_email: job
                .get("contact_email")
                .and_then(|v| v.as_str())
                .map(String::from),
            contact_phone: job
                .get("contact_phone")
                .and_then(|v| v.as_str())
                .map(String::from),
            contact_website: job
                .get("contact_website")
                .and_then(|v| v.as_str())
                .map(String::from),
            applications_enabled: job
                .get("applications_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            status: job
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("open")
                .to_string(),
            expires_at: expires_str.to_string(),
            created_at: job
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            updated_at: job
                .get("updated_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            roles,
            production_title,
            production_slug,
            production_poster,
            can_edit,
            is_expired,
            application_count,
            applications,
        })
    }

    /// Update a job posting (full overwrite of fields and embedded roles)
    /// identified by its bare record key.
    pub async fn update(
        job_id: &str,
        data: UpdateJobData,
        roles: Vec<CreateJobRoleData>,
    ) -> Result<(), Error> {
        debug!("Updating job posting: {}", job_id);

        validate_record_key(job_id)?;
        let job_rid = RecordId::new("job_posting", job_id);
        let expires_at = Self::compute_expires_at(&data.expires_in);
        let roles_json = Self::roles_json(&roles);

        let query = format!(
            r#"UPDATE {} SET
                title = $title,
                description = $description,
                location = $location,
                contact_name = $contact_name,
                contact_email = $contact_email,
                contact_phone = $contact_phone,
                contact_website = $contact_website,
                applications_enabled = <bool> $applications_enabled,
                roles = {},
                expires_at = <datetime> $expires_at;"#,
            job_rid.display(),
            roles_json
        );

        DB.query(&query)
            .bind(("title", data.title))
            .bind(("description", data.description))
            .bind(("location", data.location))
            .bind(("contact_name", data.contact_name))
            .bind(("contact_email", data.contact_email))
            .bind(("contact_phone", data.contact_phone))
            .bind(("contact_website", data.contact_website))
            .bind((
                "applications_enabled",
                if data.applications_enabled {
                    "true"
                } else {
                    "false"
                },
            ))
            .bind(("expires_at", expires_at.to_rfc3339()))
            .await
            .map_err(|e| Error::Database(format!("Failed to update job: {}", e)))?;

        Ok(())
    }

    /// Delete a job posting (by bare record key) and all of its application
    /// edges.
    pub async fn delete(job_id: &str) -> Result<(), Error> {
        debug!("Deleting job posting: {}", job_id);

        validate_record_key(job_id)?;
        let job_rid = RecordId::new("job_posting", job_id);

        // Delete applications
        let delete_apps = format!("DELETE FROM application WHERE out = {}", job_rid.display());
        DB.query(&delete_apps)
            .await
            .map_err(|e| Error::Database(format!("Failed to delete applications: {}", e)))?;

        // Delete job
        let delete_job = format!("DELETE {}", job_rid.display());
        DB.query(&delete_job)
            .await
            .map_err(|e| Error::Database(format!("Failed to delete job: {}", e)))?;

        Ok(())
    }

    /// Set a posting's status to 'closed' (by bare record key).
    pub async fn close(job_id: &str) -> Result<(), Error> {
        validate_record_key(job_id)?;
        let job_rid = RecordId::new("job_posting", job_id);
        DB.query(format!(
            "UPDATE {} SET status = 'closed'",
            job_rid.display()
        ))
        .await
        .map_err(|e| Error::Database(format!("Failed to close job: {}", e)))?;
        Ok(())
    }

    /// Apply to a specific role on a job posting by RELATE-ing an
    /// `application` edge from the person to the posting.
    ///
    /// Both `person_id` and `job_id` are full `table:key` id strings (unlike
    /// the bare-key getters above). The duplicate-application check counts
    /// with `GROUP ALL` so the aggregate comes back as a single row.
    ///
    /// # Errors
    /// `Error::BadRequest` if either id is malformed or the person already
    /// has a live (non-withdrawn) application for that role.
    pub async fn apply(
        person_id: &str,
        job_id: &str,
        role_title: &str,
        cover_letter: Option<String>,
    ) -> Result<(), Error> {
        debug!(
            "Applying {} to job {} role '{}'",
            person_id, job_id, role_title
        );

        let person_record = parse_record_id(person_id)?;
        let job_record = parse_record_id(job_id)?;

        // Check not already applied to this role
        let check = format!(
            "SELECT count() AS count FROM application WHERE in = {} AND out = {} AND role_title = $role_title AND status != 'withdrawn' GROUP ALL",
            person_record.display(),
            job_record.display()
        );
        let mut check_result = DB
            .query(&check)
            .bind(("role_title", role_title.to_string()))
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let existing: Option<serde_json::Value> = check_result.take(0)?;
        if let Some(obj) = existing
            && obj.get("count").and_then(|c| c.as_i64()).unwrap_or(0) > 0
        {
            return Err(Error::BadRequest(
                "Already applied to this role".to_string(),
            ));
        }

        let query = format!(
            "RELATE {}->application->{} SET role_title = $role_title, cover_letter = $cover_letter",
            person_record.display(),
            job_record.display()
        );

        DB.query(&query)
            .bind(("role_title", role_title.to_string()))
            .bind(("cover_letter", cover_letter))
            .await
            .map_err(|e| Error::Database(format!("Failed to apply: {}", e)))?;

        Ok(())
    }

    /// Mark a person's application for one role as 'withdrawn' (soft delete).
    /// `person_id` and `job_id` are full `table:key` id strings.
    pub async fn withdraw(person_id: &str, job_id: &str, role_title: &str) -> Result<(), Error> {
        let person_record = parse_record_id(person_id)?;
        let job_record = parse_record_id(job_id)?;

        let query = format!(
            "UPDATE application SET status = 'withdrawn' WHERE in = {} AND out = {} AND role_title = $role_title",
            person_record.display(),
            job_record.display()
        );
        DB.query(&query)
            .bind(("role_title", role_title.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to withdraw: {}", e)))?;
        Ok(())
    }

    /// Set an application's status. `status` must satisfy the schema ASSERT
    /// on `application.status`: "submitted" | "reviewed" | "shortlisted" |
    /// "rejected" | "withdrawn"; `app_id` is a full `table:key` id string.
    pub async fn update_application_status(app_id: &str, status: &str) -> Result<(), Error> {
        let app_record = parse_record_id(app_id)?;
        DB.query(format!(
            "UPDATE {} SET status = $status",
            app_record.display()
        ))
        .bind(("status", status.to_string()))
        .await
        .map_err(|e| Error::Database(format!("Failed to update application: {}", e)))?;
        Ok(())
    }

    /// Get non-withdrawn applications for a posting (by bare record key),
    /// newest first, with applicant profile fields pulled through the `in`
    /// edge link.
    pub async fn get_applications(job_id: &str) -> Result<Vec<ApplicationView>, Error> {
        validate_record_key(job_id)?;
        let job_rid = RecordId::new("job_posting", job_id);

        let query = format!(
            r#"SELECT
                <string> id AS id,
                in.name ?? in.username AS applicant_name,
                in.username AS applicant_username,
                in.profile.avatar AS applicant_avatar,
                role_title,
                cover_letter,
                status,
                <string> applied_at AS applied_at
            FROM application
            WHERE out = {}
            AND status != 'withdrawn'
            ORDER BY applied_at DESC"#,
            job_rid.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        Ok(rows
            .iter()
            .map(|r| {
                let raw_id = r.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let id = raw_id
                    .strip_prefix("application:")
                    .unwrap_or(raw_id)
                    .to_string();
                ApplicationView {
                    id,
                    applicant_name: r
                        .get("applicant_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    applicant_username: r
                        .get("applicant_username")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    applicant_avatar: r
                        .get("applicant_avatar")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    role_title: r
                        .get("role_title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    cover_letter: r
                        .get("cover_letter")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    status: r
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("submitted")
                        .to_string(),
                    applied_at: r
                        .get("applied_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                }
            })
            .collect())
    }

    /// Get user's own applications
    pub async fn get_user_applications(person_id: &str) -> Result<Vec<UserApplicationView>, Error> {
        let person_record = parse_record_id(person_id)?;

        let query = format!(
            r#"SELECT
                <string> id AS id,
                <string> out AS job_id,
                out.title AS job_title,
                role_title,
                out.posted_by.name ?? 'Unknown' AS poster_name,
                cover_letter,
                status,
                <string> applied_at AS applied_at
            FROM application
            WHERE in = {}
            ORDER BY applied_at DESC"#,
            person_record.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        Ok(rows
            .iter()
            .map(|r| UserApplicationView {
                id: r
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                job_id: r
                    .get("job_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                job_title: r
                    .get("job_title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                role_title: r
                    .get("role_title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                poster_name: r
                    .get("poster_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                cover_letter: r
                    .get("cover_letter")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                status: r
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("submitted")
                    .to_string(),
                applied_at: r
                    .get("applied_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
            .collect())
    }

    /// Get user's posted jobs (direct + via org)
    pub async fn get_user_postings(person_id: &str) -> Result<Vec<JobListItem>, Error> {
        let person_record = parse_record_id(person_id)?;
        let person_display = person_record.display().to_string();

        let query = format!(
            r#"SELECT
                <string> id AS id,
                title,
                description,
                location,
                <string> posted_by AS posted_by_id,
                array::len(roles) AS role_count,
                status,
                <string> expires_at AS expires_at,
                <string> created_at AS created_at,
                <string> related_production AS related_production_id,
                applications_enabled
            FROM job_posting
            WHERE posted_by = {person_display}
            OR posted_by IN (
                SELECT VALUE out FROM member_of
                WHERE in = {person_display}
                AND <string> type::table(out) = 'organization'
                AND role IN ['owner', 'admin']
                AND invitation_status = 'accepted'
            )
            ORDER BY created_at DESC"#,
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        let mut jobs = Vec::new();
        for row in &rows {
            let posted_by_id = row
                .get("posted_by_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let (poster_name, poster_slug, poster_type, is_poster_verified) =
                Self::fetch_poster_info(posted_by_id).await;

            jobs.push(JobListItem {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                title: row
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: row
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                location: row
                    .get("location")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                poster_name,
                poster_slug,
                poster_type,
                is_poster_verified,
                role_count: row.get("role_count").and_then(|v| v.as_i64()).unwrap_or(0),
                status: row
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("open")
                    .to_string(),
                expires_at: row
                    .get("expires_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                created_at: row
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                production_title: None,
                production_poster: None,
                applications_enabled: row
                    .get("applications_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            });
        }

        Ok(jobs)
    }

    /// Check whether a person can edit a posting (by bare record key):
    /// either they posted it directly, or it was posted by an organization
    /// they own/administer with an accepted membership.
    pub async fn can_edit(job_id: &str, person_id: &str) -> Result<bool, Error> {
        validate_record_key(job_id)?;
        let job_rid = RecordId::new("job_posting", job_id);

        let query = format!(
            "SELECT <string> posted_by AS poster FROM ONLY {}",
            job_rid.display()
        );
        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let row: Option<serde_json::Value> = result.take(0)?;

        if let Some(obj) = row {
            let poster = obj.get("poster").and_then(|v| v.as_str()).unwrap_or("");
            if poster == person_id {
                return Ok(true);
            }

            if poster.starts_with("organization:") {
                let person_record = parse_record_id(person_id)?;
                let poster_record = parse_record_id(poster)?;
                let org_check = format!(
                    "SELECT role FROM member_of WHERE in = {} AND out = {} AND role IN ['owner', 'admin'] AND invitation_status = 'accepted'",
                    person_record.display(),
                    poster_record.display()
                );
                let mut org_result = DB
                    .query(&org_check)
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?;
                let org_member: Option<serde_json::Value> = org_result.take(0)?;
                if org_member.is_some() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Get pay rate types
    pub async fn get_pay_rate_types() -> Result<Vec<String>, Error> {
        let mut result = DB
            .query("SELECT name FROM pay_rate_type ORDER BY name")
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch pay rate types: {}", e)))?;

        let types: Vec<serde_json::Value> = result.take(0)?;
        Ok(types
            .into_iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
    }

    /// Get orgs where user is owner/admin
    pub async fn get_user_orgs_for_posting(
        person_id: &str,
    ) -> Result<Vec<(String, String)>, Error> {
        let person_record = parse_record_id(person_id)?;

        let query = format!(
            r#"SELECT
                <string> out AS org_id,
                out.name AS org_name
            FROM member_of
            WHERE in = {}
            AND <string> type::table(out) = 'organization'
            AND role IN ['owner', 'admin']
            AND invitation_status = 'accepted'
            ORDER BY out.name ASC"#,
            person_record.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let id = r.get("org_id").and_then(|v| v.as_str()).map(String::from)?;
                let name = r
                    .get("org_name")
                    .and_then(|v| v.as_str())
                    .map(String::from)?;
                Some((id, name))
            })
            .collect())
    }

    /// Get raw job data (by bare record key) plus parsed embedded roles, for
    /// pre-filling the edit form.
    pub async fn get_for_edit(
        job_id: &str,
    ) -> Result<(serde_json::Value, Vec<JobRoleView>), Error> {
        validate_record_key(job_id)?;
        let job_rid = RecordId::new("job_posting", job_id);

        let query = format!(
            r#"SELECT
                <string> id AS id,
                title,
                description,
                location,
                <string> posted_by AS posted_by,
                <string> related_production AS related_production,
                contact_name,
                contact_email,
                contact_phone,
                contact_website,
                applications_enabled,
                status,
                roles,
                <string> expires_at AS expires_at,
                <string> created_at AS created_at,
                <string> updated_at AS updated_at
            FROM ONLY {}"#,
            job_rid.display()
        );

        let mut result = DB
            .query(&query)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let job: Option<serde_json::Value> = result.take(0)?;
        let job = job.ok_or(Error::NotFound)?;

        let roles: Vec<JobRoleView> = job
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|r| JobRoleView {
                        title: r
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        description: r
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        rate_type: r
                            .get("rate_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("TBD")
                            .to_string(),
                        rate_amount: r
                            .get("rate_amount")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        location_override: r
                            .get("location_override")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        has_applied: false,
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok((job, roles))
    }

    /// Helper: fetch poster info from a record ID
    async fn fetch_poster_info(posted_by_id: &str) -> (String, String, String, bool) {
        if posted_by_id.is_empty() {
            return (
                "Unknown".to_string(),
                String::new(),
                "person".to_string(),
                false,
            );
        }

        let p_type = if posted_by_id.starts_with("organization:") {
            "organization"
        } else {
            "person"
        };
        let poster_record = match parse_record_id(posted_by_id) {
            Ok(r) => r,
            Err(_) => {
                return (
                    "Unknown".to_string(),
                    String::new(),
                    p_type.to_string(),
                    false,
                );
            }
        };
        let pq = format!(
            "SELECT name, slug ?? username ?? '' AS slug, (verified ?? false) OR (verification_status ?? '' = 'identity') AS verified FROM ONLY {}",
            poster_record.display()
        );

        if let Ok(mut pr) = DB.query(&pq).await {
            let p: Option<serde_json::Value> = pr.take(0).unwrap_or(None);
            if let Some(p) = p {
                return (
                    p.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                    p.get("slug")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    p_type.to_string(),
                    p.get("verified").and_then(|v| v.as_bool()).unwrap_or(false),
                );
            }
        }

        (
            "Unknown".to_string(),
            String::new(),
            p_type.to_string(),
            false,
        )
    }

    /// Helper: fetch production info from a record ID
    async fn fetch_production_info(
        related_prod_id: &str,
    ) -> (Option<String>, Option<String>, Option<String>) {
        if related_prod_id.is_empty() || related_prod_id == "NONE" {
            return (None, None, None);
        }

        let prod_record = match parse_record_id(related_prod_id) {
            Ok(r) => r,
            Err(_) => return (None, None, None),
        };
        let pdq = format!(
            "SELECT title, slug, poster_photo ?? poster_url ?? NONE AS poster FROM ONLY {}",
            prod_record.display()
        );

        if let Ok(mut pdr) = DB.query(&pdq).await {
            let pd: Option<serde_json::Value> = pdr.take(0).unwrap_or(None);
            if let Some(p) = pd {
                return (
                    p.get("title").and_then(|v| v.as_str()).map(String::from),
                    p.get("slug").and_then(|v| v.as_str()).map(String::from),
                    p.get("poster").and_then(|v| v.as_str()).map(String::from),
                );
            }
        }

        (None, None, None)
    }
}
