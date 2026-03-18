use crate::error::Error;
use crate::middleware::{AuthenticatedUser, UserExtractor};
use crate::models::likes::LikesModel;
use crate::models::location::{
    CreateLocationData, CreateRateData, LocationModel, LocationRate, UpdateLocationData,
};
use crate::record_id_ext::RecordIdExt;
use crate::serde_utils::deserialize_optional_i32;
use crate::templates::{
    BaseContext, LocationCreateTemplate, LocationEditTemplate, LocationTemplate, LocationsTemplate,
    User,
};
use askama::Template;
use axum::{
    Form, Json, Router,
    extract::{Path, Query, Request},
    http::header,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use surrealdb::types::RecordId;
use tracing::{debug, error, info};
use crate::services::embedding::generate_embedding_async;
use crate::services::search_log::log_search;

const PAGE_SIZE: usize = 20;

/// Location routes
pub fn router() -> Router {
    Router::new()
        .route("/locations", get(list_locations))
        .route(
            "/locations/new",
            get(new_location_form).post(create_location),
        )
        .route("/locations/{id}", get(view_location))
        .route(
            "/locations/{id}/edit",
            get(edit_location_form).post(update_location),
        )
        .route("/locations/{id}/delete", post(delete_location))
        .route("/locations/{id}/rates", get(get_rates))
        .route("/locations/{id}/rates/add", post(add_rate))
        .route("/locations/{id}/rates/{rate_id}/delete", post(delete_rate))
        .route("/api/locations/more-sse", get(locations_more_sse))
}

/// Query parameters for filtering locations
#[derive(Debug, Deserialize)]
struct ListQuery {
    filter: Option<String>,
    city: Option<String>,
    public_only: Option<bool>,
    sort: Option<String>,
}

/// List all locations
async fn list_locations(
    Query(params): Query<ListQuery>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Listing locations with filters: {:?}", params);

    let mut base = BaseContext::new().with_page("locations");

    // Add user to context if authenticated
    let user_id = if let Some(user) = request.get_user() {
        let id = user.id.clone();
        base = base.with_user(User::from_session_user(&user).await);
        Some(id)
    } else {
        None
    };

    let sort_by = params.sort.unwrap_or_else(|| "recent".to_string());
    let filter_text = params.filter.filter(|s| !s.is_empty());
    let city_text = params.city.filter(|s| !s.is_empty());

    let query_embedding = if let Some(ref f) = filter_text {
        generate_embedding_async(f).await.ok()
    } else {
        None
    };

    if let Some(ref f) = filter_text {
        log_search(f, "web", "locations", None);
    }

    // Determine if showing only public locations or user's locations
    let (locations, show_private) = if params.public_only.unwrap_or(true) || user_id.is_none() {
        (
            LocationModel::list(
                Some(PAGE_SIZE + 1), true, city_text.as_deref(), None,
                filter_text.as_deref(), query_embedding.clone(), Some(sort_by.as_str()), 0,
            ).await?,
            false,
        )
    } else {
        let mut all_locations = LocationModel::list(
            Some(PAGE_SIZE + 1), false, city_text.as_deref(), None,
            filter_text.as_deref(), query_embedding.clone(), Some(sort_by.as_str()), 0,
        ).await?;

        if let Some(ref uid) = user_id {
            all_locations.retain(|loc| loc.is_public || loc.created_by.key_string() == *uid);
        }

        (all_locations, true)
    };

    let has_more = locations.len() > PAGE_SIZE;
    let locations: Vec<crate::templates::LocationView> = locations
        .into_iter()
        .take(PAGE_SIZE)
        .map(|l| crate::templates::LocationView {
            id: l.id.key_string(),
            name: l.name,
            address: l.address,
            city: l.city,
            state: l.state,
            country: l.country,
            description: l.description,
            is_public: l.is_public,
            profile_photo: l.profile_photo,
            created_at: l.created_at.to_string(),
        })
        .collect();

    // Fetch liked IDs if user is logged in
    let liked_ids = if let Some(ref uid) = user_id {
        let person_rid = if uid.starts_with("person:") {
            RecordId::parse_simple(uid).ok()
        } else {
            Some(RecordId::new("person", uid.as_str()))
        };
        if let Some(rid) = person_rid {
            let target_ids: Vec<RecordId> = locations
                .iter()
                .map(|l| RecordId::new("location", l.id.as_str()))
                .collect();
            LikesModel::get_liked_ids(&rid, &target_ids)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|id| id.strip_prefix("location:").unwrap_or(&id).to_string())
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let template = LocationsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        locations,
        filter: filter_text,
        city: city_text,
        show_private,
        sort_by,
        liked_ids,
        has_more,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render locations template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// View a single location
async fn view_location(Path(id): Path<String>, request: Request) -> Result<Html<String>, Error> {
    debug!("Viewing location: {}", id);

    let location_id = RecordId::new("location", id.as_str());
    let location = LocationModel::get(&location_id).await?;

    let mut base = BaseContext::new().with_page("locations");

    // Add user to context if authenticated
    let mut can_edit = false;
    let mut is_liked = false;
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);

        // Check if user can edit this location
        can_edit = LocationModel::can_edit(&location.id, &user.id)
            .await
            .unwrap_or(false);

        // Check if user has liked this location
        let person_rid = if user.id.starts_with("person:") {
            RecordId::parse_simple(&user.id).ok()
        } else {
            Some(RecordId::new("person", user.id.as_str()))
        };
        if let Some(rid) = person_rid {
            is_liked = LikesModel::is_liked(&rid, &location.id).await.unwrap_or(false);
        }
    }

    // Get location rates
    let rates = LocationModel::get_rates(&location.id)
        .await
        .unwrap_or_default();

    let template = LocationTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        location: crate::templates::LocationDetail {
            id: location.id.key_string(),
            name: location.name,
            address: location.address,
            city: location.city,
            state: location.state,
            country: location.country,
            postal_code: location.postal_code,
            description: location.description,
            contact_name: location.contact_name,
            contact_email: location.contact_email,
            contact_phone: location.contact_phone,
            is_public: location.is_public,
            amenities: location.amenities,
            restrictions: location.restrictions,
            parking_info: location.parking_info,
            max_capacity: location.max_capacity,
            profile_photo: location.profile_photo,
            photos: location.photos.into_iter().map(|p| crate::templates::LocationPhoto {
                url: p.url,
                thumbnail_url: p.thumbnail_url,
                caption: p.caption,
            }).collect(),
            created_at: location.created_at.to_string(),
            updated_at: location.updated_at.to_string(),
            rates: rates
                .into_iter()
                .map(|r| crate::templates::RateView {
                    id: r
                        .id
                        .strip_prefix("location_rate:")
                        .unwrap_or(&r.id)
                        .to_string(),
                    rate_type: r.rate_type,
                    amount: r.amount,
                    currency: r.currency,
                    minimum_duration: r.minimum_duration,
                    description: r.description,
                })
                .collect(),
            can_edit,
        },
        is_liked,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render location template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show form to create a new location
#[axum::debug_handler]
async fn new_location_form(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    debug!("Showing new location form");

    let mut base = BaseContext::new().with_page("locations");
    base = base.with_user(User::from_session_user(&user).await);

    let template = LocationCreateTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        errors: None,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render location create template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Create a new location
#[axum::debug_handler]
async fn create_location(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(data): Form<CreateLocationForm>,
) -> Result<Response, Error> {
    debug!("Creating new location: {}", data.name);

    // Validate required fields
    if data.name.is_empty() {
        return Err(Error::Validation("Name is required".to_string()));
    }
    if data.address.is_empty() {
        return Err(Error::Validation("Address is required".to_string()));
    }
    if data.contact_name.is_empty() {
        return Err(Error::Validation("Contact name is required".to_string()));
    }
    if data.contact_email.is_empty() {
        return Err(Error::Validation("Contact email is required".to_string()));
    }

    // Create location data
    let location_data = CreateLocationData {
        name: data.name,
        address: data.address,
        city: data.city,
        state: data.state,
        country: data.country,
        postal_code: data.postal_code.filter(|s| !s.is_empty()),
        description: data.description.filter(|s| !s.is_empty()),
        contact_name: data.contact_name,
        contact_email: data.contact_email,
        contact_phone: data.contact_phone.filter(|s| !s.is_empty()),
        is_public: data.is_public.unwrap_or(false),
        amenities: data
            .amenities
            .map(|a| a.split(',').map(|s| s.trim().to_string()).collect()),
        restrictions: data
            .restrictions
            .map(|r| r.split(',').map(|s| s.trim().to_string()).collect()),
        parking_info: data.parking_info.filter(|s| !s.is_empty()),
        max_capacity: data.max_capacity,
    };

    // Create the location
    let location = LocationModel::create(location_data, &user.id).await?;

    info!("Created location: {} ({})", location.name, location.id.display());

    // Redirect to the edit page so user can add photos
    Ok(Redirect::to(&format!("/locations/{}/edit", location.id.key_string())).into_response())
}

/// Show form to edit a location
#[axum::debug_handler]
async fn edit_location_form(
    Path(id): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    debug!("Showing edit form for location: {}", id);

    let location_id = RecordId::new("location", id.as_str());
    let location = LocationModel::get(&location_id).await?;

    // Check if user can edit
    if !LocationModel::can_edit(&location.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    let mut base = BaseContext::new().with_page("locations");
    base = base.with_user(User::from_session_user(&user).await);

    let template = LocationEditTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        location: crate::templates::LocationEditData {
            id: location.id.key_string(),
            name: location.name,
            address: location.address,
            city: location.city,
            state: location.state,
            country: location.country,
            postal_code: location.postal_code,
            description: location.description,
            contact_name: location.contact_name,
            contact_email: location.contact_email,
            contact_phone: location.contact_phone,
            is_public: location.is_public,
            amenities: location.amenities.map(|a| a.join(", ")),
            restrictions: location.restrictions.map(|r| r.join(", ")),
            parking_info: location.parking_info,
            max_capacity: location.max_capacity,
            profile_photo: location.profile_photo,
            photos: location.photos.into_iter().map(|p| crate::templates::LocationPhoto {
                url: p.url,
                thumbnail_url: p.thumbnail_url,
                caption: p.caption,
            }).collect(),
        },
        errors: None,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render location edit template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

/// Update a location
#[axum::debug_handler]
async fn update_location(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
    Form(data): Form<UpdateLocationForm>,
) -> Result<Response, Error> {
    debug!("Updating location: {}", id);

    let location_id = RecordId::new("location", id.as_str());
    let location = LocationModel::get(&location_id).await?;

    // Check if user can edit
    if !LocationModel::can_edit(&location.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Create update data
    let update_data = UpdateLocationData {
        name: data.name.filter(|s| !s.is_empty()),
        address: data.address.filter(|s| !s.is_empty()),
        city: data.city.filter(|s| !s.is_empty()),
        state: data.state.filter(|s| !s.is_empty()),
        country: data.country.filter(|s| !s.is_empty()),
        postal_code: data.postal_code.filter(|s| !s.is_empty()),
        description: data.description.filter(|s| !s.is_empty()),
        contact_name: data.contact_name.filter(|s| !s.is_empty()),
        contact_email: data.contact_email.filter(|s| !s.is_empty()),
        contact_phone: data.contact_phone.filter(|s| !s.is_empty()),
        is_public: data.is_public,
        amenities: data
            .amenities
            .map(|a| a.split(',').map(|s| s.trim().to_string()).collect()),
        restrictions: data
            .restrictions
            .map(|r| r.split(',').map(|s| s.trim().to_string()).collect()),
        parking_info: data.parking_info.filter(|s| !s.is_empty()),
        max_capacity: data.max_capacity,
    };

    // Update the location
    let updated = LocationModel::update(&location.id, update_data).await?;

    info!("Updated location: {} ({})", updated.name, updated.id.display());

    // Redirect to the location page
    Ok(Redirect::to(&format!("/locations/{}", updated.id.key_string())).into_response())
}

/// Delete a location
#[axum::debug_handler]
async fn delete_location(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Response, Error> {
    debug!("Deleting location: {}", id);

    let location_id = RecordId::new("location", id.as_str());
    let location = LocationModel::get(&location_id).await?;

    // Check if user can edit (owner can delete)
    if !LocationModel::can_edit(&location.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Delete the location
    LocationModel::delete(&location.id).await?;

    info!("Deleted location: {} ({})", location.name, location.id.display());

    // Redirect to locations list
    Ok(Redirect::to("/locations").into_response())
}

/// Get rates for a location (JSON API)
async fn get_rates(Path(id): Path<String>) -> Result<Json<Vec<LocationRate>>, Error> {
    debug!("Getting rates for location: {}", id);

    let location_id = RecordId::new("location", id.as_str());
    let location = LocationModel::get(&location_id).await?;
    let rates = LocationModel::get_rates(&location.id).await?;

    Ok(Json(rates))
}

/// Add a rate to a location
#[axum::debug_handler]
async fn add_rate(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
    Form(data): Form<CreateRateForm>,
) -> Result<Response, Error> {
    debug!("Adding rate to location: {}", id);

    let location_id = RecordId::new("location", id.as_str());
    let location = LocationModel::get(&location_id).await?;

    // Check if user can edit
    if !LocationModel::can_edit(&location.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Create rate data
    let rate_data = CreateRateData {
        rate_type: data.rate_type,
        amount: data.amount,
        currency: data.currency,
        minimum_duration: data.minimum_duration,
        description: data.description.filter(|s| !s.is_empty()),
    };

    // Add the rate
    LocationModel::add_rate(&location.id, rate_data).await?;

    info!("Added rate to location: {}", location.id.display());

    // Redirect back to location page
    Ok(Redirect::to(&format!("/locations/{}", location.id.key_string())).into_response())
}

/// Delete a rate from a location
#[axum::debug_handler]
async fn delete_rate(
    AuthenticatedUser(user): AuthenticatedUser,
    Path((id, rate_id)): Path<(String, String)>,
) -> Result<Response, Error> {
    debug!("Deleting rate {} from location: {}", rate_id, id);

    let location_id = RecordId::new("location", id.as_str());
    let location = LocationModel::get(&location_id).await?;

    // Check if user can edit
    if !LocationModel::can_edit(&location.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Delete the rate
    let full_rate_id = format!("location_rate:{}", rate_id);
    LocationModel::delete_rate(&full_rate_id).await?;

    info!("Deleted rate {} from location: {}", rate_id, location.id.display());

    // Redirect back to location page
    Ok(Redirect::to(&format!("/locations/{}", location.id.key_string())).into_response())
}

// SSE infinite scroll

#[derive(Debug, Deserialize)]
struct MoreQuery {
    offset: usize,
    filter: Option<String>,
    city: Option<String>,
    sort: Option<String>,
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

fn render_location_card(loc: &crate::templates::LocationView) -> String {
    let mut html = String::new();
    html.push_str(r#"<article class="loc-card">"#);
    html.push_str(&format!(r#"<a href="/locations/{}" class="loc-card-visual">"#, escape_html(&loc.id)));

    if let Some(ref photo) = loc.profile_photo {
        html.push_str(&format!(r#"<img src="{}" alt="{}" style="width:100%;height:100%;object-fit:cover;" />"#, escape_html(photo), escape_html(&loc.name)));
    } else {
        html.push_str(r#"<div class="loc-card-placeholder"><svg width="80" height="80" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="0.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0118 0z"/><circle cx="12" cy="10" r="3"/></svg></div>"#);
    }

    html.push_str(r#"<div class="loc-card-overlay">"#);
    html.push_str(&format!("<h3>{}</h3>", escape_html(&loc.name)));
    html.push_str(r#"<div class="loc-card-meta">"#);
    html.push_str(&format!(r#"<span class="loc-city">{}, {}</span>"#, escape_html(&loc.city), escape_html(&loc.state)));
    if loc.is_public {
        html.push_str(r#"<span class="loc-badge" data-value="public">Public</span>"#);
    } else {
        html.push_str(r#"<span class="loc-badge" data-value="private">Private</span>"#);
    }
    html.push_str("</div></div></a>");

    html.push_str(r#"<div class="loc-card-content">"#);
    if let Some(ref desc) = loc.description {
        html.push_str(&format!(
            r#"<p class="loc-card-desc">{}</p>"#,
            escape_html(desc)
        ));
    }
    html.push_str("</div></article>");

    html
}

async fn locations_more_sse(Query(params): Query<MoreQuery>) -> Response {
    let filter = params.filter.as_deref().filter(|s| !s.is_empty());
    let city = params.city.as_deref().filter(|s| !s.is_empty());
    let sort = params.sort.as_deref().filter(|s| !s.is_empty());
    let offset = params.offset;

    let query_embedding = if let Some(f) = filter {
        generate_embedding_async(f).await.ok()
    } else {
        None
    };
    let all = LocationModel::list(Some(PAGE_SIZE + 1), true, city, None, filter, query_embedding, sort, offset).await.unwrap_or_default();
    let has_more = all.len() > PAGE_SIZE;

    let locs: Vec<crate::templates::LocationView> = all.into_iter().take(PAGE_SIZE).map(|l| crate::templates::LocationView {
        id: l.id.key_string(),
        name: l.name,
        address: l.address,
        city: l.city,
        state: l.state,
        country: l.country,
        description: l.description,
        is_public: l.is_public,
        profile_photo: l.profile_photo,
        created_at: l.created_at.to_string(),
    }).collect();

    if locs.is_empty() {
        return sse_response(sse_patch_elements("#loc-sentinel", "remove", ""));
    }

    let mut replacement = String::new();
    for loc in &locs {
        replacement.push_str(&render_location_card(loc));
    }

    if has_more {
        let new_offset = offset + PAGE_SIZE;
        let mut q_params = format!("offset={}", new_offset);
        if let Some(f) = filter {
            q_params.push_str(&format!("&filter={}", urlencoding::encode(f)));
        }
        if let Some(c) = city {
            q_params.push_str(&format!("&city={}", urlencoding::encode(c)));
        }
        if let Some(s) = sort {
            q_params.push_str(&format!("&sort={}", urlencoding::encode(s)));
        }
        replacement.push_str(&format!(
            r#"<div id="loc-sentinel" data-on-intersect="@get('/api/locations/more-sse?{}')"><div class="loc-loading">Loading more...</div></div>"#,
            q_params
        ));
    }

    sse_response(sse_patch_elements("#loc-sentinel", "outer", &replacement))
}

// Form structures

#[derive(Debug, Deserialize)]
struct CreateLocationForm {
    name: String,
    address: String,
    city: String,
    state: String,
    country: String,
    postal_code: Option<String>,
    description: Option<String>,
    contact_name: String,
    contact_email: String,
    contact_phone: Option<String>,
    is_public: Option<bool>,
    amenities: Option<String>,
    restrictions: Option<String>,
    parking_info: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_i32")]
    max_capacity: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct UpdateLocationForm {
    name: Option<String>,
    address: Option<String>,
    city: Option<String>,
    state: Option<String>,
    country: Option<String>,
    postal_code: Option<String>,
    description: Option<String>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    contact_phone: Option<String>,
    is_public: Option<bool>,
    amenities: Option<String>,
    restrictions: Option<String>,
    parking_info: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_i32")]
    max_capacity: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct CreateRateForm {
    rate_type: String,
    amount: f64,
    currency: Option<String>,
    minimum_duration: Option<i32>,
    description: Option<String>,
}
