use crate::error::Error;
use crate::middleware::{AuthenticatedUser, UserExtractor};
use crate::models::job::{
    CreateJobData, CreateJobRoleData, JobModel, UpdateJobData,
};
use crate::templates::{
    BaseContext, JobCreateTemplate, JobDetailView, JobEditTemplate, JobListView,
    JobOrgOption, JobRoleEditData, JobTemplate, JobsTemplate,
    MyJobsTemplate, User, UserApplicationView,
};
use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, Request},
    http::header,
    response::{Html, Redirect, Response, IntoResponse},
    routing::{get, post},
};
use axum_extra::extract::Form;
use serde::Deserialize;
use tracing::{debug, error, info};

const JOBS_PAGE_SIZE: usize = 20;

pub fn router() -> Router {
    Router::new()
        .route("/jobs", get(list_jobs))
        .route("/my-jobs", get(my_jobs))
        .route("/jobs/new", get(new_job_form).post(create_job))
        .route("/jobs/{id}", get(view_job))
        .route("/jobs/{id}/edit", get(edit_job_form).post(update_job))
        .route("/jobs/{id}/delete", post(delete_job))
        .route("/jobs/{id}/close", post(close_job))
        .route("/jobs/{id}/roles/{role_index}/apply", post(apply_to_role))
        .route("/jobs/{id}/roles/{role_index}/withdraw", post(withdraw_from_role))
        .route(
            "/jobs/{id}/applications/{app_id}/status",
            post(update_app_status),
        )
        .route("/api/jobs/more-sse", get(jobs_more_sse))
}

#[derive(Debug, Deserialize)]
struct JobsQuery {
    q: Option<String>,
}

/// List all jobs
async fn list_jobs(
    Query(params): Query<JobsQuery>,
    request: Request,
) -> Result<Html<String>, Error> {
    let mut base = BaseContext::new().with_page("jobs");
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let search = params.q.as_deref().filter(|s| !s.is_empty());
    let all_jobs = JobModel::list(search, JOBS_PAGE_SIZE + 1, 0).await.unwrap_or_default();
    let has_more = all_jobs.len() > JOBS_PAGE_SIZE;
    let jobs: Vec<JobListView> = all_jobs
        .into_iter()
        .take(JOBS_PAGE_SIZE)
        .map(|j| JobListView {
            id: j.id.strip_prefix("job_posting:").unwrap_or(&j.id).to_string(),
            title: j.title,
            description: j.description,
            location: j.location,
            poster_name: j.poster_name,
            poster_slug: j.poster_slug,
            poster_type: j.poster_type,
            is_poster_verified: j.is_poster_verified,
            role_count: j.role_count,
            status: j.status,
            expires_at: j.expires_at,
            created_at: j.created_at,
            production_title: j.production_title,
            production_poster: j.production_poster,
            applications_enabled: j.applications_enabled,
        })
        .collect();

    let template = JobsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        jobs,
        search_query: params.q,
        has_more,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render jobs template: {}", e);
        Error::template(e.to_string())
    })?))
}

/// View a single job
async fn view_job(
    Path(id): Path<String>,
    request: Request,
) -> Result<Html<String>, Error> {
    let mut base = BaseContext::new().with_page("jobs");
    let current_user_id = if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
        Some(user.id.clone())
    } else {
        None
    };

    let detail = JobModel::get(&id, current_user_id.as_deref()).await?;

    let job = JobDetailView {
        id: detail.id.strip_prefix("job_posting:").unwrap_or(&detail.id).to_string(),
        title: detail.title,
        description: detail.description,
        location: detail.location,
        poster_name: detail.poster_name,
        poster_slug: detail.poster_slug,
        poster_type: detail.poster_type,
        is_poster_verified: detail.is_poster_verified,
        contact_name: detail.contact_name,
        contact_email: detail.contact_email,
        contact_phone: detail.contact_phone,
        contact_website: detail.contact_website,
        applications_enabled: detail.applications_enabled,
        status: detail.status,
        expires_at: detail.expires_at,
        created_at: detail.created_at,
        updated_at: detail.updated_at,
        roles: detail.roles,
        production_title: detail.production_title,
        production_slug: detail.production_slug,
        production_poster: detail.production_poster,
        can_edit: detail.can_edit,
        is_expired: detail.is_expired,
        application_count: detail.application_count,
        applications: detail.applications,
    };

    let template = JobTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        job,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render job template: {}", e);
        Error::template(e.to_string())
    })?))
}

/// Show create job form
async fn new_job_form(request: Request) -> Result<Html<String>, Error> {
    let user = request.get_user().ok_or(Error::Unauthorized)?;
    let mut base = BaseContext::new().with_page("jobs");
    base = base.with_user(User::from_session_user(&user).await);

    let pay_rate_types = JobModel::get_pay_rate_types().await.unwrap_or_default();
    let orgs = JobModel::get_user_orgs_for_posting(&user.id).await.unwrap_or_default();

    let user_organizations: Vec<JobOrgOption> = orgs
        .into_iter()
        .map(|(id, name)| JobOrgOption { id, name })
        .collect();

    let template = JobCreateTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        pay_rate_types,
        user_organizations,
        errors: None,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render job create template: {}", e);
        Error::template(e.to_string())
    })?))
}

#[derive(Debug, Deserialize)]
struct CreateJobForm {
    title: String,
    description: String,
    location: Option<String>,
    post_as: Option<String>,
    related_production: Option<String>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    contact_phone: Option<String>,
    contact_website: Option<String>,
    applications_enabled: Option<String>,
    expires_in: String,
    #[serde(default, rename = "role_title[]")]
    role_title: Vec<String>,
    #[serde(default, rename = "role_description[]")]
    role_description: Vec<String>,
    #[serde(default, rename = "role_rate_type[]")]
    role_rate_type: Vec<String>,
    #[serde(default, rename = "role_rate_amount[]")]
    role_rate_amount: Vec<String>,
    #[serde(default, rename = "role_location[]")]
    role_location: Vec<String>,
}

/// Create a new job posting
async fn create_job(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<CreateJobForm>,
) -> Result<Response, Error> {
    debug!("Creating job: {}", data.title);

    if data.title.is_empty() {
        return Err(Error::Validation("Title is required".to_string()));
    }
    if data.description.is_empty() {
        return Err(Error::Validation("Description is required".to_string()));
    }

    // Determine poster
    let poster_id = if let Some(ref org_id) = data.post_as {
        if !org_id.is_empty() && org_id != &user.id {
            let org_model = crate::models::organization::OrganizationModel::new();
            let role = org_model.get_member_role(org_id, &user.id).await?;
            match role.as_deref() {
                Some("owner") | Some("admin") => {}
                _ => return Err(Error::Forbidden),
            }
            org_id.clone()
        } else {
            user.id.clone()
        }
    } else {
        user.id.clone()
    };

    // Build roles from parallel arrays
    let mut roles = Vec::new();
    for i in 0..data.role_title.len() {
        let title = data.role_title.get(i).cloned().unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        roles.push(CreateJobRoleData {
            title,
            description: data.role_description.get(i).cloned().filter(|s| !s.is_empty()),
            rate_type: data.role_rate_type.get(i).cloned().unwrap_or_else(|| "TBD".to_string()),
            rate_amount: data.role_rate_amount.get(i).cloned().filter(|s| !s.is_empty()),
            location_override: data.role_location.get(i).cloned().filter(|s| !s.is_empty()),
        });
    }

    if roles.is_empty() {
        return Err(Error::Validation("At least one role is required".to_string()));
    }

    let job_data = CreateJobData {
        title: data.title,
        description: data.description,
        location: data.location.filter(|s| !s.is_empty()),
        contact_name: data.contact_name.filter(|s| !s.is_empty()),
        contact_email: data.contact_email.filter(|s| !s.is_empty()),
        contact_phone: data.contact_phone.filter(|s| !s.is_empty()),
        contact_website: data.contact_website.filter(|s| !s.is_empty()),
        applications_enabled: data.applications_enabled.as_deref() == Some("on"),
        related_production: data.related_production.filter(|s| !s.is_empty()),
        expires_in: data.expires_in,
    };

    let key = JobModel::create(job_data, roles, &poster_id).await?;

    info!("Created job posting: {}", key);
    Ok(Redirect::to(&format!("/jobs/{}", key)).into_response())
}

/// Show edit job form
async fn edit_job_form(
    Path(id): Path<String>,
    request: Request,
) -> Result<Html<String>, Error> {
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    if !JobModel::can_edit(&id, &user.id).await.unwrap_or(false) {
        return Err(Error::Forbidden);
    }

    let mut base = BaseContext::new().with_page("jobs");
    base = base.with_user(User::from_session_user(&user).await);

    let (job, roles) = JobModel::get_for_edit(&id).await?;
    let pay_rate_types = JobModel::get_pay_rate_types().await.unwrap_or_default();

    let roles: Vec<JobRoleEditData> = roles
        .into_iter()
        .map(|r| JobRoleEditData {
            title: r.title,
            description: r.description,
            rate_type: r.rate_type,
            rate_amount: r.rate_amount,
            location_override: r.location_override,
        })
        .collect();

    let template = JobEditTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        job_id: id,
        title: job.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        description: job.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        location: job.get("location").and_then(|v| v.as_str()).map(String::from),
        contact_name: job.get("contact_name").and_then(|v| v.as_str()).map(String::from),
        contact_email: job.get("contact_email").and_then(|v| v.as_str()).map(String::from),
        contact_phone: job.get("contact_phone").and_then(|v| v.as_str()).map(String::from),
        contact_website: job.get("contact_website").and_then(|v| v.as_str()).map(String::from),
        applications_enabled: job.get("applications_enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        roles,
        pay_rate_types,
        errors: None,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render job edit template: {}", e);
        Error::template(e.to_string())
    })?))
}

#[derive(Debug, Deserialize)]
struct UpdateJobForm {
    title: String,
    description: String,
    location: Option<String>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    contact_phone: Option<String>,
    contact_website: Option<String>,
    applications_enabled: Option<String>,
    expires_in: String,
    #[serde(default, rename = "role_title[]")]
    role_title: Vec<String>,
    #[serde(default, rename = "role_description[]")]
    role_description: Vec<String>,
    #[serde(default, rename = "role_rate_type[]")]
    role_rate_type: Vec<String>,
    #[serde(default, rename = "role_rate_amount[]")]
    role_rate_amount: Vec<String>,
    #[serde(default, rename = "role_location[]")]
    role_location: Vec<String>,
}

/// Update a job posting
async fn update_job(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
    Form(data): Form<UpdateJobForm>,
) -> Result<Response, Error> {
    if !JobModel::can_edit(&id, &user.id).await.unwrap_or(false) {
        return Err(Error::Forbidden);
    }

    let mut roles = Vec::new();
    for i in 0..data.role_title.len() {
        let title = data.role_title.get(i).cloned().unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        roles.push(CreateJobRoleData {
            title,
            description: data.role_description.get(i).cloned().filter(|s| !s.is_empty()),
            rate_type: data.role_rate_type.get(i).cloned().unwrap_or_else(|| "TBD".to_string()),
            rate_amount: data.role_rate_amount.get(i).cloned().filter(|s| !s.is_empty()),
            location_override: data.role_location.get(i).cloned().filter(|s| !s.is_empty()),
        });
    }

    if roles.is_empty() {
        return Err(Error::Validation("At least one role is required".to_string()));
    }

    let update_data = UpdateJobData {
        title: data.title,
        description: data.description,
        location: data.location.filter(|s| !s.is_empty()),
        contact_name: data.contact_name.filter(|s| !s.is_empty()),
        contact_email: data.contact_email.filter(|s| !s.is_empty()),
        contact_phone: data.contact_phone.filter(|s| !s.is_empty()),
        contact_website: data.contact_website.filter(|s| !s.is_empty()),
        applications_enabled: data.applications_enabled.as_deref() == Some("on"),
        expires_in: data.expires_in,
    };

    JobModel::update(&id, update_data, roles).await?;

    info!("Updated job posting: {}", id);
    Ok(Redirect::to(&format!("/jobs/{}", id)).into_response())
}

/// Delete a job posting
async fn delete_job(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Response, Error> {
    if !JobModel::can_edit(&id, &user.id).await.unwrap_or(false) {
        return Err(Error::Forbidden);
    }

    JobModel::delete(&id).await?;
    info!("Deleted job posting: {}", id);
    Ok(Redirect::to("/jobs").into_response())
}

/// Close a job posting
async fn close_job(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Response, Error> {
    if !JobModel::can_edit(&id, &user.id).await.unwrap_or(false) {
        return Err(Error::Forbidden);
    }

    JobModel::close(&id).await?;
    info!("Closed job posting: {}", id);
    Ok(Redirect::to(&format!("/jobs/{}", id)).into_response())
}

#[derive(Debug, Deserialize)]
struct ApplyForm {
    cover_letter: Option<String>,
}

/// Apply to a specific role on a job
async fn apply_to_role(
    AuthenticatedUser(user): AuthenticatedUser,
    Path((id, role_index)): Path<(String, usize)>,
    Form(data): Form<ApplyForm>,
) -> Result<Response, Error> {
    let detail = JobModel::get(&id, Some(&user.id)).await?;
    let role = detail.roles.get(role_index)
        .ok_or_else(|| Error::BadRequest("Invalid role index".to_string()))?;

    let full_job_id = format!("job_posting:{}", id);
    JobModel::apply(
        &user.id,
        &full_job_id,
        &role.title,
        data.cover_letter.filter(|s| !s.is_empty()),
    )
    .await?;

    info!("User {} applied to job {} role '{}'", user.id, id, role.title);
    Ok(Redirect::to(&format!("/jobs/{}", id)).into_response())
}

/// Withdraw application from a specific role
async fn withdraw_from_role(
    AuthenticatedUser(user): AuthenticatedUser,
    Path((id, role_index)): Path<(String, usize)>,
) -> Result<Response, Error> {
    let detail = JobModel::get(&id, Some(&user.id)).await?;
    let role = detail.roles.get(role_index)
        .ok_or_else(|| Error::BadRequest("Invalid role index".to_string()))?;

    let full_job_id = format!("job_posting:{}", id);
    JobModel::withdraw(&user.id, &full_job_id, &role.title).await?;

    info!("User {} withdrew from job {} role '{}'", user.id, id, role.title);
    Ok(Redirect::to(&format!("/jobs/{}", id)).into_response())
}

#[derive(Debug, Deserialize)]
struct UpdateStatusForm {
    status: String,
}

/// Update application status
async fn update_app_status(
    AuthenticatedUser(user): AuthenticatedUser,
    Path((id, app_id)): Path<(String, String)>,
    Form(data): Form<UpdateStatusForm>,
) -> Result<Response, Error> {
    if !JobModel::can_edit(&id, &user.id).await.unwrap_or(false) {
        return Err(Error::Forbidden);
    }

    let full_app_id = format!("application:{}", app_id);
    JobModel::update_application_status(&full_app_id, &data.status).await?;

    info!("Updated application {} to {}", app_id, data.status);
    Ok(Redirect::to(&format!("/jobs/{}", id)).into_response())
}

/// My jobs page
async fn my_jobs(request: Request) -> Result<Html<String>, Error> {
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    let mut base = BaseContext::new().with_page("my-jobs");
    base = base.with_user(User::from_session_user(&user).await);

    let postings = JobModel::get_user_postings(&user.id).await.unwrap_or_default();
    let applications = JobModel::get_user_applications(&user.id).await.unwrap_or_default();

    let postings: Vec<JobListView> = postings
        .into_iter()
        .map(|j| JobListView {
            id: j.id.strip_prefix("job_posting:").unwrap_or(&j.id).to_string(),
            title: j.title,
            description: j.description,
            location: j.location,
            poster_name: j.poster_name,
            poster_slug: j.poster_slug,
            poster_type: j.poster_type,
            is_poster_verified: j.is_poster_verified,
            role_count: j.role_count,
            status: j.status,
            expires_at: j.expires_at,
            created_at: j.created_at,
            production_title: j.production_title,
            production_poster: j.production_poster,
            applications_enabled: j.applications_enabled,
        })
        .collect();

    let applications: Vec<UserApplicationView> = applications
        .into_iter()
        .map(|a| UserApplicationView {
            id: a.id,
            job_id: a.job_id.strip_prefix("job_posting:").unwrap_or(&a.job_id).to_string(),
            job_title: a.job_title,
            role_title: a.role_title,
            poster_name: a.poster_name,
            cover_letter: a.cover_letter,
            status: a.status,
            applied_at: a.applied_at,
        })
        .collect();

    let template = MyJobsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        postings,
        applications,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render my jobs template: {}", e);
        Error::template(e.to_string())
    })?))
}

// === SSE infinite scroll ===

fn sse_patch_elements(selector: &str, mode: &str, elements: &str) -> String {
    let mut s = format!(
        "event: datastar-patch-elements\ndata: selector {}\ndata: mode {}\n",
        selector, mode
    );
    if !elements.is_empty() {
        s += &format!("data: elements {}\n", elements.replace('\n', " "));
    }
    s += "\n";
    s
}

fn sse_response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

fn escape_html(s: &str) -> String {
    ammonia::clean_text(s)
}

const VERIFIED_BADGE_PATH: &str = "M22.5 12.5c0-1.58-.875-2.95-2.148-3.6.154-.435.238-.905.238-1.4 0-2.21-1.71-3.998-3.818-3.998-.47 0-.92.084-1.336.25C14.818 2.415 13.51 1.5 12 1.5s-2.816.917-3.437 2.25c-.415-.165-.866-.25-1.336-.25-2.11 0-3.818 1.79-3.818 4 0 .494.083.964.237 1.4-1.272.65-2.147 2.018-2.147 3.6 0 1.495.782 2.798 1.942 3.486-.02.17-.032.34-.032.514 0 2.21 1.708 4 3.818 4 .47 0 .92-.086 1.335-.25.62 1.334 1.926 2.25 3.437 2.25 1.512 0 2.818-.916 3.437-2.25.415.163.865.248 1.336.248 2.11 0 3.818-1.79 3.818-4 0-.174-.012-.344-.033-.513 1.158-.687 1.943-1.99 1.943-3.484zm-6.616-3.334l-4.334 6.5c-.145.217-.382.334-.625.334-.143 0-.288-.04-.416-.126l-.115-.094-2.415-2.415c-.293-.293-.293-.768 0-1.06s.768-.294 1.06 0l1.77 1.767 3.825-5.74c.23-.345.696-.436 1.04-.207.346.23.44.696.21 1.04z";

/// Render a single job card as HTML string (must match jobs.html card markup)
fn render_job_card(job: &JobListView) -> String {
    let mut html = String::new();

    let verified_class = if job.is_poster_verified { " job-card-verified" } else { "" };
    html.push_str(&format!(r#"<article class="job-card{}">"#, verified_class));

    if let Some(ref poster) = job.production_poster {
        html.push_str(&format!(
            r#"<div class="job-card-poster"><img src="{}" alt="" loading="lazy" /></div>"#,
            escape_html(poster)
        ));
    }

    html.push_str(r#"<div class="job-card-body"><div class="job-card-header">"#);
    html.push_str(&format!(
        r#"<h3><a href="/jobs/{}">{}</a></h3>"#,
        escape_html(&job.id),
        escape_html(&job.title)
    ));

    html.push_str(r#"<div class="job-card-meta"><span class="job-poster">"#);
    let poster_url = if job.poster_type == "organization" {
        format!("/orgs/{}", escape_html(&job.poster_slug))
    } else {
        format!("/{}", escape_html(&job.poster_slug))
    };
    html.push_str(&format!(
        r#"<a href="{}">{}</a>"#,
        poster_url,
        escape_html(&job.poster_name)
    ));

    if job.is_poster_verified {
        let (fill, label) = if job.poster_type == "organization" {
            ("#FFD700", "Verified Organization")
        } else {
            ("#1d9bf0", "Verified")
        };
        html.push_str(&format!(
            r#"<svg class="verified-badge" width="16" height="16" viewBox="0 0 24 24" fill="{}" aria-label="{}"><path d="{}"/></svg>"#,
            fill, label, VERIFIED_BADGE_PATH
        ));
    }
    html.push_str("</span>");

    if let Some(ref loc) = job.location {
        html.push_str(&format!(r#"<span class="job-location">{}</span>"#, escape_html(loc)));
    }
    html.push_str("</div></div>");

    html.push_str(&format!(r#"<p class="job-card-desc">{}</p>"#, escape_html(&job.description)));

    html.push_str(r#"<div class="job-card-footer">"#);
    let role_s = if job.role_count != 1 { "s" } else { "" };
    html.push_str(&format!(r#"<span class="job-roles">{} role{}</span>"#, job.role_count, role_s));
    if let Some(ref prod) = job.production_title {
        html.push_str(&format!(r#"<span class="job-production">{}</span>"#, escape_html(prod)));
    }
    html.push_str(&format!(r#"<a href="/jobs/{}" class="jobs-btn-sm">View</a>"#, escape_html(&job.id)));
    html.push_str("</div></div></article>");

    html
}

#[derive(Debug, Deserialize)]
struct JobsMoreQuery {
    offset: usize,
    q: Option<String>,
}

/// SSE endpoint for infinite scroll — appends more job cards
async fn jobs_more_sse(
    Query(params): Query<JobsMoreQuery>,
) -> Response {
    let search = params.q.as_deref().filter(|s| !s.is_empty());
    let offset = params.offset;

    let all_jobs = JobModel::list(search, JOBS_PAGE_SIZE + 1, offset)
        .await
        .unwrap_or_default();

    let has_more = all_jobs.len() > JOBS_PAGE_SIZE;

    let jobs: Vec<JobListView> = all_jobs
        .into_iter()
        .take(JOBS_PAGE_SIZE)
        .map(|j| JobListView {
            id: j.id.strip_prefix("job_posting:").unwrap_or(&j.id).to_string(),
            title: j.title,
            description: j.description,
            location: j.location,
            poster_name: j.poster_name,
            poster_slug: j.poster_slug,
            poster_type: j.poster_type,
            is_poster_verified: j.is_poster_verified,
            role_count: j.role_count,
            status: j.status,
            expires_at: j.expires_at,
            created_at: j.created_at,
            production_title: j.production_title,
            production_poster: j.production_poster,
            applications_enabled: j.applications_enabled,
        })
        .collect();

    if jobs.is_empty() {
        return sse_response(sse_patch_elements("#jobs-sentinel", "remove", ""));
    }

    let mut replacement_html = String::new();
    for job in &jobs {
        replacement_html.push_str(&render_job_card(job));
    }

    if has_more {
        let new_offset = offset + JOBS_PAGE_SIZE;
        let q_param = match search {
            Some(q) => format!("&q={}", urlencoding::encode(q)),
            None => String::new(),
        };
        replacement_html.push_str(&format!(
            r#"<div id="jobs-sentinel" data-on-intersect="@get('/api/jobs/more-sse?offset={}{}')"><div class="jobs-loading">Loading more...</div></div>"#,
            new_offset, q_param
        ));
    }

    // Replace sentinel with cards + new sentinel, keeping sentinel at the bottom
    let sse = sse_patch_elements("#jobs-sentinel", "outer", &replacement_html);

    sse_response(sse)
}
