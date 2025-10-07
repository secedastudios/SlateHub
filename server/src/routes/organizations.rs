use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, Request},
    response::{Html, Json, Redirect},
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
    templates::{BaseContext, User},
};

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
    pub _message: Option<String>,
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
        base = base.with_user(User {
            id: user.id.clone(),
            name: user.username.clone(),
            email: user.email.clone(),
            avatar: format!("/api/avatar?id={}", user.id),
            avatar_url: Some(format!("/api/avatar?id={}", user.id)),
            initials: user
                .username
                .chars()
                .take(2)
                .collect::<String>()
                .to_uppercase(),
        });
    }

    // Use model to fetch organizations
    let model = OrganizationModel::new();
    let organizations = model
        .search(
            params.q.as_deref(),
            params.org_type.as_deref(),
            params.location.as_deref(),
        )
        .await?;

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
    base = base.with_user(User {
        id: user.id.clone(),
        name: user.username.clone(),
        email: user.email.clone(),
        avatar: format!("/api/avatar?id={}", user.id),
        avatar_url: Some(format!("/api/avatar?id={}", user.id)),
        initials: user
            .username
            .chars()
            .take(2)
            .collect::<String>()
            .to_uppercase(),
    });

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
    base = base.with_user(User {
        id: user.id.clone(),
        name: user.username.clone(),
        email: user.email.clone(),
        avatar: format!("/api/avatar?id={}", user.id),
        avatar_url: Some(format!("/api/avatar?id={}", user.id)),
        initials: user
            .username
            .chars()
            .take(2)
            .collect::<String>()
            .to_uppercase(),
    });

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
        base = base.with_user(User {
            id: user.id.clone(),
            name: user.username.clone(),
            email: user.email.clone(),
            avatar: format!("/api/avatar?id={}", user.id),
            avatar_url: Some(format!("/api/avatar?id={}", user.id)),
            initials: user
                .username
                .chars()
                .take(2)
                .collect::<String>()
                .to_uppercase(),
        });

        // Check user's role in the organization using model
        if let Some(member_role) = model
            .get_member_role(&organization.id.to_string(), &user.id)
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
    let members = model.get_members(&organization.id.to_string()).await?;

    let template = OrganizationProfileTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        organization,
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
        .get_member_role(&organization.id.to_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) {
        return Err(Error::Forbidden);
    }

    let mut base = BaseContext::new().with_page("edit-organization");
    base = base.with_user(User {
        id: user.id.clone(),
        name: user.username.clone(),
        email: user.email.clone(),
        avatar: format!("/api/avatar?id={}", user.id),
        avatar_url: Some(format!("/api/avatar?id={}", user.id)),
        initials: user
            .username
            .chars()
            .take(2)
            .collect::<String>()
            .to_uppercase(),
    });

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
        .get_member_role(&organization.id.to_string(), &user.id)
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
        .update(&organization.id.to_string(), update_data)
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
        .get_member_role(&organization.id.to_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) {
        return Err(Error::Forbidden);
    }

    // Use model to delete
    model.delete(&organization.id.to_string()).await?;

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
    let members = model.get_members(&organization.id.to_string()).await?;

    Ok(Json(members))
}

#[axum::debug_handler]
async fn invite_member(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
    axum::Form(data): axum::Form<InviteMemberForm>,
) -> Result<Json<serde_json::Value>, Error> {
    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;

    // Check if user has permission to invite
    let role = model
        .get_member_role(&organization.id.to_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) && role != Some("admin".to_string()) {
        return Err(Error::Forbidden);
    }

    // Find user by username or email
    let invited_user_id: String = model.find_user_by_username_or_email(&data.username).await?;

    // Add member with pending status
    model
        .add_member(
            &organization.id.to_string(),
            &invited_user_id,
            &data.role,
            Some(&user.id),
        )
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Invitation sent successfully"
    })))
}

#[axum::debug_handler]
async fn update_member_role(
    AuthenticatedUser(user): AuthenticatedUser,
    Path((slug, member_id)): Path<(String, String)>,
    axum::Form(data): axum::Form<UpdateRoleForm>,
) -> Result<Redirect, Error> {
    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;

    // Check if user is owner
    let role = model
        .get_member_role(&organization.id.to_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) {
        return Err(Error::Forbidden);
    }

    // Update member role
    model.update_member_role(&member_id, &data.role).await?;

    Ok(Redirect::to(&format!("/org/{}", slug)))
}

async fn remove_member(
    Path((slug, member_id)): Path<(String, String)>,
    request: Request,
) -> Result<Redirect, Error> {
    // Check if user is authenticated
    let user = request.get_user().ok_or(Error::Unauthorized)?;

    let model = OrganizationModel::new();
    let organization = model.get_by_slug(&slug).await?;

    // Check if user is owner
    let role = model
        .get_member_role(&organization.id.to_string(), &user.id)
        .await?;
    if role != Some("owner".to_string()) {
        return Err(Error::Forbidden);
    }

    // Remove member
    model.remove_member(&member_id).await?;

    Ok(Redirect::to(&format!("/org/{}", slug)))
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
        .get_member_role(&organization.id.to_string(), &user.id)
        .await?
    {
        return Ok(Json(json!({
            "success": false,
            "message": "You are already a member of this organization"
        })));
    }

    // Add as pending member
    model
        .add_member(&organization.id.to_string(), &user.id, "member", None)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Join request sent. An admin will review your request."
    })))
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
