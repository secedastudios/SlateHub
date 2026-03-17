use crate::db::DB;
use crate::error::Error;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use crate::record_id_ext::RecordIdExt;
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
    pub poster_type: String,
    pub is_poster_verified: bool,
    pub role_count: i64,
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
    pub poster_type: String,
    pub is_poster_verified: bool,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_website: Option<String>,
    pub applications_enabled: bool,
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
    pub expires_in: String,
}

/// Data for a role (embedded in job posting)
#[derive(Debug, Clone, Serialize)]
pub struct CreateJobRoleData {
    pub title: String,
    pub description: Option<String>,
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
    pub expires_in: String,
}

pub struct JobModel;

impl JobModel {
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
            poster_record.display(), roles_json, prod_clause
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
            .bind(("applications_enabled", if data.applications_enabled { "true" } else { "false" }))
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
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobListItem>, Error> {
        debug!("Listing jobs, search: {:?}", search);

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
                applications_enabled
            FROM job_posting
            WHERE status = 'open'
            AND expires_at > time::now()"#,
        );

        if search.is_some() {
            query.push_str(
                " AND (string::lowercase(title) CONTAINS string::lowercase($search) \
                 OR string::lowercase(description) CONTAINS string::lowercase($search) \
                 OR string::lowercase(string::join(' ', roles.*.title)) CONTAINS string::lowercase($search))",
            );
        }

        query.push_str(" ORDER BY created_at DESC");
        query.push_str(&format!(" LIMIT {}", limit));
        if offset > 0 {
            query.push_str(&format!(" START {}", offset));
        }

        let mut db_query = DB.query(&query);
        if let Some(s) = search {
            db_query = db_query.bind(("search", s.to_string()));
        }

        let mut result = db_query
            .await
            .map_err(|e| Error::Database(format!("Failed to list jobs: {}", e)))?;

        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        let mut jobs = Vec::new();
        for row in &rows {
            let posted_by_id = row.get("posted_by_id").and_then(|v| v.as_str()).unwrap_or("");
            let (poster_name, poster_slug, poster_type, is_poster_verified) = Self::fetch_poster_info(posted_by_id).await;

            let related_prod_id = row.get("related_production_id").and_then(|v| v.as_str()).unwrap_or("");
            let (production_title, _slug, production_poster) = Self::fetch_production_info(related_prod_id).await;

            jobs.push(JobListItem {
                id: row.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                title: row.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                description: row.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                location: row.get("location").and_then(|v| v.as_str()).map(String::from),
                poster_name,
                poster_slug,
                poster_type,
                is_poster_verified,
                role_count: row.get("role_count").and_then(|v| v.as_i64()).unwrap_or(0),
                status: row.get("status").and_then(|v| v.as_str()).unwrap_or("open").to_string(),
                expires_at: row.get("expires_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                created_at: row.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                production_title,
                production_poster,
                applications_enabled: row.get("applications_enabled").and_then(|v| v.as_bool()).unwrap_or(true),
            });
        }

        // Sort verified posters first, then by created_at (already DESC from query)
        jobs.sort_by(|a, b| b.is_poster_verified.cmp(&a.is_poster_verified));

        Ok(jobs)
    }

    /// Get full job detail by key
    pub async fn get(
        key: &str,
        current_user_id: Option<&str>,
    ) -> Result<JobDetailView, Error> {
        debug!("Fetching job: {}", key);

        validate_record_key(key)?;
        let job_id = RecordId::new("job_posting", key);

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
            job_id.display()
        );

        let mut result = DB
            .query(&basic_query)
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch job: {}", e)))?;

        let job: Option<serde_json::Value> = result.take(0)?;
        let job = job.ok_or(Error::NotFound)?;

        let posted_by_id = job.get("posted_by_id").and_then(|v| v.as_str()).unwrap_or("");
        let (poster_name, poster_slug, poster_type, is_poster_verified) = Self::fetch_poster_info(posted_by_id).await;

        let related_prod_id = job.get("related_production_id").and_then(|v| v.as_str()).unwrap_or("");
        let (production_title, production_slug, production_poster) = Self::fetch_production_info(related_prod_id).await;

        // Parse embedded roles array
        let mut roles: Vec<JobRoleView> = job.get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(|r| JobRoleView {
                title: r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                description: r.get("description").and_then(|v| v.as_str()).map(String::from),
                rate_type: r.get("rate_type").and_then(|v| v.as_str()).unwrap_or("TBD").to_string(),
                rate_amount: r.get("rate_amount").and_then(|v| v.as_str()).map(String::from),
                location_override: r.get("location_override").and_then(|v| v.as_str()).map(String::from),
                has_applied: false,
            }).collect())
            .unwrap_or_default();

        // Check per-role application state for current user
        if let Some(uid) = current_user_id {
            let uid_record = parse_record_id(uid)?;
            let app_query = format!(
                "SELECT role_title FROM application WHERE in = {} AND out = {} AND status != 'withdrawn'",
                uid_record.display(), job_id.display()
            );
            if let Ok(mut ar) = DB.query(&app_query).await {
                let app_rows: Vec<serde_json::Value> = ar.take(0).unwrap_or_default();
                let applied_titles: Vec<String> = app_rows.iter()
                    .filter_map(|r| r.get("role_title").and_then(|v| v.as_str()).map(String::from))
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
                job_id.display()
            );
            if let Ok(mut cr) = DB.query(&count_query).await {
                let v: Option<serde_json::Value> = cr.take(0).unwrap_or(None);
                v.and_then(|o| o.get("count").and_then(|c| c.as_i64())).unwrap_or(0)
            } else {
                0
            }
        };

        let can_edit = if let Some(uid) = current_user_id {
            Self::can_edit(key, uid).await.unwrap_or(false)
        } else {
            false
        };

        // Fetch applications if user can edit this job
        let applications = if can_edit {
            Self::get_applications(key).await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let expires_str = job.get("expires_at").and_then(|v| v.as_str()).unwrap_or("");
        let is_expired = DateTime::parse_from_rfc3339(expires_str)
            .map(|dt| dt < Utc::now())
            .unwrap_or(false);

        Ok(JobDetailView {
            id: job.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title: job.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: job.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            location: job.get("location").and_then(|v| v.as_str()).map(String::from),
            poster_name,
            poster_slug,
            poster_type,
            is_poster_verified,
            contact_name: job.get("contact_name").and_then(|v| v.as_str()).map(String::from),
            contact_email: job.get("contact_email").and_then(|v| v.as_str()).map(String::from),
            contact_phone: job.get("contact_phone").and_then(|v| v.as_str()).map(String::from),
            contact_website: job.get("contact_website").and_then(|v| v.as_str()).map(String::from),
            applications_enabled: job.get("applications_enabled").and_then(|v| v.as_bool()).unwrap_or(true),
            status: job.get("status").and_then(|v| v.as_str()).unwrap_or("open").to_string(),
            expires_at: expires_str.to_string(),
            created_at: job.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            updated_at: job.get("updated_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
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

    /// Update a job posting
    pub async fn update(
        key: &str,
        data: UpdateJobData,
        roles: Vec<CreateJobRoleData>,
    ) -> Result<(), Error> {
        debug!("Updating job posting: {}", key);

        validate_record_key(key)?;
        let job_id = RecordId::new("job_posting", key);
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
            job_id.display(), roles_json
        );

        DB.query(&query)
            .bind(("title", data.title))
            .bind(("description", data.description))
            .bind(("location", data.location))
            .bind(("contact_name", data.contact_name))
            .bind(("contact_email", data.contact_email))
            .bind(("contact_phone", data.contact_phone))
            .bind(("contact_website", data.contact_website))
            .bind(("applications_enabled", if data.applications_enabled { "true" } else { "false" }))
            .bind(("expires_at", expires_at.to_rfc3339()))
            .await
            .map_err(|e| Error::Database(format!("Failed to update job: {}", e)))?;

        Ok(())
    }

    /// Delete a job posting and its applications
    pub async fn delete(key: &str) -> Result<(), Error> {
        debug!("Deleting job posting: {}", key);

        validate_record_key(key)?;
        let job_id = RecordId::new("job_posting", key);

        // Delete applications
        let delete_apps = format!("DELETE FROM application WHERE out = {}", job_id.display());
        DB.query(&delete_apps).await.map_err(|e| Error::Database(format!("Failed to delete applications: {}", e)))?;

        // Delete job
        let delete_job = format!("DELETE {}", job_id.display());
        DB.query(&delete_job).await.map_err(|e| Error::Database(format!("Failed to delete job: {}", e)))?;

        Ok(())
    }

    /// Close a job posting
    pub async fn close(key: &str) -> Result<(), Error> {
        validate_record_key(key)?;
        let job_id = RecordId::new("job_posting", key);
        DB.query(&format!("UPDATE {} SET status = 'closed'", job_id.display()))
            .await
            .map_err(|e| Error::Database(format!("Failed to close job: {}", e)))?;
        Ok(())
    }

    /// Apply to a specific role on a job posting
    pub async fn apply(
        person_id: &str,
        job_id: &str,
        role_title: &str,
        cover_letter: Option<String>,
    ) -> Result<(), Error> {
        debug!("Applying {} to job {} role '{}'", person_id, job_id, role_title);

        let person_record = parse_record_id(person_id)?;
        let job_record = parse_record_id(job_id)?;

        // Check not already applied to this role
        let check = format!(
            "SELECT count() AS count FROM application WHERE in = {} AND out = {} AND role_title = $role_title AND status != 'withdrawn' GROUP ALL",
            person_record.display(), job_record.display()
        );
        let mut check_result = DB.query(&check)
            .bind(("role_title", role_title.to_string()))
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let existing: Option<serde_json::Value> = check_result.take(0)?;
        if let Some(obj) = existing {
            if obj.get("count").and_then(|c| c.as_i64()).unwrap_or(0) > 0 {
                return Err(Error::BadRequest("Already applied to this role".to_string()));
            }
        }

        let query = format!(
            "RELATE {}->application->{} SET role_title = $role_title, cover_letter = $cover_letter",
            person_record.display(), job_record.display()
        );

        DB.query(&query)
            .bind(("role_title", role_title.to_string()))
            .bind(("cover_letter", cover_letter))
            .await
            .map_err(|e| Error::Database(format!("Failed to apply: {}", e)))?;

        Ok(())
    }

    /// Withdraw an application for a specific role
    pub async fn withdraw(person_id: &str, job_id: &str, role_title: &str) -> Result<(), Error> {
        let person_record = parse_record_id(person_id)?;
        let job_record = parse_record_id(job_id)?;

        let query = format!(
            "UPDATE application SET status = 'withdrawn' WHERE in = {} AND out = {} AND role_title = $role_title",
            person_record.display(), job_record.display()
        );
        DB.query(&query)
            .bind(("role_title", role_title.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to withdraw: {}", e)))?;
        Ok(())
    }

    /// Update application status
    pub async fn update_application_status(app_id: &str, status: &str) -> Result<(), Error> {
        let app_record = parse_record_id(app_id)?;
        DB.query(&format!("UPDATE {} SET status = $status", app_record.display()))
            .bind(("status", status.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to update application: {}", e)))?;
        Ok(())
    }

    /// Get applications for a job posting
    pub async fn get_applications(key: &str) -> Result<Vec<ApplicationView>, Error> {
        validate_record_key(key)?;
        let job_id = RecordId::new("job_posting", key);

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
            job_id.display()
        );

        let mut result = DB.query(&query).await.map_err(|e| Error::Database(e.to_string()))?;
        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        Ok(rows.iter().map(|r| {
            let raw_id = r.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let id = raw_id.strip_prefix("application:").unwrap_or(raw_id).to_string();
            ApplicationView {
            id,
            applicant_name: r.get("applicant_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            applicant_username: r.get("applicant_username").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            applicant_avatar: r.get("applicant_avatar").and_then(|v| v.as_str()).map(String::from),
            role_title: r.get("role_title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            cover_letter: r.get("cover_letter").and_then(|v| v.as_str()).map(String::from),
            status: r.get("status").and_then(|v| v.as_str()).unwrap_or("submitted").to_string(),
            applied_at: r.get("applied_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }}).collect())
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

        let mut result = DB.query(&query).await.map_err(|e| Error::Database(e.to_string()))?;
        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        Ok(rows.iter().map(|r| UserApplicationView {
            id: r.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            job_id: r.get("job_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            job_title: r.get("job_title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            role_title: r.get("role_title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            poster_name: r.get("poster_name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
            cover_letter: r.get("cover_letter").and_then(|v| v.as_str()).map(String::from),
            status: r.get("status").and_then(|v| v.as_str()).unwrap_or("submitted").to_string(),
            applied_at: r.get("applied_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }).collect())
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

        let mut result = DB.query(&query).await.map_err(|e| Error::Database(e.to_string()))?;
        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();

        let mut jobs = Vec::new();
        for row in &rows {
            let posted_by_id = row.get("posted_by_id").and_then(|v| v.as_str()).unwrap_or("");
            let (poster_name, poster_slug, poster_type, is_poster_verified) = Self::fetch_poster_info(posted_by_id).await;

            jobs.push(JobListItem {
                id: row.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                title: row.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                description: row.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                location: row.get("location").and_then(|v| v.as_str()).map(String::from),
                poster_name,
                poster_slug,
                poster_type,
                is_poster_verified,
                role_count: row.get("role_count").and_then(|v| v.as_i64()).unwrap_or(0),
                status: row.get("status").and_then(|v| v.as_str()).unwrap_or("open").to_string(),
                expires_at: row.get("expires_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                created_at: row.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                production_title: None,
                production_poster: None,
                applications_enabled: row.get("applications_enabled").and_then(|v| v.as_bool()).unwrap_or(true),
            });
        }

        Ok(jobs)
    }

    /// Check if user can edit a job
    pub async fn can_edit(key: &str, user_id: &str) -> Result<bool, Error> {
        validate_record_key(key)?;
        let job_id = RecordId::new("job_posting", key);

        let query = format!("SELECT <string> posted_by AS poster FROM ONLY {}", job_id.display());
        let mut result = DB.query(&query).await.map_err(|e| Error::Database(e.to_string()))?;
        let row: Option<serde_json::Value> = result.take(0)?;

        if let Some(obj) = row {
            let poster = obj.get("poster").and_then(|v| v.as_str()).unwrap_or("");
            if poster == user_id {
                return Ok(true);
            }

            if poster.starts_with("organization:") {
                let user_record = parse_record_id(user_id)?;
                let poster_record = parse_record_id(poster)?;
                let org_check = format!(
                    "SELECT role FROM member_of WHERE in = {} AND out = {} AND role IN ['owner', 'admin'] AND invitation_status = 'accepted'",
                    user_record.display(), poster_record.display()
                );
                let mut org_result = DB.query(&org_check).await.map_err(|e| Error::Database(e.to_string()))?;
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
    pub async fn get_user_orgs_for_posting(person_id: &str) -> Result<Vec<(String, String)>, Error> {
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

        let mut result = DB.query(&query).await.map_err(|e| Error::Database(e.to_string()))?;
        let rows: Vec<serde_json::Value> = result.take(0).unwrap_or_default();
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let id = r.get("org_id").and_then(|v| v.as_str()).map(String::from)?;
                let name = r.get("org_name").and_then(|v| v.as_str()).map(String::from)?;
                Some((id, name))
            })
            .collect())
    }

    /// Get job data for editing
    pub async fn get_for_edit(key: &str) -> Result<(serde_json::Value, Vec<JobRoleView>), Error> {
        validate_record_key(key)?;
        let job_id = RecordId::new("job_posting", key);

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
            job_id.display()
        );

        let mut result = DB.query(&query).await.map_err(|e| Error::Database(e.to_string()))?;
        let job: Option<serde_json::Value> = result.take(0)?;
        let job = job.ok_or(Error::NotFound)?;

        let roles: Vec<JobRoleView> = job.get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(|r| JobRoleView {
                title: r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                description: r.get("description").and_then(|v| v.as_str()).map(String::from),
                rate_type: r.get("rate_type").and_then(|v| v.as_str()).unwrap_or("TBD").to_string(),
                rate_amount: r.get("rate_amount").and_then(|v| v.as_str()).map(String::from),
                location_override: r.get("location_override").and_then(|v| v.as_str()).map(String::from),
                has_applied: false,
            }).collect())
            .unwrap_or_default();

        Ok((job, roles))
    }

    /// Helper: fetch poster info from a record ID
    async fn fetch_poster_info(posted_by_id: &str) -> (String, String, String, bool) {
        if posted_by_id.is_empty() {
            return ("Unknown".to_string(), String::new(), "person".to_string(), false);
        }

        let p_type = if posted_by_id.starts_with("organization:") { "organization" } else { "person" };
        let poster_record = match parse_record_id(posted_by_id) {
            Ok(r) => r,
            Err(_) => return ("Unknown".to_string(), String::new(), p_type.to_string(), false),
        };
        let pq = format!(
            "SELECT name, slug ?? username ?? '' AS slug, verified ?? false AS verified FROM ONLY {}",
            poster_record.display()
        );

        if let Ok(mut pr) = DB.query(&pq).await {
            let p: Option<serde_json::Value> = pr.take(0).unwrap_or(None);
            if let Some(p) = p {
                return (
                    p.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                    p.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    p_type.to_string(),
                    p.get("verified").and_then(|v| v.as_bool()).unwrap_or(false),
                );
            }
        }

        ("Unknown".to_string(), String::new(), p_type.to_string(), false)
    }

    /// Helper: fetch production info from a record ID
    async fn fetch_production_info(related_prod_id: &str) -> (Option<String>, Option<String>, Option<String>) {
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
