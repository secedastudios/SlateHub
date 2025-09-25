use crate::error::Error;
use crate::middleware::{AuthenticatedUser, UserExtractor};
use crate::models::location::{
    CreateLocationData, CreateRateData, LocationModel, LocationRate, UpdateLocationData,
};
use crate::serde_utils::deserialize_optional_i32;
use crate::templates::{
    BaseContext, LocationCreateTemplate, LocationEditTemplate, LocationTemplate, LocationsTemplate,
    User,
};
use askama::Template;
use axum::{
    Form, Json, Router,
    extract::{Path, Query, Request},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use surrealdb::RecordId;
use tracing::{debug, error, info};

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

    // Determine if showing only public locations or user's locations
    let (locations, show_private) = if params.public_only.unwrap_or(true) || user_id.is_none() {
        // Show only public locations
        (
            LocationModel::list(None, true, params.city.as_deref(), None).await?,
            false,
        )
    } else {
        // Show user's locations and public locations
        let mut all_locations = Vec::new();

        // Get user's locations
        if let Some(ref uid) = user_id {
            let user_locations = LocationModel::get_by_creator(uid).await?;
            all_locations.extend(user_locations);
        }

        // Get public locations
        let public_locations =
            LocationModel::list(None, true, params.city.as_deref(), None).await?;
        all_locations.extend(public_locations);

        // Deduplicate by ID
        all_locations.sort_by(|a, b| a.id.cmp(&b.id));
        all_locations.dedup_by(|a, b| a.id == b.id);

        (all_locations, true)
    };

    // Convert to template format
    let locations: Vec<crate::templates::LocationView> = locations
        .into_iter()
        .map(|l| crate::templates::LocationView {
            id: l.id.key().to_string(),
            name: l.name,
            address: l.address,
            city: l.city,
            state: l.state,
            country: l.country,
            description: l.description,
            is_public: l.is_public,
            created_at: l.created_at,
        })
        .collect();

    let template = LocationsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        locations,
        filter: params.filter,
        show_private,
        sort_by: params.sort.unwrap_or_else(|| "recent".to_string()),
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

    let location_id = RecordId::from(("location", id.as_str()));
    let location = LocationModel::get(&location_id).await?;

    let mut base = BaseContext::new().with_page("locations");

    // Add user to context if authenticated
    let mut can_edit = false;
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);

        // Check if user can edit this location
        can_edit = LocationModel::can_edit(&location.id, &user.id)
            .await
            .unwrap_or(false);
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
            id: location.id.key().to_string(),
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
            created_at: location.created_at,
            updated_at: location.updated_at,
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

    info!("Created location: {} ({})", location.name, location.id);

    // Redirect to the new location page
    Ok(Redirect::to(&format!("/locations/{}", location.id.key())).into_response())
}

/// Show form to edit a location
#[axum::debug_handler]
async fn edit_location_form(
    Path(id): Path<String>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    debug!("Showing edit form for location: {}", id);

    let location_id = RecordId::from(("location", id.as_str()));
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
            id: location.id.key().to_string(),
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

    let location_id = RecordId::from(("location", id.as_str()));
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

    info!("Updated location: {} ({})", updated.name, updated.id);

    // Redirect to the location page
    Ok(Redirect::to(&format!("/locations/{}", updated.id.key())).into_response())
}

/// Delete a location
#[axum::debug_handler]
async fn delete_location(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Response, Error> {
    debug!("Deleting location: {}", id);

    let location_id = RecordId::from(("location", id.as_str()));
    let location = LocationModel::get(&location_id).await?;

    // Check if user can edit (owner can delete)
    if !LocationModel::can_edit(&location.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Delete the location
    LocationModel::delete(&location.id).await?;

    info!("Deleted location: {} ({})", location.name, location.id);

    // Redirect to locations list
    Ok(Redirect::to("/locations").into_response())
}

/// Get rates for a location (JSON API)
async fn get_rates(Path(id): Path<String>) -> Result<Json<Vec<LocationRate>>, Error> {
    debug!("Getting rates for location: {}", id);

    let location_id = RecordId::from(("location", id.as_str()));
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

    let location_id = RecordId::from(("location", id.as_str()));
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

    info!("Added rate to location: {}", location.id);

    // Redirect back to location page
    Ok(Redirect::to(&format!("/locations/{}", location.id.key())).into_response())
}

/// Delete a rate from a location
#[axum::debug_handler]
async fn delete_rate(
    AuthenticatedUser(user): AuthenticatedUser,
    Path((id, rate_id)): Path<(String, String)>,
) -> Result<Response, Error> {
    debug!("Deleting rate {} from location: {}", rate_id, id);

    let location_id = RecordId::from(("location", id.as_str()));
    let location = LocationModel::get(&location_id).await?;

    // Check if user can edit
    if !LocationModel::can_edit(&location.id, &user.id).await? {
        return Err(Error::Forbidden);
    }

    // Delete the rate
    let full_rate_id = format!("location_rate:{}", rate_id);
    LocationModel::delete_rate(&full_rate_id).await?;

    info!("Deleted rate {} from location: {}", rate_id, location.id);

    // Redirect back to location page
    Ok(Redirect::to(&format!("/locations/{}", location.id.key())).into_response())
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
