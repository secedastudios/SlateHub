use crate::error::Error;
use crate::middleware::{AuthenticatedUser, UserExtractor};
use crate::models::involvement::InvolvementModel;
use crate::models::production::{
    CreateProductionData, ProductionMember, ProductionModel, UpdateProductionData,
};
use crate::record_id_ext::RecordIdExt;
use crate::templates::{
    BaseContext, CastCrewMember, ProductionCreateTemplate, ProductionEditTemplate,
    ProductionTemplate, ProductionsTemplate, User,
};
use askama::Template;
use axum::{
    Form, Json, Router,
    extract::{Path, Query, Request},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{debug, error, info};

/// Production routes
pub fn router() -> Router {
    Router::new()
        .route("/productions", get(list_productions))
        .route(
            "/productions/new",
            get(new_production_form).post(create_production),
        )
        .route("/productions/{slug}", get(view_production))
        .route(
            "/productions/{slug}/edit",
            get(edit_production_form).post(update_production),
        )
        .route("/productions/{slug}/delete", post(delete_production))
        .route("/productions/{slug}/members", get(get_members))
        .route("/productions/{slug}/members/add", post(add_member))
        .route("/productions/{slug}/members/remove", post(remove_member))
}

/// Query parameters for filtering productions
#[derive(Debug, Deserialize)]
struct ListQuery {
    filter: Option<String>,
    status: Option<String>,
    #[serde(rename = "type")]
    production_type: Option<String>,
    sort: Option<String>,
}

/// List all productions
async fn list_productions(
    Query(params): Query<ListQuery>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Listing productions with filters: {:?}", params);

    let mut base = BaseContext::new().with_page("productions");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let sort_by = params.sort.unwrap_or_else(|| "recent".to_string());
    let filter_text = params.filter.filter(|s| !s.is_empty());
    let status_filter = params.status.filter(|s| !s.is_empty());
    let type_filter = params.production_type.filter(|s| !s.is_empty());

    let productions = ProductionModel::list(
        None,
        status_filter.as_deref(),
        type_filter.as_deref(),
        filter_text.as_deref(),
        Some(sort_by.as_str()),
    )
    .await
    .map_err(|e| {
        error!("Failed to fetch productions: {}", e);
        Error::Database(format!("Failed to fetch productions: {}", e))
    })?;

    let productions: Vec<crate::templates::Production> = productions
        .into_iter()
        .map(|p| crate::templates::Production {
            id: p.id.key_string(),
            slug: p.slug,
            title: p.title,
            description: p.description.unwrap_or_default(),
            status: p.status,
            production_type: p.production_type,
            created_at: p.created_at.to_string(),
            owner: String::new(),
            tags: vec![],
            poster_url: p.poster_url,
        })
        .collect();

    let template = ProductionsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        productions,
        filter: filter_text,
        sort_by,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render productions template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// View a single production
async fn view_production(
    Path(slug): Path<String>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Viewing production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    let mut base = BaseContext::new().with_page("productions");

    // Add user to context if authenticated
    let mut can_edit = false;
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);

        // Check if user can edit this production
        can_edit = ProductionModel::can_edit(&production.id, &user.id)
            .await
            .unwrap_or(false);
    }

    // Get production members
    let members = ProductionModel::get_members(&production.id)
        .await
        .unwrap_or_default();

    // Fetch involvements (cast/crew) via graph traversal
    let involvements = InvolvementModel::get_for_production(&production.id)
        .await
        .unwrap_or_default();

    let is_claimed = ProductionModel::is_claimed(&production.id)
        .await
        .unwrap_or(false);

    // Split into cast and crew
    let mut cast = Vec::new();
    let mut crew = Vec::new();
    for inv in &involvements {
        let member = CastCrewMember {
            involvement_id: inv.id.to_raw_string(),
            person_name: inv.person_name.clone(),
            person_username: inv.person_username.clone(),
            person_avatar: inv.person_avatar.clone(),
            role: inv.role.clone(),
            department: inv.department.clone(),
            verification_status: inv.verification_status.clone(),
        };
        if inv.relation_type == "cast" {
            cast.push(member);
        } else {
            crew.push(member);
        }
    }

    // Fetch pending credits if user is owner
    let pending_credits = if can_edit {
        InvolvementModel::get_pending_for_production(&production.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|inv| CastCrewMember {
                involvement_id: inv.id.to_raw_string(),
                person_name: inv.person_name,
                person_username: inv.person_username,
                person_avatar: inv.person_avatar,
                role: inv.role,
                department: inv.department,
                verification_status: inv.verification_status,
            })
            .collect()
    } else {
        vec![]
    };

    let template = ProductionTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        production: crate::templates::ProductionDetail {
            id: production.id.key_string(),
            slug: production.slug.clone(),
            title: production.title,
            description: production.description,
            status: production.status,
            production_type: production.production_type,
            start_date: production.start_date.map(|d| d.to_string()),
            end_date: production.end_date.map(|d| d.to_string()),
            location: production.location,
            created_at: production.created_at.to_string(),
            updated_at: production.updated_at.to_string(),
            members: members
                .into_iter()
                .map(|m| crate::templates::ProductionMemberView {
                    id: m
                        .id
                        .strip_prefix("person:")
                        .or_else(|| m.id.strip_prefix("organization:"))
                        .unwrap_or(&m.id)
                        .to_string(),
                    name: m.name,
                    username: m.username,
                    slug: m.slug,
                    role: m.role,
                    member_type: m.member_type,
                })
                .collect(),
            can_edit,
            poster_url: production.poster_url,
            tmdb_url: production.tmdb_url,
            release_date: production.release_date,
            source: production.source,
            is_claimed,
            cast,
            crew,
            pending_credits,
        },
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render production template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show form to create a new production
#[axum::debug_handler]
async fn new_production_form(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    debug!("Showing new production form");

    let mut base = BaseContext::new().with_page("productions");
    base = base.with_user(User::from_session_user(&user).await);

    // Get production types and statuses for dropdowns
    let production_types = ProductionModel::get_production_types()
        .await
        .unwrap_or_default();
    let production_statuses = ProductionModel::get_production_statuses()
        .await
        .unwrap_or_default();

    let template = ProductionCreateTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        production_types,
        production_statuses,
        errors: None,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render production create template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Create a new production
#[axum::debug_handler]
async fn create_production(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<CreateProductionForm>,
) -> Result<Response, Error> {
    debug!("Creating new production: {}", data.title);

    // Validate form data
    if data.title.is_empty() {
        return Err(Error::Validation("Title is required".to_string()));
    }

    // Create production data
    let production_data = CreateProductionData {
        title: data.title,
        production_type: data.production_type,
        status: data.status,
        start_date: data.start_date.filter(|s| !s.is_empty()),
        end_date: data.end_date.filter(|s| !s.is_empty()),
        description: data.description.filter(|s| !s.is_empty()),
        location: data.location.filter(|s| !s.is_empty()),
    };

    // Determine creator type (check if creating as organization)
    let (creator_id, creator_type) = if let Some(org_id) = data.organization_id {
        // TODO: Verify user has permission to create for this organization
        (org_id, "organization")
    } else {
        (user.id.clone(), "person")
    };

    // Create the production
    let production = ProductionModel::create(production_data, &creator_id, creator_type).await?;

    info!(
        "Created production: {} ({})",
        production.title, production.id.display()
    );

    // Redirect to the new production page
    Ok(Redirect::to(&format!("/productions/{}", production.slug)).into_response())
}

/// Show form to edit a production
#[axum::debug_handler]
async fn edit_production_form(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    debug!("Showing edit form for production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    // Check if user can edit
    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let mut base = BaseContext::new().with_page("productions");
    base = base.with_user(User::from_session_user(&user).await);

    // Get production types and statuses for dropdowns
    let production_types = ProductionModel::get_production_types()
        .await
        .unwrap_or_default();
    let production_statuses = ProductionModel::get_production_statuses()
        .await
        .unwrap_or_default();

    let template = ProductionEditTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        production: crate::templates::ProductionEditData {
            id: production.id.key_string(),
            slug: production.slug,
            title: production.title,
            description: production.description,
            status: production.status,
            production_type: production.production_type,
            start_date: production.start_date.map(|d| d.to_string()),
            end_date: production.end_date.map(|d| d.to_string()),
            location: production.location,
        },
        production_types,
        production_statuses,
        errors: None,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render production edit template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Update a production
#[axum::debug_handler]
async fn update_production(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<UpdateProductionForm>,
) -> Result<Response, Error> {
    debug!("Updating production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    // Check if user can edit
    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Create update data
    let update_data = UpdateProductionData {
        title: data.title.filter(|s| !s.is_empty()),
        production_type: data.production_type.filter(|s| !s.is_empty()),
        status: data.status.filter(|s| !s.is_empty()),
        start_date: data.start_date.filter(|s| !s.is_empty()),
        end_date: data.end_date.filter(|s| !s.is_empty()),
        description: data.description.filter(|s| !s.is_empty()),
        location: data.location.filter(|s| !s.is_empty()),
    };

    // Update the production
    let updated = ProductionModel::update(&production.id, update_data).await?;

    info!("Updated production: {} ({})", updated.title, updated.id.display());

    // Redirect to the production page
    Ok(Redirect::to(&format!("/productions/{}", updated.slug)).into_response())
}

/// Delete a production
#[axum::debug_handler]
async fn delete_production(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Response, Error> {
    debug!("Deleting production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    // Check if user can edit (only owners can delete)
    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Delete the production
    ProductionModel::delete(&production.id).await?;

    info!(
        "Deleted production: {} ({})",
        production.title, production.id.display()
    );

    // Redirect to productions list
    Ok(Redirect::to("/productions").into_response())
}

/// Get members of a production (JSON response)
async fn get_members(Path(slug): Path<String>) -> Result<Json<Vec<ProductionMember>>, Error> {
    debug!("Getting members for production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;
    let members = ProductionModel::get_members(&production.id).await?;

    Ok(Json(members))
}

/// Add a member to a production
#[axum::debug_handler]
async fn add_member(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<AddMemberForm>,
) -> Result<Response, Error> {
    debug!("Adding member to production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    // Check if user can edit
    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Add the member
    ProductionModel::add_member(&production.id, &data.member_id, &data.role).await?;

    info!(
        "Added member {} to production {} with role {}",
        data.member_id, production.id.display(), data.role
    );

    // Redirect back to production page
    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

/// Remove a member from a production
#[axum::debug_handler]
async fn remove_member(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<RemoveMemberForm>,
) -> Result<Response, Error> {
    debug!("Removing member from production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    // Check if user can edit
    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Remove the member
    ProductionModel::remove_member(&production.id, &data.member_id).await?;

    info!(
        "Removed member {} from production {}",
        data.member_id, production.id.display()
    );

    // Redirect back to production page
    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

// Form structures

#[derive(Debug, Deserialize)]
struct CreateProductionForm {
    title: String,
    production_type: String,
    status: String,
    start_date: Option<String>,
    end_date: Option<String>,
    description: Option<String>,
    location: Option<String>,
    organization_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateProductionForm {
    title: Option<String>,
    production_type: Option<String>,
    status: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    description: Option<String>,
    location: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddMemberForm {
    member_id: String,
    role: String,
}

#[derive(Debug, Deserialize)]
struct RemoveMemberForm {
    member_id: String,
}
