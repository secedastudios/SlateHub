use askama::Template;
use axum::{
    Router,
    extract::{Path, Query},
    response::{Html, Redirect},
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
}

struct AdminStats {
    person_count: usize,
    production_count: usize,
    location_count: usize,
    organization_count: usize,
    feedback_count: usize,
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
        .route("/admin/people/{id}/verification", post(update_verification))
        .route("/admin/productions", get(list_productions))
        .route("/admin/productions/{id}/delete", post(delete_production))
        .route("/admin/organizations", get(list_organizations))
        .route("/admin/organizations/{id}/delete", post(delete_organization))
        .route("/admin/locations", get(list_locations))
        .route("/admin/locations/{id}/delete", post(delete_location))
        .route("/admin/rebuild-embeddings", post(rebuild_embeddings))
}

// ============================
// Handlers
// ============================

async fn dashboard(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    let template_user = require_admin(&user).await?;

    let stats = AdminStats {
        person_count: count_table("person").await,
        production_count: count_table("production").await,
        location_count: count_table("location").await,
        organization_count: count_table("organization").await,
        feedback_count: count_table("feedback").await,
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
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let orgs: Vec<DbOrgRow> = if search.is_empty() {
        DB.query("SELECT id, name, slug, type.name AS org_type, public, created_at FROM organization ORDER BY created_at DESC LIMIT 50")
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .take(0)
            .unwrap_or_default()
    } else {
        let q = search.to_lowercase();
        DB.query("SELECT id, name, slug, type.name AS org_type, public, created_at FROM organization WHERE string::lowercase(name) CONTAINS $q OR string::lowercase(slug) CONTAINS $q ORDER BY created_at DESC LIMIT 50")
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
        let people: Vec<PersonRow> = resp.take(0).unwrap_or_default();
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

// ============================
// Helpers
// ============================

async fn count_table(table: &str) -> usize {
    use crate::models::system::System;
    System::count_records(table).await.unwrap_or(0)
}
