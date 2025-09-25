use axum::{
    Form, Router,
    extract::{Path, Query, Request},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

use crate::{
    error::Error,
    middleware::{AuthenticatedUser, UserExtractor},
    models::{
        equipment::{
            CheckinData, CheckoutData, CreateEquipmentData, CreateKitData, Equipment,
            EquipmentModel, UpdateEquipmentData,
        },
        organization::OrganizationModel,
        person::SessionUser,
    },
    templates::{
        BaseContext, User,
        equipment::{
            EquipmentCheckInTemplate, EquipmentCheckoutTemplate, EquipmentDetailTemplate,
            EquipmentFormTemplate, EquipmentListTemplate, KitDetailTemplate, KitFormTemplate,
        },
    },
};

// ============================
// Query Parameters
// ============================

#[derive(Debug, Deserialize)]
pub struct EquipmentQuery {
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub category: Option<String>,
    pub available_only: Option<bool>,
    pub equipment_id: Option<String>,
    pub kit_id: Option<String>,
}

// ============================
// Form Data Structures
// ============================

#[derive(Debug, Deserialize)]
pub struct EquipmentFormData {
    pub name: String,
    pub category: String,
    pub serial_number: Option<String>,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub description: Option<String>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub condition: String,
    pub notes: Option<String>,
    pub current_location: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KitFormData {
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub notes: Option<String>,
    pub equipment_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutFormData {
    pub equipment_id: Option<String>,
    pub kit_id: Option<String>,
    pub renter_type: String,
    pub renter_id: String,
    pub expected_return_date: Option<String>,
    pub condition: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckinFormData {
    pub return_condition: String,
    pub return_notes: Option<String>,
}

// ============================
// Equipment List & Management
// ============================

pub async fn list_equipment(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Query(query): Query<EquipmentQuery>,
) -> Result<Response, Error> {
    // Determine owner context
    let (owner_type, owner_id) = if let (Some(ot), Some(oi)) = (query.owner_type, query.owner_id) {
        // Verify authorization for the specified owner
        if ot == "organization" {
            // Check if user is a member of the organization
            let org_model = OrganizationModel::new();
            let _org = org_model.get_by_id(&oi).await?;
            let members = org_model.get_members(&oi).await?;
            if !members
                .iter()
                .any(|m| m.person_id.to_string() == current_user.id)
            {
                return Err(Error::Unauthorized);
            }
            ("organization".to_string(), oi)
        } else if ot == "person" && oi == current_user.id {
            ("person".to_string(), oi)
        } else {
            return Err(Error::Unauthorized);
        }
    } else {
        // Default to current user's personal equipment
        ("person".to_string(), current_user.id.clone())
    };

    // Get equipment list
    let equipment = EquipmentModel::list_equipment_for_owner(&owner_type, &owner_id).await?;

    // Get kits list
    let kits = EquipmentModel::list_kits_for_owner(&owner_type, &owner_id).await?;

    // Filter by category if specified
    let equipment: Vec<Equipment> = if let Some(category) = query.category {
        equipment
            .into_iter()
            .filter(|e| e.category.name == category)
            .collect()
    } else {
        equipment
    };

    // Filter by availability if specified
    let equipment: Vec<Equipment> = if let Some(true) = query.available_only {
        equipment.into_iter().filter(|e| e.is_available).collect()
    } else {
        equipment
    };

    let base = BaseContext::new().with_page("equipment");
    let user = User::from_session_user(&current_user).await;

    let template = EquipmentListTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: Some(user),
        current_user: Some((*current_user).clone()),
        equipment,
        kits,
        owner_type,
        owner_id,
        page_title: "Equipment".to_string(),
        error_message: None,
    };

    Ok(Html(template.to_string()).into_response())
}

// ============================
// Equipment CRUD Operations
// ============================

pub async fn show_create_equipment_form(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Query(query): Query<EquipmentQuery>,
) -> Result<Response, Error> {
    // Get categories and conditions for dropdowns
    let categories = EquipmentModel::get_all_categories().await?;
    let conditions = EquipmentModel::get_all_conditions().await?;

    let owner_type = query.owner_type.unwrap_or("person".to_string());
    let owner_id = query.owner_id.unwrap_or(current_user.id.clone());

    let base = BaseContext::new().with_page("equipment");
    let user = User::from_session_user(&current_user).await;

    let template = EquipmentFormTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: Some(user),
        current_user: Some((*current_user).clone()),
        equipment: None,
        categories,
        conditions,
        owner_type,
        owner_id,
        page_title: "Add Equipment".to_string(),
        error_message: None,
    };

    Ok(Html(template.to_string()).into_response())
}

pub async fn create_equipment(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Query(query): Query<EquipmentQuery>,
    Form(form): Form<EquipmentFormData>,
) -> Result<Response, Error> {
    let owner_type = query.owner_type.unwrap_or("person".to_string());
    let owner_id = query.owner_id.unwrap_or(current_user.id.clone());

    // Verify authorization
    if owner_type == "organization" {
        let org_model = OrganizationModel::new();
        let members = org_model.get_members(&owner_id).await?;
        if !members
            .iter()
            .any(|m| m.person_id.to_string() == current_user.id)
        {
            return Err(Error::Unauthorized);
        }
    } else if owner_id != current_user.id {
        return Err(Error::Unauthorized);
    }

    // Parse purchase date if provided
    let purchase_date = form.purchase_date.as_ref().and_then(|d| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .map(|date| date.and_hms_opt(0, 0, 0).unwrap())
            .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc))
    });

    let data = CreateEquipmentData {
        name: form.name,
        category: form.category,
        serial_number: form.serial_number,
        model: form.model,
        manufacturer: form.manufacturer,
        description: form.description,
        purchase_date,
        purchase_price: form.purchase_price,
        condition: form.condition,
        notes: form.notes,
        owner_type: owner_type.clone(),
        owner_person: if owner_type == "person" {
            Some(owner_id.clone())
        } else {
            None
        },
        owner_organization: if owner_type == "organization" {
            Some(owner_id.clone())
        } else {
            None
        },
        is_kit_item: false,
        parent_kit: None,
        current_location: form.current_location,
    };

    let equipment = EquipmentModel::create_equipment(data).await?;

    info!("Equipment created: {}", equipment.id);

    Ok(Redirect::to(&format!("/equipment/{}", equipment.id)).into_response())
}

pub async fn show_equipment_detail(
    Path(id): Path<String>,
    request: Request,
) -> Result<Response, Error> {
    let current_user_opt = request.get_user();

    let equipment = EquipmentModel::get_equipment(&id).await?;

    // Get rental history
    let rentals = EquipmentModel::get_rental_history_for_equipment(&id).await?;

    // Check if user can edit (is owner)
    let can_edit = if let Some(ref user) = current_user_opt {
        if equipment.owner_type == "person" {
            equipment
                .owner_person
                .as_ref()
                .map_or(false, |p| p.to_string() == user.id)
        } else if let Some(org_id) = equipment.owner_organization.as_ref() {
            let org_model = OrganizationModel::new();
            let members = org_model
                .get_members(&org_id.to_string())
                .await
                .unwrap_or_default();
            members.iter().any(|m| m.person_id.to_string() == user.id)
        } else {
            false
        }
    } else {
        false
    };

    let base = BaseContext::new().with_page("equipment");
    let user = if let Some(ref cu) = current_user_opt {
        Some(User::from_session_user(cu).await)
    } else {
        None
    };

    let template = EquipmentDetailTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user,
        current_user: current_user_opt.as_ref().map(|u| (**u).clone()),
        equipment,
        rentals,
        can_edit,
        page_title: "Equipment Details".to_string(),
        error_message: None,
    };

    Ok(Html(template.to_string()).into_response())
}

pub async fn show_edit_equipment_form(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Response, Error> {
    let equipment = EquipmentModel::get_equipment(&id).await?;

    // Verify authorization
    if equipment.owner_type == "person" {
        if equipment
            .owner_person
            .as_ref()
            .map_or(true, |p| p.to_string() != current_user.id)
        {
            return Err(Error::Unauthorized);
        }
    } else if let Some(org_id) = equipment.owner_organization.as_ref() {
        let org_model = OrganizationModel::new();
        let members = org_model.get_members(&org_id.to_string()).await?;
        if !members
            .iter()
            .any(|m| m.person_id.to_string() == current_user.id)
        {
            return Err(Error::Unauthorized);
        }
    } else {
        return Err(Error::Unauthorized);
    }

    // Get categories and conditions for dropdowns
    let categories = EquipmentModel::get_all_categories().await?;
    let conditions = EquipmentModel::get_all_conditions().await?;

    let base = BaseContext::new().with_page("equipment");
    let user = User::from_session_user(&current_user).await;

    let template = EquipmentFormTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: Some(user),
        current_user: Some((*current_user).clone()),
        equipment: Some(equipment.clone()),
        categories,
        conditions,
        owner_type: equipment.owner_type.clone(),
        owner_id: equipment
            .owner_person
            .or(equipment.owner_organization)
            .map(|r| r.to_string())
            .unwrap_or_default(),
        page_title: "Edit Equipment".to_string(),
        error_message: None,
    };

    Ok(Html(template.to_string()).into_response())
}

pub async fn update_equipment(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(id): Path<String>,
    Form(form): Form<EquipmentFormData>,
) -> Result<Response, Error> {
    let equipment = EquipmentModel::get_equipment(&id).await?;

    // Verify authorization
    if equipment.owner_type == "person" {
        if equipment
            .owner_person
            .as_ref()
            .map_or(true, |p| p.to_string() != current_user.id)
        {
            return Err(Error::Unauthorized);
        }
    } else if let Some(org_id) = equipment.owner_organization.as_ref() {
        let org_model = OrganizationModel::new();
        let members = org_model.get_members(&org_id.to_string()).await?;
        if !members
            .iter()
            .any(|m| m.person_id.to_string() == current_user.id)
        {
            return Err(Error::Unauthorized);
        }
    } else {
        return Err(Error::Unauthorized);
    }

    // Parse purchase date if provided
    let purchase_date = form.purchase_date.as_ref().and_then(|d| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .map(|date| date.and_hms_opt(0, 0, 0).unwrap())
            .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc))
    });

    let data = UpdateEquipmentData {
        name: form.name,
        category: form.category,
        serial_number: form.serial_number,
        model: form.model,
        manufacturer: form.manufacturer,
        description: form.description,
        purchase_date,
        purchase_price: form.purchase_price,
        condition: form.condition,
        notes: form.notes,
        current_location: form.current_location,
    };

    let updated_equipment = EquipmentModel::update_equipment(&id, data).await?;

    info!("Equipment updated: {}", updated_equipment.id);

    Ok(Redirect::to(&format!("/equipment/{}", id)).into_response())
}

pub async fn delete_equipment(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Response, Error> {
    let equipment = EquipmentModel::get_equipment(&id).await?;

    // Verify authorization
    if equipment.owner_type == "person" {
        if equipment
            .owner_person
            .as_ref()
            .map_or(true, |p| p.to_string() != current_user.id)
        {
            return Err(Error::Unauthorized);
        }
    } else if let Some(org_id) = equipment.owner_organization.as_ref() {
        let org_model = OrganizationModel::new();
        let members = org_model.get_members(&org_id.to_string()).await?;
        if !members
            .iter()
            .any(|m| m.person_id.to_string() == current_user.id)
        {
            return Err(Error::Unauthorized);
        }
    } else {
        return Err(Error::Unauthorized);
    }

    let owner_type = equipment.owner_type.clone();
    let owner_id = equipment
        .owner_person
        .or(equipment.owner_organization)
        .map(|r| r.to_string())
        .unwrap_or_default();

    EquipmentModel::delete_equipment(&id).await?;

    info!("Equipment deleted: {}", id);

    Ok(Redirect::to(&format!(
        "/equipment?owner_type={}&owner_id={}",
        owner_type, owner_id
    ))
    .into_response())
}

// ============================
// Kit Management
// ============================

pub async fn show_create_kit_form(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Query(query): Query<EquipmentQuery>,
) -> Result<Response, Error> {
    let owner_type = query.owner_type.unwrap_or("person".to_string());
    let owner_id = query.owner_id.unwrap_or(current_user.id.clone());

    // Get available equipment for this owner
    let available_equipment = EquipmentModel::list_equipment_for_owner(&owner_type, &owner_id)
        .await?
        .into_iter()
        .filter(|e| e.is_available && !e.is_kit_item)
        .collect();

    // Get categories for dropdown
    let categories = EquipmentModel::get_all_categories().await?;

    let base = BaseContext::new().with_page("equipment");
    let user = User::from_session_user(&current_user).await;

    let template = KitFormTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: Some(user),
        current_user: Some((*current_user).clone()),
        kit: None,
        available_equipment,
        selected_equipment: vec![],
        categories,
        owner_type,
        owner_id,
        page_title: "Create Equipment Kit".to_string(),
        error_message: None,
    };

    Ok(Html(template.to_string()).into_response())
}

pub async fn create_kit(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Query(query): Query<EquipmentQuery>,
    Form(form): Form<KitFormData>,
) -> Result<Response, Error> {
    let owner_type = query.owner_type.unwrap_or("person".to_string());
    let owner_id = query.owner_id.unwrap_or(current_user.id.clone());

    // Verify authorization
    if owner_type == "organization" {
        let org_model = OrganizationModel::new();
        let members = org_model.get_members(&owner_id).await?;
        if !members
            .iter()
            .any(|m| m.person_id.to_string() == current_user.id)
        {
            return Err(Error::Unauthorized);
        }
    } else if owner_id != current_user.id {
        return Err(Error::Unauthorized);
    }

    let data = CreateKitData {
        name: form.name,
        description: form.description,
        category: form.category,
        owner_type: owner_type.clone(),
        owner_person: if owner_type == "person" {
            Some(owner_id.clone())
        } else {
            None
        },
        owner_organization: if owner_type == "organization" {
            Some(owner_id.clone())
        } else {
            None
        },
        notes: form.notes,
        equipment_ids: form.equipment_ids,
    };

    let kit = EquipmentModel::create_kit(data).await?;

    info!("Kit created: {}", kit.id);

    Ok(Redirect::to(&format!("/equipment/kit/{}", kit.id)).into_response())
}

pub async fn show_kit_detail(Path(id): Path<String>, request: Request) -> Result<Response, Error> {
    let current_user_opt = request.get_user();

    let kit = EquipmentModel::get_kit(&id).await?;
    let kit_items = EquipmentModel::get_kit_items(&id).await?;

    // Get rental history
    let rentals = EquipmentModel::get_rental_history_for_kit(&id).await?;

    // Check if user can edit (is owner)
    let can_edit = if let Some(ref user) = current_user_opt {
        if kit.owner_type == "person" {
            kit.owner_person
                .as_ref()
                .map_or(false, |p| p.to_string() == user.id)
        } else if let Some(org_id) = kit.owner_organization.as_ref() {
            let org_model = OrganizationModel::new();
            let members = org_model
                .get_members(&org_id.to_string())
                .await
                .unwrap_or_default();
            members.iter().any(|m| m.person_id.to_string() == user.id)
        } else {
            false
        }
    } else {
        false
    };

    let base = BaseContext::new().with_page("equipment");
    let user = if let Some(ref cu) = current_user_opt {
        Some(User::from_session_user(cu).await)
    } else {
        None
    };

    let template = KitDetailTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user,
        current_user: current_user_opt.as_ref().map(|u| (**u).clone()),
        kit,
        kit_items,
        rentals,
        can_edit,
        page_title: "Kit Details".to_string(),
        error_message: None,
    };

    Ok(Html(template.to_string()).into_response())
}

// ============================
// Rental Operations
// ============================

pub async fn show_checkout_form(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Query(query): Query<EquipmentQuery>,
) -> Result<Response, Error> {
    // Get conditions for dropdown
    let conditions = EquipmentModel::get_all_conditions().await?;

    // Get the equipment or kit to checkout
    let (equipment, kit) = if let Some(ref eq_id) = query.equipment_id {
        (Some(EquipmentModel::get_equipment(eq_id).await?), None)
    } else if let Some(ref kit_id) = query.kit_id {
        (None, Some(EquipmentModel::get_kit(kit_id).await?))
    } else {
        return Err(Error::Validation(
            "No equipment or kit specified".to_string(),
        ));
    };

    let base = BaseContext::new().with_page("equipment");
    let user = User::from_session_user(&current_user).await;

    let template = EquipmentCheckoutTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: Some(user),
        current_user: Some((*current_user).clone()),
        equipment,
        kit,
        conditions,
        page_title: "Checkout Equipment".to_string(),
        error_message: None,
    };

    Ok(Html(template.to_string()).into_response())
}

pub async fn checkout_equipment_post(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Form(form): Form<CheckoutFormData>,
) -> Result<Response, Error> {
    // Parse expected return date if provided
    let expected_return_date = form.expected_return_date.as_ref().and_then(|d| {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .map(|date| date.and_hms_opt(0, 0, 0).unwrap())
            .map(|dt| chrono::DateTime::from_naive_utc_and_offset(dt, chrono::Utc))
    });

    let data = CheckoutData {
        equipment_id: form.equipment_id.clone(),
        kit_id: form.kit_id.clone(),
        renter_type: form.renter_type.clone(),
        renter_person: if form.renter_type == "person" {
            Some(form.renter_id.clone())
        } else {
            None
        },
        renter_organization: if form.renter_type == "organization" {
            Some(form.renter_id.clone())
        } else {
            None
        },
        expected_return_date,
        condition: form.condition,
        notes: form.notes,
        checkout_by: current_user.id.clone(),
    };

    let rental = EquipmentModel::checkout_equipment(data).await?;

    info!("Equipment checked out - rental: {}", rental.id);

    // Redirect to equipment or kit detail page
    if let Some(ref eq_id) = form.equipment_id {
        Ok(Redirect::to(&format!("/equipment/{}", eq_id)).into_response())
    } else if let Some(ref kit_id) = form.kit_id {
        Ok(Redirect::to(&format!("/equipment/kit/{}", kit_id)).into_response())
    } else {
        Ok(Redirect::to("/equipment").into_response())
    }
}

pub async fn show_checkin_form(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(rental_id): Path<String>,
) -> Result<Response, Error> {
    let rental = EquipmentModel::get_rental(&rental_id).await?;

    // Get conditions for dropdown
    let conditions = EquipmentModel::get_all_conditions().await?;

    let base = BaseContext::new().with_page("equipment");
    let user = User::from_session_user(&current_user).await;

    let template = EquipmentCheckInTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: Some(user),
        current_user: Some((*current_user).clone()),
        rental,
        conditions,
        page_title: "Return Equipment".to_string(),
        error_message: None,
    };

    Ok(Html(template.to_string()).into_response())
}

pub async fn checkin_equipment_post(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(rental_id): Path<String>,
    Form(form): Form<CheckinFormData>,
) -> Result<Response, Error> {
    let data = CheckinData {
        return_condition: form.return_condition,
        return_notes: form.return_notes,
        return_by: current_user.id.clone(),
    };

    let rental = EquipmentModel::checkin_equipment(&rental_id, data).await?;

    info!("Equipment checked in - rental: {}", rental.id);

    // Redirect to equipment or kit detail page
    if let Some(ref eq_id) = rental.equipment_id {
        Ok(Redirect::to(&format!("/equipment/{}", eq_id)).into_response())
    } else if let Some(ref kit_id) = rental.kit_id {
        Ok(Redirect::to(&format!("/equipment/kit/{}", kit_id)).into_response())
    } else {
        Ok(Redirect::to("/equipment").into_response())
    }
}

// ============================
// Router Configuration
// ============================

pub fn router() -> Router {
    Router::new()
        // Equipment list
        .route("/equipment", get(list_equipment))
        // Equipment CRUD
        .route(
            "/equipment/new",
            get(show_create_equipment_form).post(create_equipment),
        )
        .route("/equipment/{id}", get(show_equipment_detail))
        .route(
            "/equipment/{id}/edit",
            get(show_edit_equipment_form).post(update_equipment),
        )
        .route("/equipment/{id}/delete", post(delete_equipment))
        // Kit management
        .route(
            "/equipment/kit/new",
            get(show_create_kit_form).post(create_kit),
        )
        .route("/equipment/kit/{id}", get(show_kit_detail))
        // Checkout/Checkin
        .route(
            "/equipment/checkout",
            get(show_checkout_form).post(checkout_equipment_post),
        )
        .route(
            "/equipment/rental/{id}/checkin",
            get(show_checkin_form).post(checkin_equipment_post),
        )
}
