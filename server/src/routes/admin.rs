use askama::Template;
use axum::{
    Router,
    extract::{Path, Query},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use surrealdb::types::SurrealValue;
use tracing::{error, info, warn};

use crate::{
    db::DB,
    error::Error,
    middleware::AuthenticatedUser,
    models::person::SessionUser,
    record_id_ext::RecordIdExt,
    services::s3::s3,
    templates::{BaseContext, User},
};

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

/// Flag to prevent concurrent embedding rebuilds
static REBUILD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

// ============================
// Admin guard helper
// ============================

async fn require_admin(user: &SessionUser) -> Result<User, Error> {
    let template_user = User::from_session_user(user).await;
    if !template_user.is_admin {
        return Err(Error::Forbidden);
    }
    Ok(template_user)
}

// ============================
// Templates
// ============================

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
struct AdminDashboardTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    stats: AdminStats,
    embedding_rebuild_in_progress: bool,
    build_info: String,
}

struct AdminStats {
    person_count: usize,
    production_count: usize,
    location_count: usize,
    organization_count: usize,
    feedback_count: usize,
    engagement: crate::models::activity::EngagementMetrics,
    top_pages: Vec<crate::models::activity::PageStat>,
    daily_activity: Vec<crate::models::activity::DayStat>,
    event_counts: Vec<(String, u64)>,
}

#[derive(Template)]
#[template(path = "admin/feedback.html")]
struct AdminFeedbackTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    feedback_items: Vec<FeedbackItem>,
}

struct FeedbackItem {
    id: String,
    username: String,
    page_url: String,
    message: String,
    created_at: String,
}

#[derive(Template)]
#[template(path = "admin/people.html")]
struct AdminPeopleTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    people: Vec<PersonRow>,
    search_query: String,
}

struct PersonRow {
    id: String,
    username: String,
    email: String,
    name: Option<String>,
    is_admin: bool,
    verification_status: String,
    created_at: String,
}

#[derive(Template)]
#[template(path = "admin/productions.html")]
struct AdminProductionsTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    productions: Vec<ProductionRow>,
    search_query: String,
}

struct ProductionRow {
    id: String,
    title: String,
    slug: String,
    production_type: String,
    status: String,
    created_at: String,
}

#[derive(Template)]
#[template(path = "admin/organizations.html")]
struct AdminOrganizationsTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    organizations: Vec<OrgRow>,
    search_query: String,
}

struct OrgRow {
    id: String,
    name: String,
    slug: String,
    org_type: String,
    is_public: bool,
    is_verified: bool,
    created_at: String,
}

#[derive(Template)]
#[template(path = "admin/locations.html")]
struct AdminLocationsTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    locations: Vec<LocationRow>,
    search_query: String,
}

struct LocationRow {
    id: String,
    name: String,
    city: String,
    state: String,
    is_public: bool,
    created_at: String,
}

// ============================
// Router
// ============================

pub fn router() -> Router {
    Router::new()
        .route("/admin", get(dashboard))
        .route("/admin/feedback", get(list_feedback))
        .route("/admin/feedback/{id}/delete", post(delete_feedback))
        .route("/admin/people", get(list_people))
        .route("/admin/people/{id}/delete", post(delete_person))
        .route("/admin/people/{id}/toggle-admin", post(toggle_admin))
        .route("/admin/people/{id}/reset-password", post(admin_reset_password))
        .route("/admin/people/{id}/verification", post(update_verification))
        .route("/admin/productions", get(list_productions))
        .route("/admin/productions/{id}/delete", post(delete_production))
        .route("/admin/organizations", get(list_organizations))
        .route("/admin/organizations/{id}/delete", post(delete_organization))
        .route("/admin/organizations/{id}/toggle-verified", post(toggle_org_verified))
        .route("/admin/locations", get(list_locations))
        .route("/admin/locations/{id}/delete", post(delete_location))
        .route("/admin/rebuild-embeddings", post(rebuild_embeddings))
        .route("/admin/backup", post(backup_all))
        .route("/admin/cleanup-files", get(preview_orphaned_files))
        .route("/admin/cleanup-files", post(cleanup_orphaned_files))
}

// ============================
// Handlers
// ============================

async fn dashboard(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    let template_user = require_admin(&user).await?;

    use crate::models::activity::ActivityModel;

    // Run all queries in parallel
    let (person_count, production_count, location_count, organization_count, feedback_count,
         engagement, top_pages, daily_activity, event_counts) = tokio::join!(
        count_table("person"),
        count_table("production"),
        count_table("location"),
        count_table("organization"),
        count_table("feedback"),
        ActivityModel::engagement_metrics(),
        ActivityModel::top_pages(10),
        ActivityModel::daily_activity(30),
        ActivityModel::event_counts(),
    );

    let stats = AdminStats {
        person_count,
        production_count,
        location_count,
        organization_count,
        feedback_count,
        engagement,
        top_pages,
        daily_activity,
        event_counts,
    };

    let base = BaseContext::new()
        .with_page("admin")
        .with_user(template_user);

    let template = AdminDashboardTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        stats,
        embedding_rebuild_in_progress: REBUILD_IN_PROGRESS.load(Ordering::Relaxed),
        build_info: format!("v{}", crate::version::VERSION),
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render admin dashboard: {}", e);
        Error::template(e.to_string())
    })?))
}

// -- Feedback --

async fn list_feedback(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    let template_user = require_admin(&user).await?;

    #[derive(Debug, Deserialize, surrealdb::types::SurrealValue)]
    struct FeedbackRow {
        id: surrealdb::types::RecordId,
        username: String,
        page_url: String,
        message: String,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let items: Vec<FeedbackRow> = DB
        .query("SELECT * FROM feedback ORDER BY created_at DESC LIMIT 100")
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .take(0)
        .unwrap_or_default();

    let feedback_items: Vec<FeedbackItem> = items
        .into_iter()
        .map(|f| FeedbackItem {
            id: f.id.key_string(),
            username: f.username,
            page_url: f.page_url,
            message: f.message,
            created_at: f.created_at.format("%b %d, %Y %H:%M").to_string(),
        })
        .collect();

    let base = BaseContext::new()
        .with_page("admin")
        .with_user(template_user);

    let template = AdminFeedbackTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        feedback_items,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render admin feedback: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn delete_feedback(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    let record_id = surrealdb::types::RecordId::new("feedback", id.as_str());

    info!("Deleting feedback record: {} (RecordId: {:?})", id, record_id);

    let response = DB.query("DELETE $id")
        .bind(("id", record_id))
        .await
        .map_err(|e| {
            error!("Feedback delete query failed: {}", e);
            Error::Database(e.to_string())
        })?;

    if let Err(e) = response.check() {
        error!("Feedback delete statement error: {}", e);
        return Err(Error::Database(e.to_string()));
    }

    info!("Admin {} deleted feedback {}", user.username, id);
    Ok(Redirect::to("/admin/feedback"))
}

// -- People --

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
}

async fn list_people(
    AuthenticatedUser(user): AuthenticatedUser,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, Error> {
    let template_user = require_admin(&user).await?;

    let search = params.q.clone().unwrap_or_default();

    #[derive(Debug, Deserialize, surrealdb::types::SurrealValue)]
    struct PRow {
        id: surrealdb::types::RecordId,
        username: String,
        email: String,
        name: Option<String>,
        is_admin: Option<bool>,
        verification_status: String,
        created_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let people: Vec<PRow> = if search.is_empty() {
        DB.query("SELECT id, username, email, name, is_admin, verification_status, created_at FROM person ORDER BY created_at DESC LIMIT 50")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    } else {
        let q = search.to_lowercase();
        DB.query("SELECT id, username, email, name, is_admin, verification_status, created_at FROM person WHERE string::lowercase(username) CONTAINS $q OR string::lowercase(email) CONTAINS $q OR string::lowercase(name ?? '') CONTAINS $q ORDER BY created_at DESC LIMIT 50")
            .bind(("q", q))
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    };

    let people: Vec<PersonRow> = people
        .into_iter()
        .map(|p| PersonRow {
            id: p.id.key_string(),
            username: p.username,
            email: p.email,
            name: p.name,
            is_admin: p.is_admin.unwrap_or(false),
            verification_status: p.verification_status,
            created_at: p.created_at
                .map(|d| d.format("%b %d, %Y").to_string())
                .unwrap_or_default(),
        })
        .collect();

    let base = BaseContext::new()
        .with_page("admin")
        .with_user(template_user);

    let template = AdminPeopleTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        people,
        search_query: search,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render admin people: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn delete_person(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    let record_id = surrealdb::types::RecordId::new("person", id.as_str());

    // Don't allow deleting yourself
    let self_key = if user.id.starts_with("person:") {
        user.id.strip_prefix("person:").unwrap_or(&user.id).to_string()
    } else {
        user.id.clone()
    };
    if id == self_key {
        return Err(Error::BadRequest("Cannot delete your own account from admin".to_string()));
    }

    // Clean up related data then delete
    DB.query("DELETE FROM involvement WHERE in = $pid; DELETE FROM notification WHERE person_id = $pid; DELETE FROM member_of WHERE in = $pid; DELETE $pid")
        .bind(("pid", record_id))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Admin {} deleted person {}", user.username, id);
    Ok(Redirect::to("/admin/people"))
}

async fn toggle_admin(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    let record_id = surrealdb::types::RecordId::new("person", id.as_str());

    // Don't allow toggling your own admin status
    let self_key = if user.id.starts_with("person:") {
        user.id.strip_prefix("person:").unwrap_or(&user.id).to_string()
    } else {
        user.id.clone()
    };
    if id == self_key {
        return Err(Error::BadRequest("Cannot change your own admin status".to_string()));
    }

    DB.query("UPDATE $pid SET is_admin = !is_admin")
        .bind(("pid", record_id))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Admin {} toggled admin for {}", user.username, id);
    Ok(Redirect::to("/admin/people"))
}

#[derive(Deserialize)]
struct AdminResetPasswordForm {
    new_password: String,
}

async fn admin_reset_password(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
    axum::Form(form): axum::Form<AdminResetPasswordForm>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    if form.new_password.len() < 8 {
        return Err(Error::BadRequest("Password must be at least 8 characters".to_string()));
    }

    let record_id = surrealdb::types::RecordId::new("person", id.as_str());
    let password_hash = crate::auth::hash_password(&form.new_password)?;

    DB.query("UPDATE $pid SET password = $password")
        .bind(("pid", record_id))
        .bind(("password", password_hash))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Admin {} reset password for person:{}", user.username, id);
    Ok(Redirect::to("/admin/people"))
}

#[derive(Deserialize)]
struct VerificationForm {
    status: String,
}

async fn update_verification(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
    axum::extract::Form(form): axum::extract::Form<VerificationForm>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    let valid_statuses = ["unverified", "email", "sms", "identity"];
    if !valid_statuses.contains(&form.status.as_str()) {
        return Err(Error::BadRequest(format!("Invalid verification status: {}", form.status)));
    }

    let record_id = surrealdb::types::RecordId::new("person", id.as_str());

    info!("Updating verification_status for person:{} to '{}' (RecordId: {:?})", id, form.status, record_id);

    let response = DB.query("UPDATE $pid SET verification_status = $status")
        .bind(("pid", record_id))
        .bind(("status", form.status.clone()))
        .await
        .map_err(|e| {
            error!("Verification update query failed: {}", e);
            Error::Database(e.to_string())
        })?;

    // Check for statement-level errors in the response
    match response.check() {
        Ok(_) => info!("Admin {} set verification_status to '{}' for person:{}", user.username, form.status, id),
        Err(e) => {
            error!("Verification update statement error: {}", e);
            return Err(Error::Database(e.to_string()));
        }
    }

    Ok(Redirect::to("/admin/people"))
}

// -- Productions --

async fn list_productions(
    AuthenticatedUser(user): AuthenticatedUser,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, Error> {
    let template_user = require_admin(&user).await?;

    let search = params.q.clone().unwrap_or_default();

    #[derive(Debug, Deserialize, surrealdb::types::SurrealValue)]
    struct ProdRow {
        id: surrealdb::types::RecordId,
        title: String,
        slug: String,
        production_type: String,
        status: String,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let productions: Vec<ProdRow> = if search.is_empty() {
        DB.query("SELECT id, title, slug, type AS production_type, status, created_at FROM production ORDER BY created_at DESC LIMIT 50")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    } else {
        let q = search.to_lowercase();
        DB.query("SELECT id, title, slug, type AS production_type, status, created_at FROM production WHERE string::lowercase(title) CONTAINS $q OR string::lowercase(slug) CONTAINS $q ORDER BY created_at DESC LIMIT 50")
            .bind(("q", q))
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    };

    let productions: Vec<ProductionRow> = productions
        .into_iter()
        .map(|p| ProductionRow {
            id: p.id.key_string(),
            title: p.title,
            slug: p.slug,
            production_type: p.production_type,
            status: p.status,
            created_at: p.created_at.format("%b %d, %Y").to_string(),
        })
        .collect();

    let base = BaseContext::new()
        .with_page("admin")
        .with_user(template_user);

    let template = AdminProductionsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        productions,
        search_query: search,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render admin productions: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn delete_production(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    let record_id = surrealdb::types::RecordId::new("production", id.as_str());

    // Clean up involvements then delete
    DB.query("DELETE FROM involvement WHERE out = $pid; DELETE FROM member_of WHERE out = $pid; DELETE $pid")
        .bind(("pid", record_id))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Admin {} deleted production {}", user.username, id);
    Ok(Redirect::to("/admin/productions"))
}

// -- Organizations --

async fn list_organizations(
    AuthenticatedUser(user): AuthenticatedUser,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, Error> {
    let template_user = require_admin(&user).await?;

    let search = params.q.clone().unwrap_or_default();

    #[derive(Debug, Deserialize, surrealdb::types::SurrealValue)]
    struct DbOrgRow {
        id: surrealdb::types::RecordId,
        name: String,
        slug: String,
        org_type: Option<String>,
        public: Option<bool>,
        #[serde(default)]
        #[surreal(default)]
        verified: bool,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let orgs: Vec<DbOrgRow> = if search.is_empty() {
        DB.query("SELECT id, name, slug, type.name AS org_type, public, verified, created_at FROM organization ORDER BY created_at DESC LIMIT 50")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    } else {
        let q = search.to_lowercase();
        DB.query("SELECT id, name, slug, type.name AS org_type, public, verified, created_at FROM organization WHERE string::lowercase(name) CONTAINS $q OR string::lowercase(slug) CONTAINS $q ORDER BY created_at DESC LIMIT 50")
            .bind(("q", q))
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    };

    let organizations: Vec<OrgRow> = orgs
        .into_iter()
        .map(|o| OrgRow {
            id: o.id.key_string(),
            name: o.name,
            slug: o.slug,
            org_type: o.org_type.unwrap_or_default(),
            is_public: o.public.unwrap_or(false),
            is_verified: o.verified,
            created_at: o.created_at.format("%b %d, %Y").to_string(),
        })
        .collect();

    let base = BaseContext::new()
        .with_page("admin")
        .with_user(template_user);

    let template = AdminOrganizationsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        organizations,
        search_query: search,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render admin organizations: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn delete_organization(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    let record_id = surrealdb::types::RecordId::new("organization", id.as_str());

    // Clean up memberships then delete
    DB.query("DELETE FROM member_of WHERE out = $oid; DELETE $oid")
        .bind(("oid", record_id))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Admin {} deleted organization {}", user.username, id);
    Ok(Redirect::to("/admin/organizations"))
}

async fn toggle_org_verified(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    let record_id = surrealdb::types::RecordId::new("organization", id.as_str());

    DB.query("UPDATE $oid SET verified = !verified")
        .bind(("oid", record_id))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Admin {} toggled verification for organization {}", user.username, id);
    Ok(Redirect::to("/admin/organizations"))
}

// -- Locations --

async fn list_locations(
    AuthenticatedUser(user): AuthenticatedUser,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, Error> {
    let template_user = require_admin(&user).await?;

    let search = params.q.clone().unwrap_or_default();

    #[derive(Debug, Deserialize, surrealdb::types::SurrealValue)]
    struct LocRow {
        id: surrealdb::types::RecordId,
        name: String,
        city: String,
        state: String,
        is_public: bool,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let locations: Vec<LocRow> = if search.is_empty() {
        DB.query("SELECT id, name, city, state, is_public, created_at FROM location ORDER BY created_at DESC LIMIT 50")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    } else {
        let q = search.to_lowercase();
        DB.query("SELECT id, name, city, state, is_public, created_at FROM location WHERE string::lowercase(name) CONTAINS $q OR string::lowercase(city) CONTAINS $q ORDER BY created_at DESC LIMIT 50")
            .bind(("q", q))
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    };

    let locations: Vec<LocationRow> = locations
        .into_iter()
        .map(|l| LocationRow {
            id: l.id.key_string(),
            name: l.name,
            city: l.city,
            state: l.state,
            is_public: l.is_public,
            created_at: l.created_at.format("%b %d, %Y").to_string(),
        })
        .collect();

    let base = BaseContext::new()
        .with_page("admin")
        .with_user(template_user);

    let template = AdminLocationsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        locations,
        search_query: search,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render admin locations: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn delete_location(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    let record_id = surrealdb::types::RecordId::new("location", id.as_str());

    DB.query("DELETE $id")
        .bind(("id", record_id))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Admin {} deleted location {}", user.username, id);
    Ok(Redirect::to("/admin/locations"))
}

// -- Embedding rebuild --

async fn rebuild_embeddings(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    if REBUILD_IN_PROGRESS.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err(Error::BadRequest("Embedding rebuild is already in progress".to_string()));
    }

    info!("Admin {} triggered embedding rebuild", user.username);

    tokio::spawn(async move {
        if let Err(e) = run_embedding_rebuild().await {
            error!("Embedding rebuild failed: {}", e);
        }
        REBUILD_IN_PROGRESS.store(false, Ordering::SeqCst);
    });

    Ok(Redirect::to("/admin"))
}

async fn run_embedding_rebuild() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::services::embedding::{
        build_location_embedding_text, build_organization_embedding_text,
        build_person_embedding_text, build_production_embedding_text,
        generate_embedding_async,
    };

    info!("Starting full embedding rebuild");
    let mut total_updated: u32 = 0;
    let mut total_failed: u32 = 0;

    // ── People ──
    {
        #[derive(Debug, serde::Deserialize, SurrealValue)]
        struct PersonRow {
            id: surrealdb::types::RecordId,
            name: Option<String>,
            username: Option<String>,
            profile: Option<PersonProfileRow>,
        }
        #[derive(Debug, Clone, serde::Deserialize, SurrealValue)]
        struct PersonProfileRow {
            headline: Option<String>,
            bio: Option<String>,
            location: Option<String>,
            skills: Option<Vec<String>>,
            gender: Option<String>,
            ethnicity: Option<Vec<String>>,
            age_range: Option<AgeRangeRow>,
            height_mm: Option<i32>,
            body_type: Option<String>,
            hair_color: Option<String>,
            eye_color: Option<String>,
            languages: Option<Vec<String>>,
            unions: Option<Vec<String>>,
            acting_age_range: Option<AgeRangeRow>,
            acting_ethnicities: Option<Vec<String>>,
            nationality: Option<String>,
        }
        #[derive(Debug, Clone, serde::Deserialize, SurrealValue)]
        struct AgeRangeRow { min: i32, max: i32 }

        let mut resp = DB.query("SELECT id, name, username, profile FROM person").await?;
        let people: Vec<PersonRow> = match resp.take(0) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to deserialize person records for embedding rebuild: {}. Falling back to name-only query.", e);
                // Fallback: fetch just id/name/username without the profile struct
                #[derive(Debug, serde::Deserialize, SurrealValue)]
                struct PersonBasic {
                    id: surrealdb::types::RecordId,
                    name: Option<String>,
                    username: Option<String>,
                }
                let mut resp2 = DB.query("SELECT id, name, username FROM person").await?;
                let basics: Vec<PersonBasic> = resp2.take(0).unwrap_or_default();
                basics.into_iter().map(|b| PersonRow {
                    id: b.id, name: b.name, username: b.username, profile: None,
                }).collect()
            }
        };
        info!("Rebuilding embeddings for {} people", people.len());

        for person in people {
            let display_name = person.name.as_deref()
                .unwrap_or(person.username.as_deref().unwrap_or("unknown"));
            let embedding_text = if let Some(ref profile) = person.profile {
                build_person_embedding_text(
                    display_name,
                    profile.headline.as_deref(),
                    profile.bio.as_deref(),
                    &profile.skills.clone().unwrap_or_default(),
                    profile.location.as_deref(),
                    profile.age_range.as_ref().map(|ar| (ar.min, ar.max)),
                    profile.gender.as_deref(),
                    &profile.ethnicity.clone().unwrap_or_default(),
                    profile.height_mm,
                    profile.body_type.as_deref(),
                    profile.hair_color.as_deref(),
                    profile.eye_color.as_deref(),
                    &profile.languages.clone().unwrap_or_default(),
                    &profile.unions.clone().unwrap_or_default(),
                    &[],
                    profile.acting_age_range.as_ref().map(|ar| (ar.min, ar.max)),
                    &profile.acting_ethnicities.clone().unwrap_or_default(),
                    profile.nationality.as_deref(),
                )
            } else {
                build_person_embedding_text(
                    display_name, None, None, &[], None, None, None, &[], None, None, None, None, &[], &[], &[], None, &[], None,
                )
            };

            match generate_embedding_async(&embedding_text).await {
                Ok(emb) => {
                    if let Err(e) = DB.query("UPDATE $id SET embedding = $embedding, embedding_text = $embedding_text")
                        .bind(("id", person.id.clone()))
                        .bind(("embedding", emb))
                        .bind(("embedding_text", embedding_text))
                        .await
                    {
                        warn!("Failed to update embedding for person {:?}: {}", person.id, e);
                        total_failed += 1;
                    } else {
                        total_updated += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to generate embedding for person {:?}: {}", person.id, e);
                    total_failed += 1;
                }
            }
        }
    }

    // ── Organizations ──
    {
        #[derive(Debug, serde::Deserialize, SurrealValue)]
        struct OrgRow {
            id: surrealdb::types::RecordId,
            name: Option<String>,
            org_type: Option<String>,
            description: Option<String>,
            services: Option<Vec<String>>,
            location: Option<String>,
            founded_year: Option<i32>,
            employees_count: Option<i32>,
        }

        let mut resp = DB.query("SELECT id, name, type.name AS org_type, description, services, location, founded_year, employees_count FROM organization").await?;
        let orgs: Vec<OrgRow> = resp.take(0).unwrap_or_default();
        info!("Rebuilding embeddings for {} organizations", orgs.len());

        for org in orgs {
            let name = org.name.as_deref().unwrap_or("unknown");
            let embedding_text = build_organization_embedding_text(
                name,
                org.org_type.as_deref().unwrap_or(""),
                org.description.as_deref(),
                &org.services.unwrap_or_default(),
                org.location.as_deref(),
                org.founded_year,
                org.employees_count,
            );

            match generate_embedding_async(&embedding_text).await {
                Ok(emb) => {
                    if let Err(e) = DB.query("UPDATE $id SET embedding = $embedding, embedding_text = $embedding_text")
                        .bind(("id", org.id.clone()))
                        .bind(("embedding", emb))
                        .bind(("embedding_text", embedding_text))
                        .await
                    {
                        warn!("Failed to update embedding for org {:?}: {}", org.id, e);
                        total_failed += 1;
                    } else {
                        total_updated += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to generate embedding for org {:?}: {}", org.id, e);
                    total_failed += 1;
                }
            }
        }
    }

    // ── Locations ──
    {
        #[derive(Debug, serde::Deserialize, SurrealValue)]
        struct LocRow {
            id: surrealdb::types::RecordId,
            name: Option<String>,
            description: Option<String>,
            city: Option<String>,
            state: Option<String>,
            country: Option<String>,
            amenities: Option<Vec<String>>,
            restrictions: Option<Vec<String>>,
            max_capacity: Option<i32>,
            parking_info: Option<String>,
        }

        let mut resp = DB.query("SELECT id, name, description, city, state, country, amenities, restrictions, max_capacity, parking_info FROM location").await?;
        let locations: Vec<LocRow> = resp.take(0).unwrap_or_default();
        info!("Rebuilding embeddings for {} locations", locations.len());

        for loc in locations {
            let name = loc.name.as_deref().unwrap_or("unknown");
            let embedding_text = build_location_embedding_text(
                name,
                loc.description.as_deref(),
                loc.city.as_deref().unwrap_or(""),
                loc.state.as_deref().unwrap_or(""),
                loc.country.as_deref().unwrap_or(""),
                &loc.amenities.unwrap_or_default(),
                &loc.restrictions.unwrap_or_default(),
                loc.max_capacity,
                loc.parking_info.as_deref(),
            );

            match generate_embedding_async(&embedding_text).await {
                Ok(emb) => {
                    if let Err(e) = DB.query("UPDATE $id SET embedding = $embedding, embedding_text = $embedding_text")
                        .bind(("id", loc.id.clone()))
                        .bind(("embedding", emb))
                        .bind(("embedding_text", embedding_text))
                        .await
                    {
                        warn!("Failed to update embedding for location {:?}: {}", loc.id, e);
                        total_failed += 1;
                    } else {
                        total_updated += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to generate embedding for location {:?}: {}", loc.id, e);
                    total_failed += 1;
                }
            }
        }
    }

    // ── Productions ──
    {
        #[derive(Debug, serde::Deserialize, SurrealValue)]
        struct ProdRow {
            id: surrealdb::types::RecordId,
            title: Option<String>,
            production_type: Option<String>,
            status: Option<String>,
            description: Option<String>,
            location: Option<String>,
            start_date: Option<String>,
            end_date: Option<String>,
        }

        let mut resp = DB.query("SELECT id, title, type AS production_type, status, description, location, <string> start_date AS start_date, <string> end_date AS end_date FROM production").await?;
        let productions: Vec<ProdRow> = resp.take(0).unwrap_or_default();
        info!("Rebuilding embeddings for {} productions", productions.len());

        for prod in productions {
            let title = prod.title.as_deref().unwrap_or("unknown");
            let embedding_text = build_production_embedding_text(
                title,
                prod.production_type.as_deref().unwrap_or(""),
                prod.status.as_deref().unwrap_or(""),
                prod.description.as_deref(),
                prod.location.as_deref(),
                prod.start_date.as_deref(),
                prod.end_date.as_deref(),
            );

            match generate_embedding_async(&embedding_text).await {
                Ok(emb) => {
                    if let Err(e) = DB.query("UPDATE $id SET embedding = $embedding, embedding_text = $embedding_text")
                        .bind(("id", prod.id.clone()))
                        .bind(("embedding", emb))
                        .bind(("embedding_text", embedding_text))
                        .await
                    {
                        warn!("Failed to update embedding for production {:?}: {}", prod.id, e);
                        total_failed += 1;
                    } else {
                        total_updated += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to generate embedding for production {:?}: {}", prod.id, e);
                    total_failed += 1;
                }
            }
        }
    }

    info!("Embedding rebuild complete: {} updated, {} failed", total_updated, total_failed);
    Ok(())
}

// -- Backup --

async fn backup_all(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, Error> {
    require_admin(&user).await?;

    info!("Admin {} initiated full backup", user.username);

    // 1. Export database via SurrealDB HTTP endpoint
    //    (WS client doesn't support export, so we hit the HTTP API directly)
    let db_config = crate::config::Config::from_env()
        .map_err(|e| Error::Internal(format!("Config error: {}", e)))?;
    let db = &db_config.database;
    let export_url = format!("http://{}:{}/export", db.host, db.port);

    let http_client = reqwest::Client::new();
    let db_export = http_client
        .get(&export_url)
        .header("Accept", "application/octet-stream")
        .header("surreal-ns", &db.namespace)
        .header("surreal-db", &db.name)
        .basic_auth(&db.username, Some(&db.password))
        .send()
        .await
        .map_err(|e| Error::Internal(format!("DB export request failed: {}", e)))?
        .bytes()
        .await
        .map_err(|e| Error::Internal(format!("DB export read failed: {}", e)))?;

    info!("DB export complete: {} bytes", db_export.len());

    // 2. List all S3 objects
    let s3_service = s3()?;
    let all_keys = s3_service.list_all_objects().await?;
    info!("Found {} files in S3 to back up", all_keys.len());

    // 3. Build zip archive in memory
    let mut zip_buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Add DB export
        zip.start_file("db_export.surql", options)
            .map_err(|e| Error::Internal(format!("Zip error: {}", e)))?;
        std::io::Write::write_all(&mut zip, &db_export)
            .map_err(|e| Error::Internal(format!("Zip write error: {}", e)))?;

        // Add each S3 file
        for key in &all_keys {
            match s3_service.download_file(key).await {
                Ok((data, _content_type)) => {
                    let path = format!("files/{}", key);
                    zip.start_file(&path, options)
                        .map_err(|e| Error::Internal(format!("Zip error: {}", e)))?;
                    std::io::Write::write_all(&mut zip, &data)
                        .map_err(|e| Error::Internal(format!("Zip write error: {}", e)))?;
                }
                Err(e) => {
                    warn!("Skipping file {} during backup: {}", key, e);
                }
            }
        }

        zip.finish()
            .map_err(|e| Error::Internal(format!("Zip finish error: {}", e)))?;
    }

    info!("Backup zip created: {} bytes", zip_buffer.len());

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("slatehub_backup_{}.zip", timestamp);

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "application/zip".parse().unwrap(),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename).parse().unwrap(),
    );

    Ok((headers, zip_buffer))
}

// -- Cleanup orphaned files --

/// A file reference from the database: S3 key + source info for broken link reporting.
#[derive(Debug, Clone)]
struct FileRef {
    key: String,
    entity: String,  // e.g. "person:abc123"
    field: String,    // e.g. "avatar", "photos[2].url"
}

/// Collect all referenced S3 keys from every table that stores file URLs.
/// Returns (set of all keys for orphan detection, vec of all refs for broken link detection).
async fn collect_all_referenced_files() -> Result<(std::collections::HashSet<String>, Vec<FileRef>), Error> {
    let mut keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut refs: Vec<FileRef> = Vec::new();

    #[derive(Debug, Deserialize, SurrealValue)]
    struct PhotoRef {
        url: Option<String>,
        thumbnail_url: Option<String>,
    }

    /// Helper: extract key, insert into set, and record the reference.
    fn track(keys: &mut std::collections::HashSet<String>, refs: &mut Vec<FileRef>, url: &str, entity: &str, field: &str) {
        let key = url.strip_prefix("/api/media/").unwrap_or(url).to_string();
        keys.insert(key.clone());
        // Also derive thumbnail key
        if let Some(slash_pos) = key.rfind('/') {
            let dir = &key[..slash_pos];
            let filename = &key[slash_pos + 1..];
            if !filename.starts_with("thumb_") {
                keys.insert(format!("{}/thumb_{}", dir, filename));
            }
        }
        refs.push(FileRef { key, entity: entity.to_string(), field: field.to_string() });
    }

    // Person: avatar + photo gallery
    {
        #[derive(Debug, Deserialize, SurrealValue)]
        struct PersonFiles {
            id: String,
            name: Option<String>,
            avatar: Option<String>,
            photos: Option<Vec<PhotoRef>>,
        }

        let rows: Vec<PersonFiles> = DB
            .query("SELECT <string> id AS id, name, profile.avatar AS avatar, profile.photos AS photos FROM person")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default();

        for row in rows {
            let entity = format!("person {} ({})", row.name.as_deref().unwrap_or("?"), row.id);
            if let Some(avatar) = row.avatar {
                track(&mut keys, &mut refs, &avatar, &entity, "avatar");
            }
            if let Some(photos) = row.photos {
                for (i, photo) in photos.iter().enumerate() {
                    if let Some(ref url) = photo.url {
                        track(&mut keys, &mut refs, url, &entity, &format!("photos[{}]", i));
                    }
                    if let Some(ref thumb) = photo.thumbnail_url {
                        track(&mut keys, &mut refs, thumb, &entity, &format!("photos[{}].thumb", i));
                    }
                }
            }
        }
    }

    // Organization: logo
    {
        #[derive(Debug, Deserialize, SurrealValue)]
        struct OrgFiles {
            id: String,
            name: Option<String>,
            logo: Option<String>,
        }

        let rows: Vec<OrgFiles> = DB
            .query("SELECT <string> id AS id, name, logo FROM organization")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default();

        for row in rows {
            let entity = format!("org {} ({})", row.name.as_deref().unwrap_or("?"), row.id);
            if let Some(logo) = row.logo {
                track(&mut keys, &mut refs, &logo, &entity, "logo");
            }
        }
    }

    // Production: header_photo, poster_photo, poster_url, photo gallery
    {
        #[derive(Debug, Deserialize, SurrealValue)]
        struct ProdFiles {
            id: String,
            title: Option<String>,
            header_photo: Option<String>,
            poster_photo: Option<String>,
            poster_url: Option<String>,
            photos: Option<Vec<PhotoRef>>,
        }

        let rows: Vec<ProdFiles> = DB
            .query("SELECT <string> id AS id, title, header_photo, poster_photo, poster_url, photos FROM production")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default();

        for row in rows {
            let entity = format!("production {} ({})", row.title.as_deref().unwrap_or("?"), row.id);
            if let Some(v) = row.header_photo { track(&mut keys, &mut refs, &v, &entity, "header_photo"); }
            if let Some(v) = row.poster_photo { track(&mut keys, &mut refs, &v, &entity, "poster_photo"); }
            if let Some(v) = row.poster_url {
                if v.starts_with("/api/media/") { track(&mut keys, &mut refs, &v, &entity, "poster_url"); }
            }
            if let Some(photos) = row.photos {
                for (i, photo) in photos.iter().enumerate() {
                    if let Some(ref url) = photo.url { track(&mut keys, &mut refs, url, &entity, &format!("photos[{}]", i)); }
                    if let Some(ref thumb) = photo.thumbnail_url { track(&mut keys, &mut refs, thumb, &entity, &format!("photos[{}].thumb", i)); }
                }
            }
        }
    }

    // Location: profile_photo + photo gallery
    {
        #[derive(Debug, Deserialize, SurrealValue)]
        struct LocFiles {
            id: String,
            name: Option<String>,
            profile_photo: Option<String>,
            photos: Option<Vec<PhotoRef>>,
        }

        let rows: Vec<LocFiles> = DB
            .query("SELECT <string> id AS id, name, profile_photo, photos FROM location")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default();

        for row in rows {
            let entity = format!("location {} ({})", row.name.as_deref().unwrap_or("?"), row.id);
            if let Some(v) = row.profile_photo { track(&mut keys, &mut refs, &v, &entity, "profile_photo"); }
            if let Some(photos) = row.photos {
                for (i, photo) in photos.iter().enumerate() {
                    if let Some(ref url) = photo.url { track(&mut keys, &mut refs, url, &entity, &format!("photos[{}]", i)); }
                    if let Some(ref thumb) = photo.thumbnail_url { track(&mut keys, &mut refs, thumb, &entity, &format!("photos[{}].thumb", i)); }
                }
            }
        }
    }

    // Production scripts: file_url / file_key
    {
        #[derive(Debug, Deserialize, SurrealValue)]
        struct ScriptFiles {
            id: String,
            file_url: Option<String>,
            file_key: Option<String>,
        }

        let rows: Vec<ScriptFiles> = DB
            .query("SELECT <string> id AS id, file_url, file_key FROM production_script")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default();

        for row in rows {
            let entity = format!("script ({})", row.id);
            if let Some(v) = row.file_url { track(&mut keys, &mut refs, &v, &entity, "file_url"); }
            if let Some(v) = row.file_key {
                keys.insert(v.clone());
                refs.push(FileRef { key: v, entity: entity.clone(), field: "file_key".to_string() });
            }
        }
    }

    // Media table: uri (linked from person.profile.media_other, person.profile.resume)
    {
        #[derive(Debug, Deserialize, SurrealValue)]
        struct MediaFiles {
            id: String,
            uri: Option<String>,
        }

        let rows: Vec<MediaFiles> = DB
            .query("SELECT <string> id AS id, uri FROM media")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default();

        for row in rows {
            let entity = format!("media ({})", row.id);
            if let Some(v) = row.uri { track(&mut keys, &mut refs, &v, &entity, "uri"); }
        }
    }

    Ok((keys, refs))
}

/// GET /admin/cleanup-files — preview orphaned files AND broken links
async fn preview_orphaned_files(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    require_admin(&user).await?;

    let s3_service = s3()?;
    let all_keys = s3_service.list_all_objects().await?;
    let all_keys_set: std::collections::HashSet<&str> = all_keys.iter().map(|k| k.as_str()).collect();
    let (referenced_keys, all_refs) = collect_all_referenced_files().await?;

    let orphaned: Vec<&String> = all_keys.iter().filter(|k| !referenced_keys.contains(k.as_str())).collect();

    // Find broken links: DB references pointing to S3 keys that don't exist
    let broken: Vec<&FileRef> = all_refs.iter().filter(|r| !all_keys_set.contains(r.key.as_str())).collect();

    let mut html = String::from(r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>File Integrity Check</title>
<style>
body { font-family: system-ui; background: #111; color: #eee; max-width: 900px; margin: 2rem auto; padding: 0 1rem; }
h1 { font-size: 1.4rem; }
h2 { font-size: 1.1rem; margin-top: 2rem; }
.stats { color: #888; margin-bottom: 1.5rem; }
.file-list { max-height: 400px; overflow-y: auto; background: #1a1a1a; border: 1px solid #333; border-radius: 8px; padding: 1rem; margin-bottom: 1.5rem; }
.file-item { font-family: monospace; font-size: 0.8rem; padding: 0.3rem 0; border-bottom: 1px solid #222; word-break: break-all; }
.file-item:last-child { border-bottom: none; }
.broken-item { font-size: 0.8rem; padding: 0.4rem 0; border-bottom: 1px solid #222; }
.broken-item:last-child { border-bottom: none; }
.broken-key { font-family: monospace; color: #e74c3c; }
.broken-source { color: #888; font-size: 0.75rem; }
.btn { display: inline-block; padding: 0.6rem 1.2rem; border-radius: 6px; border: none; cursor: pointer; font-size: 0.9rem; text-decoration: none; margin-right: 0.5rem; }
.btn-danger { background: #c0392b; color: white; }
.btn-danger:hover { background: #e74c3c; }
.btn-back { background: #333; color: #eee; }
.btn-back:hover { background: #444; }
.ok { color: #5a5; }
.warn { color: #e67e22; }
.section { margin-bottom: 2rem; }
.checkbox-item { display: flex; align-items: center; gap: 0.5rem; cursor: pointer; }
.checkbox-item.has-preview { align-items: flex-start; padding: 0.5rem 0; }
.checkbox-item input[type=checkbox] { width: 16px; height: 16px; cursor: pointer; flex-shrink: 0; margin-top: 2px; }
.preview-img { width: 120px; height: 80px; object-fit: cover; border-radius: 4px; border: 1px solid #333; flex-shrink: 0; background: #222; }
.file-path { font-family: monospace; font-size: 0.8rem; word-break: break-all; }
.select-actions { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.75rem; }
.select-actions button { background: #333; color: #eee; border: 1px solid #555; padding: 0.3rem 0.8rem; border-radius: 4px; cursor: pointer; font-size: 0.8rem; }
.select-actions button:hover { background: #444; }
.count-label { color: #888; font-size: 0.8rem; margin-left: 0.5rem; }
</style></head><body>
<h1>File Integrity Check</h1>"#);

    html.push_str(&format!(
        r#"<div class="stats">S3 objects: {} &nbsp;|&nbsp; DB references: {} &nbsp;|&nbsp; Orphaned files: {} &nbsp;|&nbsp; Broken links: {}</div>"#,
        all_keys.len(), all_refs.len(), orphaned.len(), broken.len()
    ));

    // Broken links section
    html.push_str(r#"<div class="section"><h2>Broken Links (DB references to missing files)</h2>"#);
    if broken.is_empty() {
        html.push_str(r#"<p class="ok">No broken links found. All DB references point to existing files.</p>"#);
    } else {
        html.push_str(&format!(r#"<p class="warn">{} database references point to files that no longer exist in storage:</p>"#, broken.len()));
        html.push_str(r#"<div class="file-list">"#);
        for r in &broken {
            html.push_str(&format!(
                r#"<div class="broken-item"><span class="broken-key">{}</span><br><span class="broken-source">{} &rarr; {}</span></div>"#,
                ammonia::clean_text(&r.key),
                ammonia::clean_text(&r.entity),
                ammonia::clean_text(&r.field),
            ));
        }
        html.push_str("</div>");
    }
    html.push_str("</div>");

    // Orphaned files section
    html.push_str(r#"<div class="section"><h2>Orphaned Files (S3 objects not referenced by any DB record)</h2>"#);
    if orphaned.is_empty() {
        html.push_str(r#"<p class="ok">No orphaned files found. Storage is clean.</p>"#);
    } else {
        html.push_str(r#"<form method="post" action="/admin/cleanup-files">"#);
        html.push_str(r#"<div class="select-actions">
            <button type="button" onclick="document.querySelectorAll('input[name=keys]').forEach(c=>c.checked=true);updateCount()">Select All</button>
            <button type="button" onclick="document.querySelectorAll('input[name=keys]').forEach(c=>c.checked=false);updateCount()">Select None</button>
            <span id="selected-count" class="count-label">0 selected</span>
        </div>"#);
        html.push_str(r#"<div class="file-list">"#);
        for key in &orphaned {
            let escaped = ammonia::clean_text(key);
            let lower = key.to_lowercase();
            let is_image = lower.ends_with(".jpg") || lower.ends_with(".jpeg")
                || lower.ends_with(".png") || lower.ends_with(".webp")
                || lower.ends_with(".gif") || lower.ends_with(".svg");
            if is_image {
                html.push_str(&format!(
                    r#"<label class="file-item checkbox-item has-preview">
                        <input type="checkbox" name="keys" value="{}" onchange="updateCount()">
                        <img src="/api/media/{}" class="preview-img" loading="lazy" onerror="this.style.display='none'">
                        <span class="file-path">{}</span>
                    </label>"#,
                    escaped, escaped, escaped
                ));
            } else {
                html.push_str(&format!(
                    r#"<label class="file-item checkbox-item">
                        <input type="checkbox" name="keys" value="{}" onchange="updateCount()">
                        <span class="file-path">{}</span>
                    </label>"#,
                    escaped, escaped
                ));
            }
        }
        html.push_str("</div>");

        html.push_str(r#"
            <button type="submit" class="btn btn-danger" id="delete-btn" disabled
                onclick="return confirm('Delete the selected files? This cannot be undone.')">
                Delete Selected
            </button>
            <a href="/admin" class="btn btn-back">Cancel</a>
        </form>
        <script>
        function updateCount() {
            var checked = document.querySelectorAll('input[name=keys]:checked').length;
            document.getElementById('selected-count').textContent = checked + ' selected';
            document.getElementById('delete-btn').disabled = checked === 0;
            document.getElementById('delete-btn').textContent = checked > 0 ? 'Delete ' + checked + ' Selected' : 'Delete Selected';
        }
        </script>"#);
    }
    html.push_str("</div>");

    html.push_str(r#"<a href="/admin" class="btn btn-back">Back to Admin</a>"#);

    html.push_str("</body></html>");
    Ok(Html(html))
}

/// POST /admin/cleanup-files — delete selected orphaned files
async fn cleanup_orphaned_files(
    AuthenticatedUser(user): AuthenticatedUser,
    axum::Form(form): axum::Form<Vec<(String, String)>>,
) -> Result<Redirect, Error> {
    require_admin(&user).await?;

    // Collect selected keys from form checkboxes
    let selected_keys: Vec<String> = form.iter()
        .filter(|(name, _)| name == "keys")
        .map(|(_, value)| value.clone())
        .collect();

    if selected_keys.is_empty() {
        return Ok(Redirect::to("/admin/cleanup-files"));
    }

    // Verify the selected keys are actually orphaned (prevent deleting referenced files)
    let s3_service = s3()?;
    let all_keys = s3_service.list_all_objects().await?;
    let all_keys_set: std::collections::HashSet<&str> = all_keys.iter().map(|k| k.as_str()).collect();
    let (referenced, _refs) = collect_all_referenced_files().await?;

    info!("Admin {} deleting {} selected orphaned files", user.username, selected_keys.len());

    let mut deleted_count = 0u32;
    let mut failed_count = 0u32;
    let mut skipped_count = 0u32;

    for key in &selected_keys {
        // Safety: only delete if key exists in S3 AND is not referenced
        if !all_keys_set.contains(key.as_str()) {
            warn!("Skipping key not found in S3: {}", key);
            skipped_count += 1;
            continue;
        }
        if referenced.contains(key.as_str()) {
            warn!("Skipping referenced file (not orphaned): {}", key);
            skipped_count += 1;
            continue;
        }

        match s3_service.delete_file(key).await {
            Ok(_) => {
                info!("Deleted orphaned file: {}", key);
                deleted_count += 1;
            }
            Err(e) => {
                warn!("Failed to delete orphaned file {}: {}", key, e);
                failed_count += 1;
            }
        }
    }

    info!(
        "Cleanup complete: {} deleted, {} failed, {} skipped",
        deleted_count, failed_count, skipped_count
    );

    Ok(Redirect::to("/admin/cleanup-files"))
}


// ============================
// Helpers
// ============================

async fn count_table(table: &str) -> usize {
    use crate::models::system::System;
    System::count_records(table).await.unwrap_or(0)
}
