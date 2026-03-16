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
    response::{Html, Redirect, Response, IntoResponse},
    routing::{get, post},
};
use axum_extra::extract::Form;
use serde::Deserialize;
use tracing::{debug, error, info};

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
    let jobs = JobModel::list(search, 50).await.unwrap_or_default();

    let jobs: Vec<JobListView> = jobs
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

    let template = JobsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        jobs,
        search_query: params.q,
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
