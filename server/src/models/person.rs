//! `Person` model and related database operations.
//!
//! This module defines the data structures for a `Person` and their detailed `Profile`,
//! mirroring the `person` table in the SurrealDB schema. It provides functions to
//! interact with the database for creating, retrieving, updating, and deleting person records.

use crate::db::DB;
use crate::error::Result;
use crate::{db_span, log_error};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;

// -----------------------------------------------------------------------------
// Core Person Model
// -----------------------------------------------------------------------------

/// Represents a person record in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    /// The unique identifier for the person, represented as a SurrealDB `RecordId`.
    pub id: RecordId,
    /// The person's unique username.
    pub username: String,
    /// The person's unique email address.
    pub email: String,
    /// The detailed profile information for the person.
    #[serde(default)]
    pub profile: Option<Profile>,
}

/// Represents the detailed profile of a person.
/// Corresponds to the flexible `profile` object in the `person` table.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    pub name: Option<String>,
    pub avatar: Option<RecordId>, // Record link to 'media' table
    pub headline: Option<String>,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub phone: Option<String>,
    pub is_public: bool,

    // Physical Attributes
    pub height_mm: Option<i32>,
    pub weight_kg: Option<i32>,
    pub body_type: Option<String>,
    pub hair_color: Option<String>,
    pub eye_color: Option<String>,
    pub gender: Option<String>,
    pub ethnicity: Vec<String>,
    pub age_range: Option<AgeRange>,

    // Professional Details
    pub skills: Vec<String>,
    pub unions: Vec<String>,
    pub languages: Vec<String>,
    pub availability: Option<String>,
    pub experience: Vec<Experience>,
    pub education: Vec<Education>,
    pub awards: Vec<Award>,

    // Media
    pub reels: Vec<RecordId>,       // Record links to 'media' table
    pub media_other: Vec<RecordId>, // Record links to 'media' table
    pub resume: Option<RecordId>,   // Record link to 'media' table
    pub social_links: Vec<SocialLink>,
}

// -----------------------------------------------------------------------------
// Nested Profile Structs
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgeRange {
    pub min: i32,
    pub max: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLink {
    pub platform: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub role: String,
    pub production: Option<String>,
    pub description: Option<String>,
    pub dates: Option<DateRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Education {
    pub institution: String,
    pub degree: Option<String>,
    pub field: Option<String>,
    pub dates: Option<DateRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Award {
    pub name: String,
    pub year: i32,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: Option<String>,
    pub end: Option<String>,
}

// -----------------------------------------------------------------------------
// Database Implementations
// -----------------------------------------------------------------------------

impl Person {
    /// Retrieves a single person by their ID from the database.
    ///
    /// # Arguments
    /// * `id` - The `RecordId` representing the person's unique ID.
    ///
    /// # Returns
    /// A `Result` containing an `Option<Person>`. Returns `Some(Person)` if found,
    /// `None` if not found, or an `Error` if the database operation fails.
    pub async fn get(id: &RecordId) -> Result<Option<Self>> {
        let _span = db_span!("Person::get", id.to_string()).entered();
        match DB.select(id).await {
            Ok(person) => Ok(person),
            Err(e) => {
                log_error!(e, "Failed to get person");
                Err(e.into())
            }
        }
    }

    /// Updates the current person's record in the database.
    ///
    /// # Returns
    /// A `Result` containing the updated `Person` record.
    /// Returns an `Error` if the update operation fails.
    pub async fn update(&self) -> Result<Option<Self>> {
        let _span = db_span!("Person::update", self.id.to_string()).entered();
        match DB.update(&self.id).content(self.clone()).await {
            Ok(person) => Ok(person),
            Err(e) => {
                log_error!(e, "Failed to update person");
                Err(e.into())
            }
        }
    }

    /// Deletes the current person's record from the database.
    ///
    /// # Returns
    /// A `Result` containing the deleted `Person` record.
    /// Returns an `Error` if the deletion fails.
    pub async fn delete(&self) -> Result<Option<Self>> {
        let _span = db_span!("Person::delete", self.id.to_string()).entered();
        match DB.delete(&self.id).await {
            Ok(person) => Ok(person),
            Err(e) => {
                log_error!(e, "Failed to delete person");
                Err(e.into())
            }
        }
    }
}

// -----------------------------------------------------------------------------
// API Data Structures
// -----------------------------------------------------------------------------

/// Represents the data required to create a new user account.
/// Used for deserializing the registration form data.
#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub email: String,
    pub password: String,
}

/// Represents the data required for a user to log in.
/// Used for deserializing the login form data.
#[derive(Debug, Deserialize)]
pub struct LoginUser {
    pub user: String, // Can be username or email
    pub pass: String,
}
