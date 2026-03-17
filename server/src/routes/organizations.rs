use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, Request},
    http::header,
    response::{Html, IntoResponse, Json, Redirect, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};

use crate::{
    error::Error,
    middleware::{AuthenticatedUser, UserExtractor},
    models::organization::{
        CreateOrganizationData, Organization, OrganizationMember, OrganizationModel,
        UpdateOrganizationData,
    },
    record_id_ext::RecordIdExt,
    templates::{BaseContext, User},
};

const PAGE_SIZE: usize = 20;

pub fn router() -> Router {
    Router::new()
        // Public organization routes
        .route("/orgs", get(list_organizations))
        .route("/my-orgs", get(my_organizations))
        .route(
            "/orgs/new",
            get(new_organization_page).post(create_organization),
        )
        .route("/orgs/test-types", get(test_organization_types))
        // Organization profile uses slug for URL: /orgs/<organization-slug>
        .route("/orgs/{slug}", get(organization_profile))
        .route(
            "/orgs/{slug}/edit",
            get(edit_organization_page).post(update_organization),
        )
        .route("/orgs/{slug}/delete", post(delete_organization))
        // Member management
        .route("/orgs/{slug}/members", get(list_members))
        .route("/orgs/{slug}/members/invite", post(invite_member))
        .route(
            "/orgs/{slug}/members/{member_id}/role",
            post(update_member_role),
        )
        .route(
            "/orgs/{slug}/members/{member_id}/remove",
            post(remove_member),
        )
        .route("/orgs/{slug}/join-request", post(request_to_join))
        // API endpoints
        .route("/api/orgs/more-sse", get(orgs_more_sse))
        .route(
            "/api/organizations/check-slug",
            get(check_slug_availability),
        )
}

// ============================
// Data Models (for forms and API)
// ============================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMembership {
    pub organization: Organization,
    pub role: String,
    pub joined_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrganizationForm {
    pub name: String,
    pub slug: String,
    pub org_type: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub contact_email: Option<String>,
    pub phone: Option<String>,
    pub services: Option<String>,     // Comma-separated
    pub founded_year: Option<String>, // Parse to i32 manually
    pub public: Option<String>,       // Checkbox value "on" or None
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrganizationForm {
    pub name: String,
    pub org_type: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub contact_email: Option<String>,
    pub phone: Option<String>,
    pub services: Option<String>,        // Comma-separated
    pub founded_year: Option<String>,    // Parse to i32 manually
    pub employees_count: Option<String>, // Parse to i32 manually
    pub public: Option<String>,          // Checkbox value "on" or None
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub org_type: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SlugCheckQuery {
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct InviteMemberForm {
    pub username: String,
    pub role: String,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleForm {
    pub role: String,
}

// ============================
// Templates
// ============================

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OrgType {
    pub id: String,
    pub name: String,
}

impl std::fmt::Display for OrgType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

#[derive(Template)]
#[template(path = "organizations/list.html")]
pub struct OrganizationsListTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub organizations: Vec<Organization>,
    pub search_query: Option<String>,
    pub org_types: Vec<OrgType>,
    pub has_more: bool,
}

#[derive(Template)]
#[template(path = "organizations/profile.html")]
pub struct OrganizationProfileTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub organization: Organization,
    pub description_html: Option<String>,
    pub members: Vec<OrganizationMember>,
    pub is_member: bool,
    pub is_admin: bool,
    pub is_owner: bool,
}

#[derive(Template)]
#[template(path = "organizations/new.html")]
pub struct NewOrganizationTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub org_types: Vec<OrgType>,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "organizations/edit.html")]
pub struct EditOrganizationTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub organization: Organization,
    pub org_types: Vec<OrgType>,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "organizations/my-orgs.html")]
pub struct MyOrganizationsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub organizations: Vec<OrganizationMembership>,
}

// ============================
// Route Handlers
// ============================

#[axum::debug_handler]
async fn list_organizations(
    Query(params): Query<SearchQuery>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Listing organizations");

    let mut base = BaseContext::new().with_page("organizations");

    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    // Use model to fetch organizations
    let model = OrganizationModel::new();
    let all_orgs = model
        .search(
            params.q.as_deref(),
            params.org_type.as_deref(),
            params.location.as_deref(),
            PAGE_SIZE + 1,
            0,
        )
        .await?;
    let has_more = all_orgs.len() > PAGE_SIZE;
    let organizations: Vec<Organization> = all_orgs.into_iter().take(PAGE_SIZE).collect();

    // Get organization types for filter
    let org_types_data = model.get_organization_types().await?;
    let org_types: Vec<OrgType> = org_types_data
        .into_iter()
        .map(|(id, name)| OrgType { id, name })
        .collect();

    let template = OrganizationsListTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        organizations,
        search_query: params.q,
        org_types,
        has_more,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render organizations list template: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn my_organizations(request: Request) -> Result<Html<String>, Error> {
    debug!("Listing user's organizations");

    // Check if user is authenticated
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    debug!(
        "Fetching organizations for user: id='{}', username='{}'",
        user.id, user.username
    );

    let mut base = BaseContext::new().with_page("my-organizations");
    base = base.with_user(User::from_session_user(&user).await);

    // Fetch user's organizations
    let model = OrganizationModel::new();
    let user_orgs = model.get_user_organizations(&user.id).await?;

    debug!(
        "Found {} organizations for user '{}'",
        user_orgs.len(),
        user.id
    );

    if user_orgs.is_empty() {
        debug!("No organizations found. Check:");
        debug!("  1. User ID format: '{}'", user.id);
        debug!("  2. Database has member_of record for this user");
        debug!("  3. invitation_status is 'accepted'");
    } else {
        for (org, role, _) in &user_orgs {
            debug!(
                "  - Organization: {} ({}), Role: {}",
                org.name, org.slug, role
            );
        }
    }

    // Convert to OrganizationMembership format
    let organizations: Vec<OrganizationMembership> = user_orgs
        .into_iter()
        .map(|(org, role, joined_at)| OrganizationMembership {
            organization: org,
            role,
            joined_at,
        })
        .collect();

    let template = MyOrganizationsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        organizations,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render my organizations template: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn new_organization_page(request: Request) -> Result<Html<String>, Error> {
    // Check if user is authenticated
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    let mut base = BaseContext::new().with_page("new-organization");
    base = base.with_user(User::from_session_user(&user).await);

    // Get organization types
    let model = OrganizationModel::new();
    let org_types_data = model.get_organization_types().await?;
    let org_types: Vec<OrgType> = org_types_data
        .into_iter()
        .map(|(id, name)| OrgType { id, name })
        .collect();

    let template = NewOrganizationTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        org_types,
        error: None,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render new organization template: {}", e);
        Error::template(e.to_string())
    })?))
}

#[axum::debug_handler]
async fn create_organization(
    AuthenticatedUser(user): AuthenticatedUser,
    axum::Form(data): axum::Form<CreateOrganizationForm>,
) -> Result<Redirect, Error> {
    // Parse services from comma-separated string
    let services: Vec<String> = data
        .services
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Parse founded_year from string to i32
    let founded_year = data
        .founded_year
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i32>().ok());

    // Prepare data for model
    let create_data = CreateOrganizationData {
        name: data.name,
        slug: data.slug.clone(),
        org_type: data.org_type,
        description: data.description.filter(|s| !s.is_empty()),
        location: data.location.filter(|s| !s.is_empty()),
        website: data.website.filter(|s| !s.is_empty()),
        contact_email: data.contact_email.filter(|s| !s.is_empty()),
        phone: data.phone.filter(|s| !s.is_empty()),
        services,
        founded_year,
        employees_count: None,
        public: data.public.as_deref() == Some("on"),
    };

    // Use model to create organization
    let model = OrganizationModel::new();
    let _org = model.create(create_data, &user.id).await?;

    info!("Organization '{}' created by user {}", data.slug, user.id);

    Ok(Redirect::to(&format!("/orgs/{}", data.slug)))
}

async fn organization_profile(
    Path(slug): Path<String>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Viewing organization profile: {}", slug);

    let mut base = BaseContext::new().with_page("organization-profile");
    let mut is_member = false;
    let mut is_admin = false;
    let mut is_owner = false;

    // Use model to get organization
    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;
    debug!("Found organization: {:?}", organization);

    // Check if user is authenticated and their membership
    let user_opt = request.get_user();
    if let Some(user) = &user_opt {
        base = base.with_user(User::from_session_user(&user).await);

        // Check user's role in the organization using model
        if let Some(member_role) = model
            .get_member_role(&organization.id.to_raw_string(), &user.id)
            .await?
        {
            is_member = true;
            is_admin = member_role == "admin" || member_role == "owner";
            is_owner = member_role == "owner";
        }
    }

    // Check if organization is public or user is a member
    if !organization.public && !is_member {
        debug!(
            "Organization {} is not public and user is not a member",
            slug
        );
        return Err(Error::Forbidden);
    }

    // Get organization members using model
    let members = model.get_members(&organization.id.to_raw_string()).await?;

    let description_html = organization
        .description
        .as_deref()
        .map(crate::markdown::render);

    let template = OrganizationProfileTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        organization,
        description_html,
        members,
        is_member,
        is_admin,
        is_owner,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render organization profile template: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn edit_organization_page(
    Path(slug): Path<String>,
    request: Request,
) -> Result<Html<String>, Error> {
    // Check if user is authenticated
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;

    // Check if user has permission to edit
    let role = model
        .get_member_role(&organization.id.to_raw_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) {
        return Err(Error::Forbidden);
    }

    let mut base = BaseContext::new().with_page("edit-organization");
    base = base.with_user(User::from_session_user(&user).await);

    let org_types_data = model.get_organization_types().await?;
    let org_types: Vec<OrgType> = org_types_data
        .into_iter()
        .map(|(id, name)| OrgType { id, name })
        .collect();

    let template = EditOrganizationTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        organization,
        org_types,
        error: None,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render edit organization template: {}", e);
        Error::template(e.to_string())
    })?))
}

#[axum::debug_handler]
async fn update_organization(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
    axum::Form(data): axum::Form<UpdateOrganizationForm>,
) -> Result<Redirect, Error> {
    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;

    // Check if user has permission to edit
    let role = model
        .get_member_role(&organization.id.to_raw_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) {
        return Err(Error::Forbidden);
    }

    // Parse services from comma-separated string
    let services: Vec<String> = data
        .services
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Parse founded_year from string to i32
    let founded_year = data
        .founded_year
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i32>().ok());

    // Parse employees_count from string to i32
    let employees_count = data
        .employees_count
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i32>().ok());

    // Prepare update data
    let update_data = UpdateOrganizationData {
        name: data.name,
        org_type: data.org_type,
        description: data.description.filter(|s| !s.is_empty()),
        location: data.location.filter(|s| !s.is_empty()),
        website: data.website.filter(|s| !s.is_empty()),
        contact_email: data.contact_email.filter(|s| !s.is_empty()),
        phone: data.phone.filter(|s| !s.is_empty()),
        services,
        founded_year,
        employees_count,
        public: data.public.as_deref() == Some("on"),
    };

    // Use model to update
    model
        .update(&organization.id.to_raw_string(), update_data)
        .await?;

    info!("Organization '{}' updated by user {}", slug, user.id);

    Ok(Redirect::to(&format!("/orgs/{}", slug)))
}

async fn test_organization_types() -> Result<Html<String>, Error> {
    debug!("Test endpoint: fetching organization types");

    let model = OrganizationModel::new();
    let org_types_data = model.get_organization_types().await?;

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Organization Types Test</title>
    <style>
        body { font-family: monospace; padding: 20px; }
        h1 { color: #333; }
        .count { color: blue; font-weight: bold; }
        .type { margin: 10px 0; padding: 10px; background: #f0f0f0; }
        .error { color: red; }
        .success { color: green; }
    </style>
</head>
<body>
    <h1>Organization Types Test</h1>
    <p>This endpoint tests if organization types are being fetched correctly from the database.</p>
    "#,
    );

    html.push_str(&format!(
        "<p class='count'>Total types found: {}</p>",
        org_types_data.len()
    ));

    if org_types_data.is_empty() {
        html.push_str("<p class='error'>ERROR: No organization types found! Database may not be initialized.</p>");
        html.push_str("<p>Run: <code>make db-init</code></p>");
    } else {
        html.push_str("<p class='success'>SUCCESS: Organization types loaded correctly!</p>");
        html.push_str("<h2>Organization Types:</h2>");
        for (id, name) in org_types_data {
            html.push_str(&format!(
                "<div class='type'>ID: <strong>{}</strong><br>Name: <strong>{}</strong></div>",
                id, name
            ));
        }
    }

    html.push_str("</body></html>");
    Ok(Html(html))
}

async fn delete_organization(
    Path(slug): Path<String>,
    request: Request,
) -> Result<Redirect, Error> {
    // Check if user is authenticated
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;

    // Check if user is owner
    let role = model
        .get_member_role(&organization.id.to_raw_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) {
        return Err(Error::Forbidden);
    }

    // Use model to delete
    model.delete(&organization.id.to_raw_string()).await?;

    info!("Organization '{}' deleted by user {}", slug, user.id);

    Ok(Redirect::to("/orgs"))
}

async fn list_members(
    Path(slug): Path<String>,
    request: Request,
) -> Result<Json<Vec<OrganizationMember>>, Error> {
    // Check if user is authenticated
    let _user = request.get_user().ok_or(Error::Unauthorized)?;

    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;
    let members = model.get_members(&organization.id.to_raw_string()).await?;

    Ok(Json(members))
}

#[axum::debug_handler]
async fn invite_member(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
    axum::Form(data): axum::Form<InviteMemberForm>,
) -> Result<Redirect, Error> {
    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;

    // Check if user has permission to invite
    let role = model
        .get_member_role(&organization.id.to_raw_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) {
        return Err(Error::Forbidden);
    }

    let org_id = organization.id.to_raw_string();
    let inviter_name = user.name.clone();

    let result = crate::services::invitation::InvitationService::invite_to_organization(
        &org_id,
        &organization.name,
        &slug,
        &data.username,
        &data.role,
        &user.id,
        &inviter_name,
        data.message.as_deref(),
    )
    .await?;

    match result {
        crate::services::invitation::InviteResult::AlreadyMember => {
            info!("User '{}' is already a member of '{}'", data.username, slug);
        }
        crate::services::invitation::InviteResult::AlreadyInvited => {
            info!(
                "User '{}' already has a pending invitation to '{}'",
                data.username, slug
            );
        }
        crate::services::invitation::InviteResult::ExistingUser => {
            info!(
                "User '{}' invited existing user '{}' to '{}'",
                user.id, data.username, slug
            );
        }
        crate::services::invitation::InviteResult::NewUserInvited => {
            info!(
                "User '{}' invited new user '{}' to '{}'",
                user.id, data.username, slug
            );
        }
    }

    Ok(Redirect::to(&format!("/orgs/{slug}")))
}

#[axum::debug_handler]
async fn update_member_role(
    AuthenticatedUser(user): AuthenticatedUser,
    Path((slug, member_id)): Path<(String, String)>,
    axum::Form(data): axum::Form<UpdateRoleForm>,
) -> Result<Redirect, Error> {
    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;
    let org_id = organization.id.to_raw_string();

    // Check if user is owner
    let role = model.get_member_role(&org_id, &user.id).await?;
    if role != Some("owner".to_string()) {
        return Err(Error::Forbidden);
    }

    // Verify the member belongs to this organization
    let members = model.get_members(&org_id).await?;
    let member_belongs = members.iter().any(|m| m.id.to_raw_string() == member_id);
    if !member_belongs {
        return Err(Error::BadRequest("Member does not belong to this organization".to_string()));
    }

    // Update member role
    model.update_member_role(&member_id, &data.role).await?;

    Ok(Redirect::to(&format!("/orgs/{}", slug)))
}

async fn remove_member(
    Path((slug, member_id)): Path<(String, String)>,
    request: Request,
) -> Result<Redirect, Error> {
    // Check if user is authenticated
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;
    let org_id = organization.id.to_raw_string();

    // Check if user is owner
    let role = model.get_member_role(&org_id, &user.id).await?;
    if role != Some("owner".to_string()) {
        return Err(Error::Forbidden);
    }

    // Verify the member belongs to this organization
    let members = model.get_members(&org_id).await?;
    let member_belongs = members.iter().any(|m| m.id.to_raw_string() == member_id);
    if !member_belongs {
        return Err(Error::BadRequest("Member does not belong to this organization".to_string()));
    }

    // Remove member
    model.remove_member(&member_id).await?;

    Ok(Redirect::to(&format!("/orgs/{}", slug)))
}

async fn request_to_join(
    Path(slug): Path<String>,
    request: Request,
) -> Result<Json<serde_json::Value>, Error> {
    // Check if user is authenticated
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;

    // Check if already a member
    if let Some(_) = model
        .get_member_role(&organization.id.to_raw_string(), &user.id)
        .await?
    {
        return Ok(Json(json!({
            "success": false,
            "message": "You are already a member of this organization"
        })));
    }

    // Add as pending member
    model
        .add_member(&organization.id.to_raw_string(), &user.id, "member", None)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Join request sent. An admin will review your request."
    })))
}

#[derive(Debug, Deserialize)]
struct MoreQuery {
    offset: usize,
    q: Option<String>,
}

fn sse_patch_elements(selector: &str, mode: &str, elements: &str) -> String {
    let mut s = format!("event: datastar-patch-elements\ndata: selector {}\ndata: mode {}\n", selector, mode);
    if !elements.is_empty() {
        s += &format!("data: elements {}\n", elements.replace('\n', " "));
    }
    s += "\n";
    s
}

fn sse_response(body: String) -> Response {
    ([(header::CONTENT_TYPE, "text/event-stream"), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

fn escape_html(s: &str) -> String {
    ammonia::clean_text(s)
}

const VERIFIED_BADGE_PATH: &str = "M22.5 12.5c0-1.58-.875-2.95-2.148-3.6.154-.435.238-.905.238-1.4 0-2.21-1.71-3.998-3.818-3.998-.47 0-.92.084-1.336.25C14.818 2.415 13.51 1.5 12 1.5s-2.816.917-3.437 2.25c-.415-.165-.866-.25-1.336-.25-2.11 0-3.818 1.79-3.818 4 0 .494.083.964.237 1.4-1.272.65-2.147 2.018-2.147 3.6 0 1.495.782 2.798 1.942 3.486-.02.17-.032.34-.032.514 0 2.21 1.708 4 3.818 4 .47 0 .92-.086 1.335-.25.62 1.334 1.926 2.25 3.437 2.25 1.512 0 2.818-.916 3.437-2.25.415.163.865.248 1.336.248 2.11 0 3.818-1.79 3.818-4 0-.174-.012-.344-.033-.513 1.158-.687 1.943-1.99 1.943-3.484zm-6.616-3.334l-4.334 6.5c-.145.217-.382.334-.625.334-.143 0-.288-.04-.416-.126l-.115-.094-2.415-2.415c-.293-.293-.293-.768 0-1.06s.768-.294 1.06 0l1.77 1.767 3.825-5.74c.23-.345.696-.436 1.04-.207.346.23.44.696.21 1.04z";

fn render_org_card(org: &Organization) -> String {
    let mut html = String::new();
    html.push_str(r#"<article data-component="card" data-type="org">"#);
    html.push_str(&format!(r#"<a href="/orgs/{}" data-role="card-visual">"#, escape_html(&org.slug)));

    if let Some(ref logo) = org.logo {
        html.push_str(&format!(r#"<img src="{}" alt="{}" loading="lazy" onerror="this.style.display='none'" />"#, escape_html(logo), escape_html(&org.name)));
    } else {
        html.push_str(&format!(r#"<div data-role="placeholder"><span>{}</span></div>"#, escape_html(&org.name)));
    }

    html.push_str(r#"<div data-role="overlay">"#);
    html.push_str(&format!("<h3>{}", escape_html(&org.name)));
    if org.verified {
        html.push_str(&format!(r##" <svg data-role="verified-badge" data-verified="org" width="16" height="16" viewBox="0 0 24 24" fill="#FFD700" aria-label="Verified Organization"><path d="{}"/></svg>"##, VERIFIED_BADGE_PATH));
    }
    html.push_str("</h3>");
    html.push_str(r#"<div data-role="meta">"#);
    html.push_str(&format!(r#"<span data-role="type-label">{}</span>"#, escape_html(&org.org_type.name)));
    if let Some(ref loc) = org.location {
        html.push_str(&format!(r#"<span data-role="loc">{}</span>"#, escape_html(loc)));
    }
    html.push_str("</div></div></a>");

    let has_content = org.description.is_some() || !org.services.is_empty();
    if has_content {
        html.push_str(r#"<div data-role="content">"#);
        if let Some(ref desc) = org.description {
            html.push_str(&format!(r#"<p data-role="desc">{}</p>"#, escape_html(desc)));
        }
        if !org.services.is_empty() {
            html.push_str(r#"<p data-role="services">"#);
            for service in org.services.iter().take(4) {
                html.push_str(&format!("<span>{}</span>", escape_html(service)));
            }
            html.push_str("</p>");
        }
        html.push_str("</div>");
    }
    html.push_str("</article>");

    html
}

async fn orgs_more_sse(Query(params): Query<MoreQuery>) -> Response {
    let search = params.q.as_deref().filter(|s| !s.is_empty());
    let offset = params.offset;

    let model = OrganizationModel::new();
    let all = model.search(search, None, None, PAGE_SIZE + 1, offset).await.unwrap_or_default();
    let has_more = all.len() > PAGE_SIZE;
    let orgs: Vec<Organization> = all.into_iter().take(PAGE_SIZE).collect();

    if orgs.is_empty() {
        return sse_response(sse_patch_elements("#orgs-sentinel", "remove", ""));
    }

    let mut replacement = String::new();
    for org in &orgs {
        replacement.push_str(&render_org_card(org));
    }

    if has_more {
        let new_offset = offset + PAGE_SIZE;
        let q_param = match search {
            Some(q) => format!("&q={}", urlencoding::encode(q)),
            None => String::new(),
        };
        replacement.push_str(&format!(
            r#"<div id="orgs-sentinel" data-on-intersect="@get('/api/orgs/more-sse?offset={}{}')"><div class="orgs-loading">Loading more...</div></div>"#,
            new_offset, q_param
        ));
    }

    sse_response(sse_patch_elements("#orgs-sentinel", "outer", &replacement))
}

async fn check_slug_availability(
    Query(params): Query<SlugCheckQuery>,
) -> Result<Json<serde_json::Value>, Error> {
    let model = OrganizationModel::new();
    let (available, reason) = model.check_slug_availability(&params.slug).await?;

    Ok(Json(json!({
        "available": available,
        "reason": reason
    })))
}
