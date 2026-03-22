use crate::error::Error;
use crate::middleware::{AuthenticatedUser, UserExtractor};
use crate::models::involvement::InvolvementModel;
use crate::models::production::{
    CreateProductionData, ProductionMember, ProductionMembership, ProductionModel,
    UpdateProductionData,
};
use crate::models::script::ScriptModel;
use crate::record_id_ext::RecordIdExt;
use crate::services::invitation::InvitationService;
use crate::templates::{
    BaseContext, CastCrewMember, ProductionCreateTemplate, ProductionEditTemplate,
    ProductionScriptView, ProductionTemplate, ProductionsTemplate, User,
};
use askama::Template;
use axum::{
    Json, Router,
    extract::{Path, Query, Request, multipart::Multipart},
    http::header,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum::Form;
use axum_extra::extract::Form as HtmlForm;
use serde::Deserialize;
use tracing::{debug, error, info};
use crate::services::embedding::generate_embedding_async;
use crate::services::search_log::log_search;

const PAGE_SIZE: usize = 20;

/// Merge multi-select production roles with an optional custom role into a single Vec.
/// Filters out empty strings and deduplicates.
fn merge_production_roles(selected: &[String], custom: &Option<String>) -> Vec<String> {
    let mut roles: Vec<String> = selected
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(c) = custom {
        let c = c.trim().to_string();
        if !c.is_empty() && !roles.contains(&c) {
            roles.push(c);
        }
    }
    roles
}

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

/// Production routes
pub fn router() -> Router {
    Router::new()
        .route("/productions", get(list_productions))
        .route("/my-productions", get(my_productions))
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
        .route("/productions/{slug}/members/add-org", post(add_org_member))
        .route("/productions/{slug}/members/remove", post(remove_member))
        .route("/productions/{slug}/members/update-roles", post(update_member_roles))
        .route("/productions/{slug}/invite", post(invite_to_production))
        .route("/productions/{slug}/create-invite-link", post(create_invite_link))
        .route("/productions/{slug}/revoke-invite", post(revoke_email_invite))
        .route(
            "/productions/{slug}/scripts/upload",
            post(upload_script),
        )
        .route(
            "/productions/{slug}/scripts/{script_id}/visibility",
            post(toggle_script_visibility),
        )
        .route(
            "/productions/{slug}/scripts/{script_id}/delete",
            post(delete_script),
        )
        .route("/api/productions/more-sse", get(productions_more_sse))
}

#[derive(Template)]
#[template(path = "productions/my-productions.html")]
pub struct MyProductionsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub productions: Vec<ProductionMembership>,
}

/// Show the user's productions
async fn my_productions(request: Request) -> Result<Html<String>, Error> {
    debug!("Listing user's productions");

    let user = request.get_user().ok_or(Error::Unauthorized)?;

    let mut base = BaseContext::new().with_page("my-productions");
    base = base.with_user(User::from_session_user(&user).await);

    let productions = ProductionModel::get_member_productions(&user.id)
        .await
        .unwrap_or_default();

    let template = MyProductionsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        productions,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render my productions template: {}", e);
        Error::template(e.to_string())
    })?))
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

    let query_embedding = if let Some(ref f) = filter_text {
        generate_embedding_async(f).await.ok()
    } else {
        None
    };

    let all = ProductionModel::list(
        Some(PAGE_SIZE + 1),
        status_filter.as_deref(),
        type_filter.as_deref(),
        filter_text.as_deref(),
        query_embedding,
        Some(sort_by.as_str()),
        0,
    )
    .await
    .map_err(|e| {
        error!("Failed to fetch productions: {}", e);
        Error::Database(format!("Failed to fetch productions: {}", e))
    })?;

    if let Some(ref f) = filter_text {
        log_search(f, "web", "productions", Some(all.len()));
    }

    let has_more = all.len() > PAGE_SIZE;

    let productions: Vec<crate::templates::Production> = all
        .into_iter()
        .take(PAGE_SIZE)
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
            poster_photo: p.poster_photo,
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
        has_more,
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
            person_is_identity_verified: inv
                .person_verification_status
                .as_deref()
                .map(|s| s == "identity")
                .unwrap_or(false),
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
            .map(|inv| {
                let is_verified = inv
                    .person_verification_status
                    .as_deref()
                    .map(|s| s == "identity")
                    .unwrap_or(false);
                CastCrewMember {
                    involvement_id: inv.id.to_raw_string(),
                    person_name: inv.person_name,
                    person_username: inv.person_username,
                    person_avatar: inv.person_avatar,
                    role: inv.role,
                    department: inv.department,
                    verification_status: inv.verification_status,
                    person_is_identity_verified: is_verified,
                }
            })
            .collect()
    } else {
        vec![]
    };

    // Fetch scripts (latest versions)
    let all_scripts = ScriptModel::get_latest_for_production(&production.id)
        .await
        .unwrap_or_default();
    let scripts: Vec<ProductionScriptView> = all_scripts
        .into_iter()
        .filter(|s| can_edit || s.visibility == "public")
        .map(|s| ProductionScriptView {
            id: s.id.key_string(),
            title: s.title,
            version: s.version,
            visibility: s.visibility,
            file_url: s.file_url,
            notes: s.notes,
            created_at: s.created_at.to_string(),
        })
        .collect();

    let production_roles = ProductionModel::get_roles_by_type("individual").await.unwrap_or_default();
    let org_production_roles = ProductionModel::get_roles_by_type("organization").await.unwrap_or_default();

    let all_members: Vec<crate::templates::ProductionMemberView> = members
        .into_iter()
        .map(|m| crate::templates::ProductionMemberView {
            id: m
                .id
                .strip_prefix("person:")
                .or_else(|| m.id.strip_prefix("organization:"))
                .unwrap_or(&m.id)
                .to_string(),
            name: m.name,
            username: m.username.clone(),
            slug: m.slug,
            avatar: m.avatar,
            role: m.role,
            production_roles: m.production_roles,
            member_type: m.member_type.clone(),
            invitation_status: m.invitation_status,
            is_verified: m.is_verified,
        })
        .collect();
    let person_members: Vec<_> = all_members.iter().filter(|m| m.member_type == "person").cloned().collect();
    let org_members: Vec<_> = all_members.iter().filter(|m| m.member_type == "organization").cloned().collect();

    let template = ProductionTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        production_roles,
        org_production_roles,
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
            members: all_members,
            person_members,
            org_members,
            can_edit,
            poster_url: production.poster_url,
            poster_photo: production.poster_photo,
            header_photo: production.header_photo,
            photos: production.photos.into_iter().map(|p| crate::templates::ProductionPhotoView {
                url: p.url,
                thumbnail_url: p.thumbnail_url,
                caption: p.caption,
            }).collect(),
            scripts,
            tmdb_url: production.tmdb_url,
            release_date: production.release_date,
            source: production.source,
            is_claimed,
            cast,
            crew,
            pending_credits,
            budget_level: production.budget_level,
            production_tier: production.production_tier,
            pending_email_invites: if can_edit {
                let pi_model = crate::models::pending_invitation::PendingInvitationModel::new();
                pi_model
                    .get_pending_for_production(&production.id.to_raw_string())
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|pi| crate::templates::PendingEmailInvite {
                        id: pi.id.to_raw_string(),
                        email: pi.email.unwrap_or_default(),
                        production_roles: pi.production_roles,
                        token: pi.token,
                    })
                    .collect()
            } else {
                vec![]
            },
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

    // Get production types, statuses, budget levels, and tiers for dropdowns
    let production_types = ProductionModel::get_production_types()
        .await
        .unwrap_or_default();
    let production_statuses = ProductionModel::get_production_statuses()
        .await
        .unwrap_or_default();
    let budget_levels = ProductionModel::get_budget_levels()
        .await
        .unwrap_or_default();
    let production_tiers = ProductionModel::get_production_tiers()
        .await
        .unwrap_or_default();

    // Get user's organizations where they are owner or admin
    let org_model = crate::models::organization::OrganizationModel::new();
    let user_orgs = org_model
        .get_user_organizations(&user.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, role, _)| role == "owner" || role == "admin")
        .map(|(org, role, _)| crate::templates::OrgOption {
            id: org.id.to_raw_string(),
            name: org.name,
            role,
        })
        .collect();

    let production_roles = ProductionModel::get_roles_by_type("individual").await.unwrap_or_default();
    let org_production_roles = ProductionModel::get_roles_by_type("organization").await.unwrap_or_default();

    let template = ProductionCreateTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        production_types,
        production_statuses,
        budget_levels,
        production_tiers,
        user_organizations: user_orgs,
        production_roles,
        org_production_roles,
        errors: None,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render production create template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Create a new production (multipart form for poster upload)
#[axum::debug_handler]
async fn create_production(
    AuthenticatedUser(user): AuthenticatedUser,
    mut multipart: Multipart,
) -> Result<Response, Error> {
    // Extract fields from multipart
    let mut title = String::new();
    let mut production_type = String::new();
    let mut status = String::new();
    let mut start_date: Option<String> = None;
    let mut end_date: Option<String> = None;
    let mut description: Option<String> = None;
    let mut location: Option<String> = None;
    let mut organization_id: Option<String> = None;
    let mut owner_production_role: Vec<String> = Vec::new();
    let mut budget_level: Option<String> = None;
    let mut production_tier: Option<String> = None;
    let mut poster_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| Error::BadRequest(e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "poster" => {
                if let Ok(bytes) = field.bytes().await {
                    if !bytes.is_empty() {
                        poster_data = Some(bytes.to_vec());
                    }
                }
            }
            _ => {
                let value = field.text().await.unwrap_or_default();
                match name.as_str() {
                    "title" => title = value,
                    "production_type" => production_type = value,
                    "status" => status = value,
                    "start_date" => start_date = Some(value).filter(|s| !s.is_empty()),
                    "end_date" => end_date = Some(value).filter(|s| !s.is_empty()),
                    "description" => description = Some(value).filter(|s| !s.is_empty()),
                    "location" => location = Some(value).filter(|s| !s.is_empty()),
                    "organization_id" => organization_id = Some(value).filter(|s| !s.is_empty()),
                    "owner_production_role" => {
                        let v = value.trim().to_string();
                        if !v.is_empty() {
                            owner_production_role.push(v);
                        }
                    }
                    "budget_level" => budget_level = Some(value).filter(|s| !s.is_empty()),
                    "production_tier" => production_tier = Some(value).filter(|s| !s.is_empty()),
                    _ => {}
                }
            }
        }
    }

    debug!("Creating new production: {}", title);

    if title.is_empty() {
        return Err(Error::Validation("Title is required".to_string()));
    }

    let production_data = CreateProductionData {
        title,
        production_type,
        status,
        start_date,
        end_date,
        description,
        location,
        budget_level,
        production_tier,
    };

    // Determine creator type
    let (creator_id, creator_type) = if let Some(org_id) = organization_id {
        let org_model = crate::models::organization::OrganizationModel::new();
        let role = org_model.get_member_role(&org_id, &user.id).await?;
        match role.as_deref() {
            Some("owner") | Some("admin") => {}
            _ => return Err(Error::Forbidden),
        }
        (org_id, "organization")
    } else {
        (user.id.clone(), "person")
    };

    let owner_production_roles = if owner_production_role.is_empty() { None } else { Some(owner_production_role) };

    let production = ProductionModel::create(production_data, &creator_id, creator_type, owner_production_roles).await?;

    info!(
        "Created production: {} ({})",
        production.title, production.id.display()
    );
    crate::services::activity::log_activity(Some(&user.id), "production_create", &format!("/productions/{}", production.slug));

    // Upload poster if provided
    if let Some(image_bytes) = poster_data {
        let prod_id = production.id.key_string();
        if let Err(e) = upload_poster_for_production(&prod_id, &image_bytes).await {
            error!("Failed to upload poster for new production: {}", e);
            // Don't fail the creation — production is already created
        }
    }

    Ok(Redirect::to(&format!("/productions/{}", production.slug)).into_response())
}

/// Upload a poster image for a production (used during creation)
async fn upload_poster_for_production(production_id: &str, image_bytes: &[u8]) -> Result<(), Error> {
    use crate::services::s3::s3;

    let (processed, thumbnail) = crate::routes::media::process_poster(image_bytes)?;

    let image_id = ulid::Ulid::new().to_string();
    let main_key = format!("productions/{}/poster_{}.jpg", production_id, image_id);
    let thumb_key = format!("productions/{}/poster_thumb_{}.jpg", production_id, image_id);

    let s3_service = s3()?;
    s3_service.upload_file(&main_key, processed, "image/jpeg").await?;
    s3_service.upload_file(&thumb_key, thumbnail, "image/jpeg").await?;

    let main_url = format!("/api/media/{}", main_key);

    let prod_rid = surrealdb::types::RecordId::new("production", production_id);
    crate::db::DB
        .query("UPDATE $id SET poster_photo = $url")
        .bind(("id", prod_rid))
        .bind(("url", main_url))
        .await?;

    Ok(())
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

    // Get production types, statuses, budget levels, and tiers for dropdowns
    let production_types = ProductionModel::get_production_types()
        .await
        .unwrap_or_default();
    let production_statuses = ProductionModel::get_production_statuses()
        .await
        .unwrap_or_default();
    let budget_levels = ProductionModel::get_budget_levels()
        .await
        .unwrap_or_default();
    let production_tiers = ProductionModel::get_production_tiers()
        .await
        .unwrap_or_default();

    let members = ProductionModel::get_members(&production.id)
        .await
        .unwrap_or_default();
    let production_roles = ProductionModel::get_roles_by_type("individual").await.unwrap_or_default();
    let org_production_roles = ProductionModel::get_roles_by_type("organization").await.unwrap_or_default();

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
            header_photo: production.header_photo,
            poster_photo: production.poster_photo,
            photos: production.photos.into_iter().map(|p| crate::templates::ProductionPhotoView {
                url: p.url,
                thumbnail_url: p.thumbnail_url,
                caption: p.caption,
            }).collect(),
            budget_level: production.budget_level,
            production_tier: production.production_tier,
        },
        production_types,
        production_statuses,
        budget_levels,
        production_tiers,
        production_roles,
        org_production_roles,
        members: {
            let edit_members: Vec<crate::templates::ProductionMemberView> = members
                .into_iter()
                .map(|m| crate::templates::ProductionMemberView {
                    id: m
                        .id
                        .strip_prefix("person:")
                        .or_else(|| m.id.strip_prefix("organization:"))
                        .unwrap_or(&m.id)
                        .to_string(),
                    name: m.name,
                    username: m.username.clone(),
                    slug: m.slug,
                    avatar: m.avatar,
                    role: m.role,
                    production_roles: m.production_roles,
                    member_type: m.member_type.clone(),
                    invitation_status: m.invitation_status,
                    is_verified: m.is_verified,
                })
                .collect();
            edit_members
        },
        person_members: Vec::new(),
        org_members: Vec::new(),
        errors: None,
    };
    // Can't use closures in block initializers easily, so set after
    let template = {
        let pm: Vec<_> = template.members.iter().filter(|m| m.member_type == "person").cloned().collect();
        let om: Vec<_> = template.members.iter().filter(|m| m.member_type == "organization").cloned().collect();
        ProductionEditTemplate { person_members: pm, org_members: om, ..template }
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
        budget_level: data.budget_level.filter(|s| !s.is_empty()),
        production_tier: data.production_tier.filter(|s| !s.is_empty()),
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

/// Add a member to a production (via invitation)
#[axum::debug_handler]
async fn add_member(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    HtmlForm(data): HtmlForm<AddMemberForm>,
) -> Result<Response, Error> {
    debug!("Adding member to production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    // Check if user can edit
    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Build production roles: combine multi-select with optional custom role
    let production_roles = merge_production_roles(&data.production_role, &data.custom_role);

    let prod_id = production.id.to_raw_string();
    let user_name = if user.name.is_empty() {
        &user.username
    } else {
        &user.name
    };

    // Use invitation service — creates member_of with pending status + notification
    let result = InvitationService::invite_to_production(
        &prod_id,
        &production.title,
        &production.slug,
        &data.member_id,
        &data.role,
        if production_roles.is_empty() { None } else { Some(production_roles) },
        &user.id,
        user_name,
        None,
    )
    .await?;

    info!(
        "Production invite result for {}: {:?}",
        data.member_id, result
    );

    // Redirect back to production page
    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

/// Add an organization as a member of a production
#[axum::debug_handler]
async fn add_org_member(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    HtmlForm(data): HtmlForm<AddOrgMemberForm>,
) -> Result<Response, Error> {
    debug!("Adding org member to production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    // Check if user can edit
    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Build production roles: combine multi-select with optional custom role
    let production_roles = merge_production_roles(&data.production_role, &data.custom_role);

    // org_id comes as the full record id string like "organization:abc123"
    let org_id = &data.org_id;

    // Add org directly as accepted member (orgs don't need to accept invitations)
    let roles_opt = if production_roles.is_empty() { None } else { Some(production_roles.clone()) };
    ProductionModel::add_member_accepted(
        &production.id,
        org_id,
        &data.role,
        roles_opt,
    )
    .await?;

    info!(
        "Added organization {} to production {} with roles {:?}",
        org_id, production.title, production_roles
    );

    // Notify org owners about being added to this production
    let notification_model = crate::models::notification::NotificationModel::new();
    let org_model = crate::models::organization::OrganizationModel::new();
    if let Ok(owners) = org_model.get_org_owners(org_id).await {
        let user_name = if user.name.is_empty() { &user.username } else { &user.name };
        for owner_id in owners {
            let _ = notification_model
                .create(
                    &owner_id,
                    "production_membership",
                    &format!("Organization added to {}", production.title),
                    &format!(
                        "{} added your organization to the production {}",
                        user_name, production.title
                    ),
                    Some(&format!("/productions/{}", production.slug)),
                    None,
                )
                .await;
        }
    }

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

#[derive(Debug, Deserialize)]
struct RevokeInviteForm {
    invite_id: String,
}

async fn revoke_email_invite(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<RevokeInviteForm>,
) -> Result<Response, Error> {
    let production = ProductionModel::get_by_slug(&slug).await?;

    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let invite_rid = surrealdb::types::RecordId::parse_simple(&data.invite_id)
        .map_err(|e| Error::BadRequest(e.to_string()))?;

    crate::db::DB
        .query("DELETE $id")
        .bind(("id", invite_rid))
        .await?;

    info!("Revoked invite {} for production {}", data.invite_id, slug);

    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

#[derive(Debug, Deserialize)]
struct CreateInviteLinkForm {
    role: Option<String>,
    production_role: Option<String>,
}

async fn create_invite_link(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    axum::Form(data): axum::Form<CreateInviteLinkForm>,
) -> Result<Response, Error> {
    let production = ProductionModel::get_by_slug(&slug).await?;

    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let roles: Vec<String> = data.production_role
        .iter()
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .collect();
    let production_roles = if roles.is_empty() { None } else { Some(roles.as_slice()) };

    let pi_model = crate::models::pending_invitation::PendingInvitationModel::new();
    pi_model
        .create_link_invite(
            &production.id.to_raw_string(),
            &production.title,
            &production.slug,
            data.role.as_deref().unwrap_or("member"),
            &user.id,
            production_roles,
        )
        .await?;

    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

/// Update production roles for an existing member
#[axum::debug_handler]
async fn update_member_roles(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    HtmlForm(data): HtmlForm<UpdateMemberRolesForm>,
) -> Result<Response, Error> {
    debug!("Updating member roles in production: {}", slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    // Check if user can edit
    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let new_roles = merge_production_roles(&data.production_role, &data.custom_role);

    ProductionModel::update_member_roles(
        &production.id,
        &data.member_id,
        new_roles,
    )
    .await?;

    info!(
        "Updated roles for member {} in production {}",
        data.member_id, production.id.display()
    );

    // Redirect back to edit page
    Ok(Redirect::to(&format!("/productions/{}/edit", slug)).into_response())
}

// Form structures

#[derive(Debug, Deserialize)]
struct UpdateProductionForm {
    title: Option<String>,
    production_type: Option<String>,
    status: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    description: Option<String>,
    location: Option<String>,
    budget_level: Option<String>,
    production_tier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddMemberForm {
    member_id: String,
    role: String,
    #[serde(default)]
    production_role: Vec<String>,
    custom_role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddOrgMemberForm {
    org_id: String,
    role: String,
    #[serde(default)]
    production_role: Vec<String>,
    custom_role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoveMemberForm {
    member_id: String,
}

#[derive(Debug, Deserialize)]
struct InviteForm {
    #[serde(default)]
    identifier: String,
    role: String,
    #[serde(default)]
    production_role: Vec<String>,
    custom_role: Option<String>,
    message: Option<String>,
    #[serde(default)]
    invite_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateMemberRolesForm {
    member_id: String,
    #[serde(default)]
    production_role: Vec<String>,
    custom_role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToggleVisibilityForm {
    visibility: String,
}

/// Invite a user to a production (by username, email, or generate a link)
#[axum::debug_handler]
async fn invite_to_production(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    HtmlForm(data): HtmlForm<InviteForm>,
) -> Result<Response, Error> {
    let production = ProductionModel::get_by_slug(&slug).await?;

    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let production_roles = merge_production_roles(&data.production_role, &data.custom_role);

    // Generate invite link if requested
    if data.invite_type.as_deref() == Some("link") {
        debug!("Creating invite link for production: {}", slug);
        let pi_model = crate::models::pending_invitation::PendingInvitationModel::new();
        let roles_slice = if production_roles.is_empty() { None } else { Some(production_roles.as_slice()) };
        pi_model
            .create_link_invite(
                &production.id.to_raw_string(),
                &production.title,
                &production.slug,
                &data.role,
                &user.id,
                roles_slice,
            )
            .await?;

        return Ok(Redirect::to(&format!("/productions/{}", slug)).into_response());
    }

    // Otherwise invite by identifier (username or email)
    if data.identifier.is_empty() {
        return Err(Error::BadRequest("Please enter a username or email, or generate an invite link.".to_string()));
    }

    debug!("Inviting {} to production: {}", data.identifier, slug);

    let prod_id = production.id.to_raw_string();
    let user_name = if user.name.is_empty() { &user.username } else { &user.name };

    let result = InvitationService::invite_to_production(
        &prod_id,
        &production.title,
        &production.slug,
        &data.identifier,
        &data.role,
        if production_roles.is_empty() { None } else { Some(production_roles) },
        &user.id,
        user_name,
        data.message.as_deref(),
    )
    .await?;

    info!("Production invite result for {}: {:?}", data.identifier, result);

    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

/// Maximum script file size (50MB)
const MAX_SCRIPT_SIZE: usize = 50 * 1024 * 1024;
const ALLOWED_SCRIPT_TYPES: &[&str] = &["application/pdf"];

/// Upload a script to a production
#[axum::debug_handler]
async fn upload_script(
    Path(slug): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
    mut multipart: Multipart,
) -> Result<Response, Error> {
    debug!("User {} uploading script for production {}", user.username, slug);

    let production = ProductionModel::get_by_slug(&slug).await?;

    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let mut file_data: Option<(String, bytes::Bytes)> = None;
    let mut title = String::new();
    let mut visibility = "members".to_string();
    let mut notes: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::bad_request(format!("Failed to read multipart: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                if !ALLOWED_SCRIPT_TYPES.contains(&content_type.as_str()) {
                    return Err(Error::bad_request(
                        "Invalid file type. Only PDF files are allowed.",
                    ));
                }
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| Error::bad_request(format!("Failed to read file data: {}", e)))?;
                if data.len() > MAX_SCRIPT_SIZE {
                    return Err(Error::bad_request("File too large. Maximum size is 50MB."));
                }
                file_data = Some((content_type, data));
            }
            "title" => {
                title = field.text().await.unwrap_or_default();
            }
            "visibility" => {
                visibility = field.text().await.unwrap_or_else(|_| "members".to_string());
            }
            "notes" => {
                let val = field.text().await.unwrap_or_default();
                if !val.is_empty() {
                    notes = Some(val);
                }
            }
            _ => {}
        }
    }

    if title.is_empty() {
        return Err(Error::bad_request("Script title is required"));
    }

    let (content_type, data) =
        file_data.ok_or_else(|| Error::bad_request("No file provided"))?;

    let prod_key = production.id.key_string();
    let file_id = ulid::Ulid::new().to_string();
    let title_slug: String = title.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let file_key = format!(
        "productions/{}/scripts/{}_{}.pdf",
        prod_key, title_slug, file_id
    );

    let file_size = data.len() as i64;

    let s3_service = crate::services::s3::s3()?;
    s3_service
        .upload_file(&file_key, data, &content_type)
        .await?;

    let file_url = format!("/api/media/{}", file_key);

    ScriptModel::create(
        &production.id,
        &title,
        &file_url,
        &file_key,
        file_size,
        &content_type,
        &visibility,
        &user.id,
        notes.as_deref(),
    )
    .await?;

    info!(
        "Script '{}' uploaded for production {}",
        title, production.slug
    );

    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

/// Toggle script visibility
#[axum::debug_handler]
async fn toggle_script_visibility(
    Path((slug, script_id)): Path<(String, String)>,
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<ToggleVisibilityForm>,
) -> Result<Response, Error> {
    let production = ProductionModel::get_by_slug(&slug).await?;

    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let script_rid = surrealdb::types::RecordId::new("production_script", &*script_id);

    ScriptModel::update_visibility(&script_rid, &data.visibility).await?;

    info!("Script {} visibility changed to {}", script_id, data.visibility);

    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

/// Delete a script version
#[axum::debug_handler]
async fn delete_script(
    Path((slug, script_id)): Path<(String, String)>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Response, Error> {
    let production = ProductionModel::get_by_slug(&slug).await?;

    if !ProductionModel::can_edit(&production.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let script_rid = surrealdb::types::RecordId::new("production_script", &*script_id);

    if let Some(file_key) = ScriptModel::delete(&script_rid).await? {
        // Fire-and-forget S3 cleanup
        tokio::spawn(async move {
            if let Ok(s3_service) = crate::services::s3::s3() {
                let _ = s3_service.delete_file(&file_key).await;
            }
        });
    }

    info!("Script {} deleted from production {}", script_id, slug);

    Ok(Redirect::to(&format!("/productions/{}", slug)).into_response())
}

// ── Infinite-scroll SSE ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct MoreQuery {
    offset: usize,
    filter: Option<String>,
    sort: Option<String>,
}

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

fn render_production_card(p: &crate::templates::Production) -> String {
    let mut html = String::new();
    html.push_str(r#"<article class="prod-card">"#);
    html.push_str(&format!(
        r#"<a href="/productions/{}" class="prod-card-visual" data-status="{}">"#,
        escape_html(&p.slug),
        escape_html(&p.status)
    ));

    if let Some(ref photo) = p.poster_photo {
        html.push_str(&format!(
            r#"<img src="{}" alt="{}" class="prod-card-poster" loading="lazy" />"#,
            escape_html(photo),
            escape_html(&p.title)
        ));
    } else if let Some(ref url) = p.poster_url {
        html.push_str(&format!(
            r#"<img src="{}" alt="{}" class="prod-card-poster" loading="lazy" />"#,
            escape_html(url),
            escape_html(&p.title)
        ));
    } else {
        html.push_str(r#"<div class="prod-card-placeholder"><svg width="80" height="80" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="0.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"/><line x1="7" y1="2" x2="7" y2="22"/><line x1="17" y1="2" x2="17" y2="22"/><line x1="2" y1="12" x2="22" y2="12"/><line x1="2" y1="7" x2="7" y2="7"/><line x1="2" y1="17" x2="7" y2="17"/><line x1="17" y1="7" x2="22" y2="7"/><line x1="17" y1="17" x2="22" y2="17"/></svg></div>"#);
    }

    html.push_str(r#"<div class="prod-card-overlay">"#);
    html.push_str(&format!("<h3>{}</h3>", escape_html(&p.title)));
    html.push_str(&format!(
        r#"<div class="prod-card-badges"><span class="prod-badge" data-role="status" data-value="{}">{}</span><span class="prod-badge" data-role="type">{}</span></div>"#,
        escape_html(&p.status),
        escape_html(&p.status),
        escape_html(&p.production_type)
    ));
    html.push_str("</div></a>");

    html.push_str(r#"<div class="prod-card-content">"#);
    if !p.description.is_empty() {
        html.push_str(&format!(
            r#"<p class="prod-card-desc">{}</p>"#,
            escape_html(&p.description)
        ));
    }
    html.push_str("</div></article>");

    html
}

async fn productions_more_sse(Query(params): Query<MoreQuery>) -> Response {
    let filter = params.filter.as_deref().filter(|s| !s.is_empty());
    let sort = params.sort.as_deref().filter(|s| !s.is_empty());
    let offset = params.offset;

    let query_embedding = if let Some(f) = filter {
        generate_embedding_async(f).await.ok()
    } else {
        None
    };
    let all = ProductionModel::list(Some(PAGE_SIZE + 1), None, None, filter, query_embedding, sort, offset)
        .await
        .unwrap_or_default();
    let has_more = all.len() > PAGE_SIZE;

    let prods: Vec<crate::templates::Production> = all
        .into_iter()
        .take(PAGE_SIZE)
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
            poster_photo: p.poster_photo,
        })
        .collect();

    if prods.is_empty() {
        return sse_response(sse_patch_elements("#prod-sentinel", "remove", ""));
    }

    let mut replacement = String::new();
    for p in &prods {
        replacement.push_str(&render_production_card(p));
    }

    if has_more {
        let new_offset = offset + PAGE_SIZE;
        let mut q_params = format!("offset={}", new_offset);
        if let Some(f) = filter {
            q_params.push_str(&format!("&filter={}", urlencoding::encode(f)));
        }
        if let Some(s) = sort {
            q_params.push_str(&format!("&sort={}", urlencoding::encode(s)));
        }
        replacement.push_str(&format!(
            r#"<div id="prod-sentinel" data-on-intersect="@get('/api/productions/more-sse?{}')"><div class="prod-loading">Loading more...</div></div>"#,
            q_params
        ));
    }

    sse_response(sse_patch_elements("#prod-sentinel", "outer", &replacement))
}
