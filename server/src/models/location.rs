use crate::db::DB;
use crate::error::Error;
use serde::{Deserialize, Serialize};
use surrealdb::{RecordId, sql::Thing};
use tracing::debug;

/// Location entity from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub id: RecordId,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub country: String,
    pub postal_code: Option<String>,
    pub description: Option<String>,
    pub contact_name: String,
    pub contact_email: String,
    pub contact_phone: Option<String>,
    pub is_public: bool,
    pub amenities: Option<Vec<String>>,
    pub restrictions: Option<Vec<String>>,
    pub parking_info: Option<String>,
    pub max_capacity: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
}

/// Data required to create a new location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLocationData {
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub country: String,
    pub postal_code: Option<String>,
    pub description: Option<String>,
    pub contact_name: String,
    pub contact_email: String,
    pub contact_phone: Option<String>,
    pub is_public: bool,
    pub amenities: Option<Vec<String>>,
    pub restrictions: Option<Vec<String>>,
    pub parking_info: Option<String>,
    pub max_capacity: Option<i32>,
}

/// Data for updating an existing location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLocationData {
    pub name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub postal_code: Option<String>,
    pub description: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub is_public: Option<bool>,
    pub amenities: Option<Vec<String>>,
    pub restrictions: Option<Vec<String>>,
    pub parking_info: Option<String>,
    pub max_capacity: Option<i32>,
}

/// Location rate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationRate {
    pub id: String,
    pub location: String,
    pub rate_type: String,
    pub amount: f64,
    pub currency: String,
    pub minimum_duration: Option<i32>,
    pub description: Option<String>,
    pub created_at: String,
}

/// Data for creating a location rate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRateData {
    pub rate_type: String,
    pub amount: f64,
    pub currency: Option<String>,
    pub minimum_duration: Option<i32>,
    pub description: Option<String>,
}

/// Location model for database operations
pub struct LocationModel;

impl LocationModel {
    /// Create a new location
    pub async fn create(data: CreateLocationData, creator_id: &str) -> Result<Location, Error> {
        debug!("Creating location: {} by {}", data.name, creator_id);

        // Create the location
        let query = r#"
            CREATE location CONTENT {
                name: $name,
                address: $address,
                city: $city,
                state: $state,
                country: $country,
                postal_code: $postal_code,
                description: $description,
                contact_name: $contact_name,
                contact_email: $contact_email,
                contact_phone: $contact_phone,
                is_public: $is_public,
                amenities: $amenities,
                restrictions: $restrictions,
                parking_info: $parking_info,
                max_capacity: $max_capacity,
                created_by: $created_by
            } RETURN *;
        "#;

        let mut result = DB
            .query(query)
            .bind(("name", data.name))
            .bind(("address", data.address))
            .bind(("city", data.city))
            .bind(("state", data.state))
            .bind(("country", data.country))
            .bind(("postal_code", data.postal_code))
            .bind(("description", data.description))
            .bind(("contact_name", data.contact_name))
            .bind(("contact_email", data.contact_email))
            .bind(("contact_phone", data.contact_phone))
            .bind(("is_public", data.is_public))
            .bind(("amenities", data.amenities))
            .bind(("restrictions", data.restrictions))
            .bind(("parking_info", data.parking_info))
            .bind(("max_capacity", data.max_capacity))
            .bind(("created_by", creator_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to create location: {}", e)))?;

        let location: Option<Location> = result.take(0)?;
        let location = location.ok_or_else(|| {
            Error::Database("Failed to create location - no result returned".to_string())
        })?;

        debug!("Successfully created location: {}", location.id);
        Ok(location)
    }

    /// Get a location by ID
    pub async fn get(location_id: &RecordId) -> Result<Location, Error> {
        debug!("Fetching location: {}", location_id);

        let mut result = DB
            .query("SELECT * FROM $location_id")
            .bind(("location_id", location_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch location: {}", e)))?;

        let location: Option<Location> = result.take(0)?;
        location.ok_or_else(|| Error::NotFound)
    }

    /// List locations with optional filters
    pub async fn list(
        limit: Option<usize>,
        public_only: bool,
        city: Option<&str>,
        creator_id: Option<&str>,
    ) -> Result<Vec<Location>, Error> {
        debug!(
            "Listing locations - public_only: {}, city: {:?}, creator: {:?}",
            public_only, city, creator_id
        );

        let mut query = String::from("SELECT * FROM location WHERE 1=1");

        if public_only {
            query.push_str(" AND is_public = true");
        }

        if city.is_some() {
            query.push_str(" AND city = $city");
        }

        if creator_id.is_some() {
            query.push_str(" AND created_by = $creator_id");
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let mut db_query = DB.query(&query);

        if let Some(city) = city {
            db_query = db_query.bind(("city", city.to_string()));
        }

        if let Some(creator_id) = creator_id {
            db_query = db_query.bind(("creator_id", creator_id.to_string()));
        }

        let mut result = db_query
            .await
            .map_err(|e| Error::Database(format!("Failed to list locations: {}", e)))?;

        let locations: Vec<Location> = result.take(0)?;
        Ok(locations)
    }

    /// Update a location
    pub async fn update(
        location_id: &RecordId,
        data: UpdateLocationData,
    ) -> Result<Location, Error> {
        debug!("Updating location: {}", location_id);

        let mut update_fields = Vec::new();

        if data.name.is_some() {
            update_fields.push("name = $name");
        }
        if data.address.is_some() {
            update_fields.push("address = $address");
        }
        if data.city.is_some() {
            update_fields.push("city = $city");
        }
        if data.state.is_some() {
            update_fields.push("state = $state");
        }
        if data.country.is_some() {
            update_fields.push("country = $country");
        }
        if data.postal_code.is_some() {
            update_fields.push("postal_code = $postal_code");
        }
        if data.description.is_some() {
            update_fields.push("description = $description");
        }
        if data.contact_name.is_some() {
            update_fields.push("contact_name = $contact_name");
        }
        if data.contact_email.is_some() {
            update_fields.push("contact_email = $contact_email");
        }
        if data.contact_phone.is_some() {
            update_fields.push("contact_phone = $contact_phone");
        }
        if data.is_public.is_some() {
            update_fields.push("is_public = $is_public");
        }
        if data.amenities.is_some() {
            update_fields.push("amenities = $amenities");
        }
        if data.restrictions.is_some() {
            update_fields.push("restrictions = $restrictions");
        }
        if data.parking_info.is_some() {
            update_fields.push("parking_info = $parking_info");
        }
        if data.max_capacity.is_some() {
            update_fields.push("max_capacity = $max_capacity");
        }

        if update_fields.is_empty() {
            return Self::get(location_id).await;
        }

        let query = format!(
            "UPDATE $location_id SET {} RETURN *",
            update_fields.join(", ")
        );

        let mut db_query = DB
            .query(&query)
            .bind(("location_id", location_id.to_string()));

        if let Some(name) = data.name {
            // Also update slug if name changes
            let slug = name
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .split('-')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("-");
            db_query = db_query.bind(("name", name));
            update_fields.push("slug = $slug");
            db_query = db_query.bind(("slug", slug));
        }

        if let Some(address) = data.address {
            db_query = db_query.bind(("address", address));
        }
        if let Some(city) = data.city {
            db_query = db_query.bind(("city", city));
        }
        if let Some(state) = data.state {
            db_query = db_query.bind(("state", state));
        }
        if let Some(country) = data.country {
            db_query = db_query.bind(("country", country));
        }
        if let Some(postal_code) = data.postal_code {
            db_query = db_query.bind(("postal_code", postal_code));
        }
        if let Some(description) = data.description {
            db_query = db_query.bind(("description", description));
        }
        if let Some(contact_name) = data.contact_name {
            db_query = db_query.bind(("contact_name", contact_name));
        }
        if let Some(contact_email) = data.contact_email {
            db_query = db_query.bind(("contact_email", contact_email));
        }
        if let Some(contact_phone) = data.contact_phone {
            db_query = db_query.bind(("contact_phone", contact_phone));
        }
        if let Some(is_public) = data.is_public {
            db_query = db_query.bind(("is_public", is_public));
        }
        if let Some(amenities) = data.amenities {
            db_query = db_query.bind(("amenities", amenities));
        }
        if let Some(restrictions) = data.restrictions {
            db_query = db_query.bind(("restrictions", restrictions));
        }
        if let Some(parking_info) = data.parking_info {
            db_query = db_query.bind(("parking_info", parking_info));
        }
        if let Some(max_capacity) = data.max_capacity {
            db_query = db_query.bind(("max_capacity", max_capacity));
        }

        let mut result = db_query
            .await
            .map_err(|e| Error::Database(format!("Failed to update location: {}", e)))?;

        let location: Option<Location> = result.take(0)?;
        location.ok_or_else(|| Error::NotFound)
    }

    /// Delete a location and all its rates
    /// Delete a location
    pub async fn delete(location_id: &RecordId) -> Result<(), Error> {
        debug!("Deleting location: {}", location_id);

        // Start transaction
        DB.query("BEGIN TRANSACTION")
            .await
            .map_err(|e| Error::Database(format!("Failed to start transaction: {}", e)))?;

        // Delete all rates associated with this location
        DB.query("DELETE rate WHERE location = $location_id")
            .bind(("location_id", location_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete rates: {}", e)))?;

        // Delete the location
        DB.query("DELETE $location_id")
            .bind(("location_id", location_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete location: {}", e)))?;

        // Commit transaction
        DB.query("COMMIT TRANSACTION")
            .await
            .map_err(|e| Error::Database(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// Check if a user can edit a location
    pub async fn can_edit(location_id: &RecordId, user_id: &str) -> Result<bool, Error> {
        debug!(
            "Checking edit permission for {} on location {}",
            user_id, location_id
        );

        let query = r#"
            SELECT created_by FROM location WHERE id = $location_id
        "#;

        let mut result = DB
            .query(query)
            .bind(("location_id", location_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to check permissions: {}", e)))?;

        let location: Option<serde_json::Value> = result.take(0)?;
        if let Some(location_obj) = location {
            if let Some(created_by) = location_obj.get("created_by").and_then(|c| c.as_str()) {
                return Ok(created_by == user_id);
            }
        }
        Ok(false)
    }

    /// Get locations created by a specific user or organization
    pub async fn get_by_creator(creator_id: &str) -> Result<Vec<Location>, Error> {
        debug!("Fetching locations for creator: {}", creator_id);

        let query = r#"
            SELECT * FROM location
            WHERE created_by = $creator_id
            ORDER BY created_at DESC
        "#;

        let mut result = DB
            .query(query)
            .bind(("creator_id", creator_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch creator locations: {}", e)))?;

        let locations: Vec<Location> = result.take(0)?;
        Ok(locations)
    }

    /// Add a rate to a location
    pub async fn add_rate(
        location_id: &RecordId,
        data: CreateRateData,
    ) -> Result<LocationRate, Error> {
        debug!("Adding rate to location: {}", location_id);

        let query = r#"
            CREATE location_rate CONTENT {
                location: $location,
                rate_type: $rate_type,
                amount: $amount,
                currency: $currency,
                minimum_duration: $minimum_duration,
                description: $description
            } RETURN *;
        "#;

        let mut result = DB
            .query(query)
            .bind(("location_id", location_id.to_string()))
            .bind(("rate_type", data.rate_type))
            .bind(("amount", data.amount))
            .bind((
                "currency",
                data.currency.unwrap_or_else(|| "USD".to_string()),
            ))
            .bind(("minimum_duration", data.minimum_duration))
            .bind(("description", data.description))
            .await
            .map_err(|e| Error::Database(format!("Failed to add rate: {}", e)))?;

        let rate: Option<LocationRate> = result.take(0)?;
        rate.ok_or_else(|| Error::Database("Failed to add rate - no result returned".to_string()))
    }

    /// Get rates for a location
    pub async fn get_rates(location_id: &RecordId) -> Result<Vec<LocationRate>, Error> {
        debug!("Fetching rates for location: {}", location_id);

        let query = r#"
            SELECT * FROM location_rate
            WHERE location = $location
            ORDER BY rate_type ASC, amount ASC
        "#;

        let mut result = DB
            .query(query)
            .bind(("location_id", location_id.to_string()))
            .await
            .map_err(|e| Error::Database(format!("Failed to fetch rates: {}", e)))?;

        let rates: Vec<LocationRate> = result.take(0)?;
        Ok(rates)
    }

    /// Delete a specific rate
    pub async fn delete_rate(rate_id: &str) -> Result<(), Error> {
        debug!("Deleting rate: {}", rate_id);

        DB.query("DELETE $rate_id")
            .bind((
                "rate_id",
                Thing::from((
                    "location_rate",
                    rate_id.strip_prefix("location_rate:").unwrap_or(rate_id),
                )),
            ))
            .await
            .map_err(|e| Error::Database(format!("Failed to delete rate: {}", e)))?;

        Ok(())
    }

    /// Search public locations by keyword
    pub async fn search_public(
        keyword: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Location>, Error> {
        debug!("Searching public locations with keyword: {}", keyword);

        let query = r#"
            SELECT * FROM location
            WHERE is_public = true
            AND (
                name ~ $keyword
                OR city ~ $keyword
                OR state ~ $keyword
                OR description ~ $keyword
            )
            ORDER BY created_at DESC
            LIMIT $limit
        "#;

        let mut result = DB
            .query(query)
            .bind(("keyword", keyword.to_string()))
            .bind(("limit", limit.unwrap_or(50)))
            .await
            .map_err(|e| Error::Database(format!("Failed to search locations: {}", e)))?;

        let locations: Vec<Location> = result.take(0)?;
        Ok(locations)
    }
}
