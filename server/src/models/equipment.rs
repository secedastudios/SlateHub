use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use tracing::{debug, error};
use uuid::Uuid;

use crate::{db::DB, error::Error};

// ============================
// Data Structures
// ============================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EquipmentCategory {
    pub id: RecordId,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EquipmentCondition {
    pub id: RecordId,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Equipment {
    pub id: RecordId,
    pub name: String,
    pub category: EquipmentCategory,
    pub serial_number: Option<String>,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub description: Option<String>,
    pub purchase_date: Option<DateTime<Utc>>,
    pub purchase_price: Option<f64>,
    pub condition: EquipmentCondition,
    pub notes: Option<String>,
    pub qr_code: Option<String>,
    pub owner_type: String,
    pub owner_person: Option<RecordId>,
    pub owner_organization: Option<RecordId>,
    pub is_kit_item: bool,
    pub parent_kit: Option<RecordId>,
    pub is_available: bool,
    pub current_location: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EquipmentKit {
    pub id: RecordId,
    pub name: String,
    pub description: Option<String>,
    pub category: EquipmentCategory,
    pub qr_code: Option<String>,
    pub owner_type: String,
    pub owner_person: Option<RecordId>,
    pub owner_organization: Option<RecordId>,
    pub is_available: bool,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EquipmentRental {
    pub id: RecordId,
    pub equipment_id: Option<RecordId>,
    pub kit_id: Option<RecordId>,
    pub renter_type: String,
    pub renter_person: Option<RecordId>,
    pub renter_organization: Option<RecordId>,
    pub checkout_date: DateTime<Utc>,
    pub expected_return_date: Option<DateTime<Utc>>,
    pub actual_return_date: Option<DateTime<Utc>>,
    pub checkout_condition: EquipmentCondition,
    pub return_condition: Option<EquipmentCondition>,
    pub checkout_notes: Option<String>,
    pub return_notes: Option<String>,
    pub checkout_by: RecordId,
    pub return_by: Option<RecordId>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EquipmentWithKit {
    pub equipment: Equipment,
    pub kit: Option<EquipmentKit>,
}

#[derive(Debug)]
pub struct CreateEquipmentData {
    pub name: String,
    pub category: String,
    pub serial_number: Option<String>,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub description: Option<String>,
    pub purchase_date: Option<DateTime<Utc>>,
    pub purchase_price: Option<f64>,
    pub condition: String,
    pub notes: Option<String>,
    pub owner_type: String,
    pub owner_person: Option<String>,
    pub owner_organization: Option<String>,
    pub is_kit_item: bool,
    pub parent_kit: Option<String>,
    pub current_location: Option<String>,
}

#[derive(Debug)]
pub struct UpdateEquipmentData {
    pub name: String,
    pub category: String,
    pub serial_number: Option<String>,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub description: Option<String>,
    pub purchase_date: Option<DateTime<Utc>>,
    pub purchase_price: Option<f64>,
    pub condition: String,
    pub notes: Option<String>,
    pub current_location: Option<String>,
}

#[derive(Debug)]
pub struct CreateKitData {
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub owner_type: String,
    pub owner_person: Option<String>,
    pub owner_organization: Option<String>,
    pub notes: Option<String>,
    pub equipment_ids: Vec<String>,
}

#[derive(Debug)]
pub struct UpdateKitData {
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub notes: Option<String>,
    pub equipment_ids: Vec<String>,
}

#[derive(Debug)]
pub struct CheckoutData {
    pub equipment_id: Option<String>,
    pub kit_id: Option<String>,
    pub renter_type: String,
    pub renter_person: Option<String>,
    pub renter_organization: Option<String>,
    pub expected_return_date: Option<DateTime<Utc>>,
    pub condition: String,
    pub notes: Option<String>,
    pub checkout_by: String,
}

#[derive(Debug)]
pub struct CheckinData {
    pub return_condition: String,
    pub return_notes: Option<String>,
    pub return_by: String,
}

// ============================
// Model Implementation
// ============================

pub struct EquipmentModel;

impl EquipmentModel {
    // Equipment CRUD Operations

    pub async fn create_equipment(data: CreateEquipmentData) -> Result<Equipment, Error> {
        debug!("Creating new equipment: {:?}", data);

        // Generate QR code identifier
        let qr_code = format!("EQ-{}", Uuid::new_v4().to_string());

        let query = r#"
            CREATE equipment CONTENT {
                name: $name,
                category: type::thing('equipment_category', $category),
                serial_number: $serial_number,
                model: $model,
                manufacturer: $manufacturer,
                description: $description,
                purchase_date: $purchase_date,
                purchase_price: $purchase_price,
                condition: type::thing('equipment_condition', $condition),
                notes: $notes,
                qr_code: $qr_code,
                owner_type: $owner_type,
                owner_person: IF $owner_person THEN type::thing('person', $owner_person) ELSE NONE END,
                owner_organization: IF $owner_organization THEN type::thing('organization', $owner_organization) ELSE NONE END,
                is_kit_item: $is_kit_item,
                parent_kit: IF $parent_kit THEN type::thing('equipment_kit', $parent_kit) ELSE NONE END,
                is_available: true,
                current_location: $current_location,
                created_at: time::now(),
                updated_at: time::now()
            } FETCH category, condition, parent_kit;
        "#;

        let mut result = DB
            .query(query)
            .bind(("name", data.name.clone()))
            .bind(("category", data.category.clone()))
            .bind(("serial_number", data.serial_number.clone()))
            .bind(("model", data.model.clone()))
            .bind(("manufacturer", data.manufacturer.clone()))
            .bind(("description", data.description.clone()))
            .bind(("purchase_date", data.purchase_date.clone()))
            .bind(("purchase_price", data.purchase_price))
            .bind(("condition", data.condition.clone()))
            .bind(("notes", data.notes.clone()))
            .bind(("qr_code", qr_code.clone()))
            .bind(("owner_type", data.owner_type.clone()))
            .bind(("owner_person", data.owner_person.clone()))
            .bind(("owner_organization", data.owner_organization.clone()))
            .bind(("is_kit_item", data.is_kit_item))
            .bind(("parent_kit", data.parent_kit.clone()))
            .bind(("current_location", data.current_location.clone()))
            .await
            .map_err(|e| {
                error!("Failed to create equipment: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let equipment: Option<Equipment> = result.take(0).map_err(|e| {
            error!("Failed to parse created equipment: {:?}", e);
            Error::Database(e.to_string())
        })?;

        equipment.ok_or_else(|| Error::NotFound)
    }

    pub async fn get_equipment(id: &str) -> Result<Equipment, Error> {
        debug!("Getting equipment with id: {}", id);

        let query = r#"
            SELECT * FROM type::thing('equipment', $id) FETCH category, condition, parent_kit;
        "#;

        let mut result = DB
            .query(query)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get equipment: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let equipment: Option<Equipment> = result.take(0).map_err(|e| {
            error!("Failed to parse equipment: {:?}", e);
            Error::Database(e.to_string())
        })?;

        equipment.ok_or_else(|| Error::NotFound)
    }

    pub async fn update_equipment(id: &str, data: UpdateEquipmentData) -> Result<Equipment, Error> {
        debug!("Updating equipment {}: {:?}", id, data);

        let query = r#"
            UPDATE type::thing('equipment', $id) SET
                name = $name,
                category = type::thing('equipment_category', $category),
                serial_number = $serial_number,
                model = $model,
                manufacturer = $manufacturer,
                description = $description,
                purchase_date = $purchase_date,
                purchase_price = $purchase_price,
                condition = type::thing('equipment_condition', $condition),
                notes = $notes,
                current_location = $current_location,
                updated_at = time::now()
            FETCH category, condition, parent_kit;
        "#;

        let mut result = DB
            .query(query)
            .bind(("id", id.to_string()))
            .bind(("name", data.name.clone()))
            .bind(("category", data.category.clone()))
            .bind(("serial_number", data.serial_number.clone()))
            .bind(("model", data.model.clone()))
            .bind(("manufacturer", data.manufacturer.clone()))
            .bind(("description", data.description.clone()))
            .bind(("purchase_date", data.purchase_date.clone()))
            .bind(("purchase_price", data.purchase_price))
            .bind(("condition", data.condition.clone()))
            .bind(("notes", data.notes.clone()))
            .bind(("current_location", data.current_location.clone()))
            .await
            .map_err(|e| {
                error!("Failed to update equipment: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let equipment: Option<Equipment> = result.take(0).map_err(|e| {
            error!("Failed to parse updated equipment: {:?}", e);
            Error::Database(e.to_string())
        })?;

        equipment.ok_or_else(|| Error::NotFound)
    }

    pub async fn delete_equipment(id: &str) -> Result<(), Error> {
        debug!("Deleting equipment: {}", id);

        // Check if equipment is currently rented
        let active_rentals = Self::get_active_rentals_for_equipment(id).await?;
        if !active_rentals.is_empty() {
            return Err(Error::Validation(
                "Cannot delete equipment that is currently rented".to_string(),
            ));
        }

        let query = r#"
            DELETE type::thing('equipment', $id);
        "#;

        DB.query(query)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to delete equipment: {:?}", e);
                Error::Database(e.to_string())
            })?;

        Ok(())
    }

    pub async fn list_equipment_for_owner(
        owner_type: &str,
        owner_id: &str,
    ) -> Result<Vec<Equipment>, Error> {
        debug!("Listing equipment for {} owner: {}", owner_type, owner_id);

        let query = if owner_type == "person" {
            r#"
                SELECT * FROM equipment
                WHERE owner_person = type::thing('person', $owner_id)
                ORDER BY created_at DESC
                FETCH category, condition, parent_kit;
            "#
        } else {
            r#"
                SELECT * FROM equipment
                WHERE owner_organization = type::thing('organization', $owner_id)
                ORDER BY created_at DESC
                FETCH category, condition, parent_kit;
            "#
        };

        let mut result = DB
            .query(query)
            .bind(("owner_id", owner_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to list equipment: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let equipment: Vec<Equipment> = result.take(0).map_err(|e| {
            error!("Failed to parse equipment list: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(equipment)
    }

    // Kit Operations

    pub async fn create_kit(data: CreateKitData) -> Result<EquipmentKit, Error> {
        debug!("Creating new equipment kit: {:?}", data);

        // Generate QR code identifier
        let qr_code = format!("KIT-{}", Uuid::new_v4().to_string());

        let query = r#"
            BEGIN TRANSACTION;

            LET $kit = CREATE equipment_kit CONTENT {
                name: $name,
                description: $description,
                category: type::thing('equipment_category', $category),
                qr_code: $qr_code,
                owner_type: $owner_type,
                owner_person: IF $owner_person THEN type::thing('person', $owner_person) ELSE NONE END,
                owner_organization: IF $owner_organization THEN type::thing('organization', $owner_organization) ELSE NONE END,
                is_available: true,
                notes: $notes,
                created_at: time::now(),
                updated_at: time::now()
            };

            FOR $eq_id IN $equipment_ids {
                UPDATE type::thing('equipment', $eq_id) SET
                    is_kit_item = true,
                    parent_kit = $kit.id,
                    updated_at = time::now();
            };

            RETURN $kit FETCH category;

            COMMIT TRANSACTION;
        "#;

        let mut result = DB
            .query(query)
            .bind(("name", data.name.clone()))
            .bind(("description", data.description.clone()))
            .bind(("category", data.category.clone()))
            .bind(("qr_code", qr_code.clone()))
            .bind(("owner_type", data.owner_type.clone()))
            .bind(("owner_person", data.owner_person.clone()))
            .bind(("owner_organization", data.owner_organization.clone()))
            .bind(("notes", data.notes.clone()))
            .bind(("equipment_ids", data.equipment_ids.clone()))
            .await
            .map_err(|e| {
                error!("Failed to create kit: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let kit: Option<EquipmentKit> = result.take("kit").map_err(|e| {
            error!("Failed to parse created kit: {:?}", e);
            Error::Database(e.to_string())
        })?;

        kit.ok_or_else(|| Error::NotFound)
    }

    pub async fn get_kit(id: &str) -> Result<EquipmentKit, Error> {
        debug!("Getting kit with id: {}", id);

        let query = r#"
            SELECT * FROM type::thing('equipment_kit', $id) FETCH category;
        "#;

        let mut result = DB
            .query(query)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get kit: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let kit: Option<EquipmentKit> = result.take(0).map_err(|e| {
            error!("Failed to parse kit: {:?}", e);
            Error::Database(e.to_string())
        })?;

        kit.ok_or_else(|| Error::NotFound)
    }

    pub async fn get_kit_items(kit_id: &str) -> Result<Vec<Equipment>, Error> {
        debug!("Getting items for kit: {}", kit_id);

        let query = r#"
            SELECT * FROM equipment
            WHERE parent_kit = type::thing('equipment_kit', $kit_id)
            ORDER BY name
            FETCH category, condition;
        "#;

        let mut result = DB
            .query(query)
            .bind(("kit_id", kit_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get kit items: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let items: Vec<Equipment> = result.take(0).map_err(|e| {
            error!("Failed to parse kit items: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(items)
    }

    pub async fn update_kit(id: &str, data: UpdateKitData) -> Result<EquipmentKit, Error> {
        debug!("Updating kit {}: {:?}", id, data);

        let query = r#"
            BEGIN TRANSACTION;

            -- Remove kit reference from all current items
            UPDATE equipment SET
                is_kit_item = false,
                parent_kit = NONE,
                updated_at = time::now()
            WHERE parent_kit = type::thing('equipment_kit', $id);

            -- Update kit
            LET $kit = UPDATE type::thing('equipment_kit', $id) SET
                name = $name,
                description = $description,
                category = type::thing('equipment_category', $category),
                notes = $notes,
                updated_at = time::now();

            -- Add new kit items
            FOR $eq_id IN $equipment_ids {
                UPDATE type::thing('equipment', $eq_id) SET
                    is_kit_item = true,
                    parent_kit = type::thing('equipment_kit', $id),
                    updated_at = time::now();
            };

            RETURN $kit FETCH category;

            COMMIT TRANSACTION;
        "#;

        let mut result = DB
            .query(query)
            .bind(("id", id.to_string()))
            .bind(("name", data.name.clone()))
            .bind(("description", data.description.clone()))
            .bind(("category", data.category.clone()))
            .bind(("notes", data.notes.clone()))
            .bind(("equipment_ids", data.equipment_ids.clone()))
            .await
            .map_err(|e| {
                error!("Failed to update kit: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let kit: Option<EquipmentKit> = result.take("kit").map_err(|e| {
            error!("Failed to parse updated kit: {:?}", e);
            Error::Database(e.to_string())
        })?;

        kit.ok_or_else(|| Error::NotFound)
    }

    pub async fn delete_kit(id: &str) -> Result<(), Error> {
        debug!("Deleting kit: {}", id);

        // Check if kit is currently rented
        let active_rentals = Self::get_active_rentals_for_kit(id).await?;
        if !active_rentals.is_empty() {
            return Err(Error::Validation(
                "Cannot delete kit that is currently rented".to_string(),
            ));
        }

        let query = r#"
            BEGIN TRANSACTION;

            -- Remove kit reference from all items
            UPDATE equipment SET
                is_kit_item = false,
                parent_kit = NONE,
                updated_at = time::now()
            WHERE parent_kit = type::thing('equipment_kit', $id);

            -- Delete the kit
            DELETE type::thing('equipment_kit', $id);

            COMMIT TRANSACTION;
        "#;

        DB.query(query)
            .bind(("id", id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to delete kit: {:?}", e);
                Error::Database(e.to_string())
            })?;

        Ok(())
    }

    pub async fn list_kits_for_owner(
        owner_type: &str,
        owner_id: &str,
    ) -> Result<Vec<EquipmentKit>, Error> {
        debug!("Listing kits for {} owner: {}", owner_type, owner_id);

        let query = if owner_type == "person" {
            r#"
                SELECT * FROM equipment_kit
                WHERE owner_person = type::thing('person', $owner_id)
                ORDER BY created_at DESC
                FETCH category;
            "#
        } else {
            r#"
                SELECT * FROM equipment_kit
                WHERE owner_organization = type::thing('organization', $owner_id)
                ORDER BY created_at DESC
                FETCH category;
            "#
        };

        let mut result = DB
            .query(query)
            .bind(("owner_id", owner_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to list kits: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let kits: Vec<EquipmentKit> = result.take(0).map_err(|e| {
            error!("Failed to parse kit list: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(kits)
    }

    // Rental Operations

    pub async fn checkout_equipment(data: CheckoutData) -> Result<EquipmentRental, Error> {
        debug!("Checking out equipment: {:?}", data);

        // Verify equipment or kit is available
        if let Some(ref eq_id) = data.equipment_id {
            let equipment = Self::get_equipment(eq_id).await?;
            if !equipment.is_available {
                return Err(Error::Validation(
                    "Equipment is not available for checkout".to_string(),
                ));
            }
        }

        if let Some(ref kit_id) = data.kit_id {
            let kit = Self::get_kit(kit_id).await?;
            if !kit.is_available {
                return Err(Error::Validation(
                    "Kit is not available for checkout".to_string(),
                ));
            }
        }

        let query = r#"
            BEGIN TRANSACTION;

            -- Create rental record
            LET $rental = CREATE equipment_rental CONTENT {
                equipment_id: IF $equipment_id THEN type::thing('equipment', $equipment_id) ELSE NONE END,
                kit_id: IF $kit_id THEN type::thing('equipment_kit', $kit_id) ELSE NONE END,
                renter_type: $renter_type,
                renter_person: IF $renter_person THEN type::thing('person', $renter_person) ELSE NONE END,
                renter_organization: IF $renter_organization THEN type::thing('organization', $renter_organization) ELSE NONE END,
                checkout_date: time::now(),
                expected_return_date: $expected_return_date,
                actual_return_date: NONE,
                checkout_condition: type::thing('equipment_condition', $condition),
                return_condition: NONE,
                checkout_notes: $notes,
                return_notes: NONE,
                checkout_by: type::thing('person', $checkout_by),
                return_by: NONE,
                is_active: true,
                created_at: time::now(),
                updated_at: time::now()
            };

            -- Update equipment availability
            IF $equipment_id THEN
                UPDATE type::thing('equipment', $equipment_id) SET
                    is_available = false,
                    updated_at = time::now()
            END;

            -- Update kit availability (and all its items)
            IF $kit_id THEN {
                UPDATE type::thing('equipment_kit', $kit_id) SET
                    is_available = false,
                    updated_at = time::now();

                UPDATE equipment SET
                    is_available = false,
                    updated_at = time::now()
                WHERE parent_kit = type::thing('equipment_kit', $kit_id);
            } END;

            RETURN $rental FETCH checkout_condition;

            COMMIT TRANSACTION;
        "#;

        let mut result = DB
            .query(query)
            .bind(("equipment_id", data.equipment_id.clone()))
            .bind(("kit_id", data.kit_id.clone()))
            .bind(("renter_type", data.renter_type.clone()))
            .bind(("renter_person", data.renter_person.clone()))
            .bind(("renter_organization", data.renter_organization.clone()))
            .bind(("expected_return_date", data.expected_return_date.clone()))
            .bind(("condition", data.condition.clone()))
            .bind(("notes", data.notes.clone()))
            .bind(("checkout_by", data.checkout_by.clone()))
            .await
            .map_err(|e| {
                error!("Failed to checkout equipment: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let rental: Option<EquipmentRental> = result.take("rental").map_err(|e| {
            error!("Failed to parse rental: {:?}", e);
            Error::Database(e.to_string())
        })?;

        rental.ok_or_else(|| Error::NotFound)
    }

    pub async fn checkin_equipment(
        rental_id: &str,
        data: CheckinData,
    ) -> Result<EquipmentRental, Error> {
        debug!("Checking in rental {}: {:?}", rental_id, data);

        let query = r#"
            BEGIN TRANSACTION;

            -- Get the rental
            LET $rental = SELECT * FROM type::thing('equipment_rental', $rental_id);

            -- Update rental record
            LET $updated_rental = UPDATE type::thing('equipment_rental', $rental_id) SET
                actual_return_date = time::now(),
                return_condition = type::thing('equipment_condition', $return_condition),
                return_notes = $return_notes,
                return_by = type::thing('person', $return_by),
                is_active = false,
                updated_at = time::now();

            -- Update equipment availability
            IF $rental.equipment_id THEN
                UPDATE $rental.equipment_id SET
                    is_available = true,
                    updated_at = time::now()
            END;

            -- Update kit availability (and all its items)
            IF $rental.kit_id THEN {
                UPDATE $rental.kit_id SET
                    is_available = true,
                    updated_at = time::now();

                UPDATE equipment SET
                    is_available = true,
                    updated_at = time::now()
                WHERE parent_kit = $rental.kit_id;
            } END;

            RETURN $updated_rental FETCH checkout_condition, return_condition;

            COMMIT TRANSACTION;
        "#;

        let mut result = DB
            .query(query)
            .bind(("rental_id", rental_id.to_string()))
            .bind(("return_condition", data.return_condition.clone()))
            .bind(("return_notes", data.return_notes.clone()))
            .bind(("return_by", data.return_by.clone()))
            .await
            .map_err(|e| {
                error!("Failed to checkin equipment: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let rental: Option<EquipmentRental> = result.take("updated_rental").map_err(|e| {
            error!("Failed to parse rental: {:?}", e);
            Error::Database(e.to_string())
        })?;

        rental.ok_or_else(|| Error::NotFound)
    }

    pub async fn get_active_rentals_for_equipment(
        equipment_id: &str,
    ) -> Result<Vec<EquipmentRental>, Error> {
        debug!("Getting active rentals for equipment: {}", equipment_id);

        let query = r#"
            SELECT * FROM equipment_rental
            WHERE equipment_id = type::thing('equipment', $equipment_id)
            AND is_active = true
            ORDER BY checkout_date DESC
            FETCH checkout_condition, return_condition;
        "#;

        let mut result = DB
            .query(query)
            .bind(("equipment_id", equipment_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get rentals: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let rentals: Vec<EquipmentRental> = result.take(0).map_err(|e| {
            error!("Failed to parse rentals: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(rentals)
    }

    pub async fn get_active_rentals_for_kit(kit_id: &str) -> Result<Vec<EquipmentRental>, Error> {
        debug!("Getting active rentals for kit: {}", kit_id);

        let query = r#"
            SELECT * FROM equipment_rental
            WHERE kit_id = type::thing('equipment_kit', $kit_id)
            AND is_active = true
            ORDER BY checkout_date DESC
            FETCH checkout_condition, return_condition;
        "#;

        let mut result = DB
            .query(query)
            .bind(("kit_id", kit_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get rentals: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let rentals: Vec<EquipmentRental> = result.take(0).map_err(|e| {
            error!("Failed to parse rentals: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(rentals)
    }

    // Helper Methods

    pub async fn get_all_categories() -> Result<Vec<EquipmentCategory>, Error> {
        debug!("Getting all equipment categories");

        let query = r#"
            SELECT * FROM equipment_category ORDER BY name;
        "#;

        let mut result = DB.query(query).await.map_err(|e| {
            error!("Failed to get categories: {:?}", e);
            Error::Database(e.to_string())
        })?;

        let categories: Vec<EquipmentCategory> = result.take(0).map_err(|e| {
            error!("Failed to parse categories: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(categories)
    }

    pub async fn get_all_conditions() -> Result<Vec<EquipmentCondition>, Error> {
        debug!("Getting all equipment conditions");

        let query = r#"
            SELECT * FROM equipment_condition ORDER BY name;
        "#;

        let mut result = DB.query(query).await.map_err(|e| {
            error!("Failed to get conditions: {:?}", e);
            Error::Database(e.to_string())
        })?;

        let conditions: Vec<EquipmentCondition> = result.take(0).map_err(|e| {
            error!("Failed to parse conditions: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(conditions)
    }

    pub async fn get_rental(rental_id: &str) -> Result<EquipmentRental, Error> {
        debug!("Getting rental with id: {}", rental_id);

        let query = r#"
            SELECT * FROM type::thing('equipment_rental', $rental_id)
            FETCH checkout_condition, return_condition;
        "#;

        let mut result = DB
            .query(query)
            .bind(("rental_id", rental_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get rental: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let rental: Option<EquipmentRental> = result.take(0).map_err(|e| {
            error!("Failed to parse rental: {:?}", e);
            Error::Database(e.to_string())
        })?;

        rental.ok_or_else(|| Error::NotFound)
    }

    pub async fn get_rental_history_for_equipment(
        equipment_id: &str,
    ) -> Result<Vec<EquipmentRental>, Error> {
        debug!("Getting rental history for equipment: {}", equipment_id);

        let query = r#"
            SELECT * FROM equipment_rental
            WHERE equipment_id = type::thing('equipment', $equipment_id)
            ORDER BY checkout_date DESC
            FETCH checkout_condition, return_condition;
        "#;

        let mut result = DB
            .query(query)
            .bind(("equipment_id", equipment_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get rental history: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let rentals: Vec<EquipmentRental> = result.take(0).map_err(|e| {
            error!("Failed to parse rental history: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(rentals)
    }

    pub async fn get_rental_history_for_kit(kit_id: &str) -> Result<Vec<EquipmentRental>, Error> {
        debug!("Getting rental history for kit: {}", kit_id);

        let query = r#"
            SELECT * FROM equipment_rental
            WHERE kit_id = type::thing('equipment_kit', $kit_id)
            ORDER BY checkout_date DESC
            FETCH checkout_condition, return_condition;
        "#;

        let mut result = DB
            .query(query)
            .bind(("kit_id", kit_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get rental history: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let rentals: Vec<EquipmentRental> = result.take(0).map_err(|e| {
            error!("Failed to parse rental history: {:?}", e);
            Error::Database(e.to_string())
        })?;

        Ok(rentals)
    }

    pub async fn get_equipment_by_qr(qr_code: &str) -> Result<Equipment, Error> {
        debug!("Getting equipment by QR code: {}", qr_code);

        let query = r#"
            SELECT * FROM equipment
            WHERE qr_code = $qr_code
            FETCH category, condition, parent_kit;
        "#;

        let mut result = DB
            .query(query)
            .bind(("qr_code", qr_code.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get equipment by QR: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let equipment: Option<Equipment> = result.take(0).map_err(|e| {
            error!("Failed to parse equipment: {:?}", e);
            Error::Database(e.to_string())
        })?;

        equipment.ok_or_else(|| Error::NotFound)
    }

    pub async fn get_kit_by_qr(qr_code: &str) -> Result<EquipmentKit, Error> {
        debug!("Getting kit by QR code: {}", qr_code);

        let query = r#"
            SELECT * FROM equipment_kit
            WHERE qr_code = $qr_code
            FETCH category;
        "#;

        let mut result = DB
            .query(query)
            .bind(("qr_code", qr_code.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to get kit by QR: {:?}", e);
                Error::Database(e.to_string())
            })?;

        let kit: Option<EquipmentKit> = result.take(0).map_err(|e| {
            error!("Failed to parse kit: {:?}", e);
            Error::Database(e.to_string())
        })?;

        kit.ok_or_else(|| Error::NotFound)
    }
}
