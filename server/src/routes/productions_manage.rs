//! Production management workspace — the `/productions/manage` hub plus
//! `/productions/{slug}/manage/*` per-production tabs.
//!
//! The hub is the persistent entry point (linked from the profile dropdown
//! when the flag allows): it lists every production the user belongs to and
//! drops them into a production's workspace with one click.
//!
//! Two-layer access gate (per project plan §2.4):
//!   1. `feature_flag::allows("production_management", user)` — global flag
//!   2. `ProductionModel::get_role(prod, user)` — must be a `member_of` row
//!      (the hub substitutes "list my memberships" for layer 2)
//!
//! Either gate failing returns **404 (NotFound)**, not 403, so non-admins
//! can't enumerate that management features exist.

use askama::Template;
use axum::{
    Router,
    extract::Path,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use tracing::error;

use crate::{
    error::Error,
    middleware::AuthenticatedUser,
    models::{
        person::SessionUser,
        production::{Production, ProductionModel},
        script::ScriptModel,
    },
    services::feature_flag,
    // `filters` must be in scope for the Template derives below — askama's
    // generated code calls `filters::<name>` unqualified at the derive site.
    templates::{BaseContext, ScriptTitleGroupView, ScriptVersionView, User, filters},
};

/// Pages of the management workspace. Used to set the `active_tab` so the
/// sidebar can highlight the current section.
#[derive(Debug, Clone, Copy)]
enum ManageTab {
    Overview,
    Script,
    Breakdown,
    Schedule,
    CallSheets,
    Team,
}

impl ManageTab {
    fn slug(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Script => "script",
            Self::Breakdown => "breakdown",
            Self::Schedule => "schedule",
            Self::CallSheets => "call-sheets",
            Self::Team => "team",
        }
    }
}

/// Mounts the `/productions/manage` hub and the per-production workspace
/// tabs under `/productions/{slug}/manage`: overview (root), `script`,
/// `breakdown`, `schedule`, `call-sheets`, and `team`. Tab routes gate
/// through `require_member`; the hub gates on the flag alone.
/// (`/productions/manage` is a static segment, so axum matches it before
/// the `{slug}` captures — same precedent as `/productions/new`.)
pub fn router() -> Router {
    Router::new()
        .route("/productions/manage", get(manage_hub))
        .route("/productions/{slug}/manage", get(overview))
        .route("/productions/{slug}/manage/script", get(script_tab))
        .route("/productions/{slug}/manage/breakdown", get(breakdown_tab))
        .route("/productions/{slug}/manage/schedule", get(schedule_tab))
        .route(
            "/productions/{slug}/manage/call-sheets",
            get(call_sheets_tab),
        )
        .route("/productions/{slug}/manage/team", get(team_tab))
}

#[derive(Template)]
#[template(path = "productions/manage/hub.html")]
struct ManageHubTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    productions: Vec<crate::models::production::ProductionMembership>,
}

/// The management hub: every production the signed-in user belongs to, as
/// entry points into their workspaces, plus the create action. Gated on the
/// feature flag alone (membership is what's being listed).
async fn manage_hub(AuthenticatedUser(user): AuthenticatedUser) -> Result<Response, Error> {
    if !feature_flag::allows("production_management", Some(&user)).await {
        return Err(Error::NotFound);
    }

    let productions = ProductionModel::get_member_productions(&user.id)
        .await
        .unwrap_or_default();

    let base = BaseContext::new()
        .with_page("production-manage")
        .with_user(User::from_session_user(&user).await);
    let template = crate::with_base!(ManageHubTemplate, base, { productions });
    Ok(render(template)?.into_response())
}

/// Resolve the production by slug and verify the user is permitted to manage
/// it. Returns 404 (not 403) whenever access is denied so we don't leak that
/// management mode exists.
async fn require_member(user: &SessionUser, slug: &str) -> Result<(Production, String), Error> {
    if !feature_flag::allows("production_management", Some(user)).await {
        return Err(Error::NotFound);
    }

    let production = ProductionModel::get_by_slug(slug).await.map_err(|e| {
        error!(slug, error = %e, "manage: production lookup failed");
        Error::NotFound
    })?;

    let role = ProductionModel::get_role(&production.id, &user.id)
        .await
        .map_err(|e| {
            error!(slug, error = %e, "manage: role lookup failed");
            Error::NotFound
        })?
        .ok_or(Error::NotFound)?;

    Ok((production, role))
}

// ── Templates ──────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "productions/manage/overview.html")]
struct OverviewTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    production: ProductionView,
    active_tab: String,
    role: String,
    stats: crate::models::production::ManageDashboardStats,
    lifecycle: crate::models::production::LifecycleView,
}

#[derive(Template)]
#[template(path = "productions/manage/script.html")]
struct ScriptTabTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    production: ProductionView,
    active_tab: String,
    role: String,
    can_edit: bool,
    script_groups: Vec<ScriptTitleGroupView>,
}

#[derive(Template)]
#[template(path = "productions/manage/breakdown.html")]
struct BreakdownTabTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    production: ProductionView,
    active_tab: String,
    role: String,
}

#[derive(Template)]
#[template(path = "productions/manage/schedule.html")]
struct ScheduleTabTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    production: ProductionView,
    active_tab: String,
    role: String,
}

#[derive(Template)]
#[template(path = "productions/manage/call_sheets.html")]
struct CallSheetsTabTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    production: ProductionView,
    active_tab: String,
    role: String,
}

#[derive(Template)]
#[template(path = "productions/manage/team.html")]
struct TeamTabTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    production: ProductionView,
    active_tab: String,
    role: String,
}

/// Slim production projection for the shell header. Just what the template
/// needs to render the title + status badge + back-to-overview link.
struct ProductionView {
    title: String,
    slug: String,
    production_type: String,
    status: String,
    // Wired up in Phase 1.2: drives the conditional Episodes/Seasons nav items.
    #[allow(dead_code)]
    is_series: bool,
}

impl ProductionView {
    fn from(p: &Production) -> Self {
        // Series-like types determine whether the Episodes nav item appears.
        // The list is intentionally loose — anything with "series" in the
        // name counts. Refine when we have a proper enum.
        let lower = p.production_type.to_lowercase();
        let is_series = lower.contains("series") || lower.contains("tv");
        Self {
            title: p.title.clone(),
            slug: p.slug.clone(),
            production_type: p.production_type.clone(),
            status: p.status.clone(),
            is_series,
        }
    }
}

// ── Handlers ───────────────────────────────────────────────────────────────

async fn overview(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let (production, role) = require_member(&user, &slug).await?;

    let stats = ProductionModel::manage_dashboard_stats(&production.id)
        .await
        .unwrap_or_default();
    let lifecycle = crate::models::production::LifecycleView::from_status(&production.status);

    let base = BaseContext::new()
        .with_page("productions")
        .with_user(User::from_session_user(&user).await);

    let template = crate::with_base!(OverviewTemplate, base, {
        production: ProductionView::from(&production),
        active_tab: ManageTab::Overview.slug().to_string(),
        role,
        stats,
        lifecycle,
    });
    Ok(render(template)?.into_response())
}

async fn script_tab(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let (production, role) = require_member(&user, &slug).await?;
    let can_edit = matches!(role.as_str(), "owner" | "admin");

    let groups = ScriptModel::list_grouped_by_title(&production.id)
        .await
        .map_err(|e| {
            error!(slug, error = %e, "manage: failed to load script groups");
            Error::Internal("Failed to load scripts".to_string())
        })?;
    let script_groups: Vec<ScriptTitleGroupView> = groups
        .into_iter()
        .map(|g| ScriptTitleGroupView {
            title: g.title,
            latest: version_view(g.latest),
            older: g.older.into_iter().map(version_view).collect(),
        })
        .collect();

    let base = BaseContext::new()
        .with_page("productions")
        .with_user(User::from_session_user(&user).await);
    let template = crate::with_base!(ScriptTabTemplate, base, {
        production: ProductionView::from(&production),
        active_tab: ManageTab::Script.slug().to_string(),
        role,
        can_edit,
        script_groups,
    });
    Ok(render(template)?.into_response())
}

fn version_view(row: crate::models::script::ScriptVersionRow) -> ScriptVersionView {
    ScriptVersionView {
        id: row.id,
        version: row.version,
        file_url: row.file_url,
        file_size: row.file_size,
        mime_type: row.mime_type,
        visibility: row.visibility,
        notes: row.notes,
        created_at: row.created_at.to_rfc3339(),
        uploader_username: row.uploader_username,
        uploader_name: row.uploader_name,
    }
}

async fn breakdown_tab(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let (production, role) = require_member(&user, &slug).await?;
    let base = BaseContext::new()
        .with_page("productions")
        .with_user(User::from_session_user(&user).await);
    let template = crate::with_base!(BreakdownTabTemplate, base, {
        production: ProductionView::from(&production),
        active_tab: ManageTab::Breakdown.slug().to_string(),
        role,
    });
    Ok(render(template)?.into_response())
}

async fn schedule_tab(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let (production, role) = require_member(&user, &slug).await?;
    let base = BaseContext::new()
        .with_page("productions")
        .with_user(User::from_session_user(&user).await);
    let template = crate::with_base!(ScheduleTabTemplate, base, {
        production: ProductionView::from(&production),
        active_tab: ManageTab::Schedule.slug().to_string(),
        role,
    });
    Ok(render(template)?.into_response())
}

async fn call_sheets_tab(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let (production, role) = require_member(&user, &slug).await?;
    let base = BaseContext::new()
        .with_page("productions")
        .with_user(User::from_session_user(&user).await);
    let template = crate::with_base!(CallSheetsTabTemplate, base, {
        production: ProductionView::from(&production),
        active_tab: ManageTab::CallSheets.slug().to_string(),
        role,
    });
    Ok(render(template)?.into_response())
}

async fn team_tab(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let (production, role) = require_member(&user, &slug).await?;
    let base = BaseContext::new()
        .with_page("productions")
        .with_user(User::from_session_user(&user).await);
    let template = crate::with_base!(TeamTabTemplate, base, {
        production: ProductionView::from(&production),
        active_tab: ManageTab::Team.slug().to_string(),
        role,
    });
    Ok(render(template)?.into_response())
}

fn render<T: Template>(t: T) -> Result<Html<String>, Error> {
    t.render()
        .map(Html)
        .map_err(|e| Error::template(e.to_string()))
}

/// Re-exported for callers that need an "is current user a member?" answer
/// without going through a route handler — used by the public production
/// page to decide whether to show the "Manage" toggle.
pub async fn is_member_of(production_id: &surrealdb::types::RecordId, user: &SessionUser) -> bool {
    if !feature_flag::allows("production_management", Some(user)).await {
        return false;
    }
    ProductionModel::get_role(production_id, &user.id)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// Silences the unused-import warning when the workspace is built without
/// the management routes ever being invoked at compile-time-known sites.
#[allow(dead_code)]
fn _redirect_unused() -> Redirect {
    Redirect::to("/")
}
