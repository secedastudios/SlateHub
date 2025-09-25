//! `Person` model and related database operations.
//!
//! This module defines the data structures for a `Person` and their detailed `Profile`,
//! mirroring the `person` table in the SurrealDB schema. It provides functions to
//! interact with the database for creating, retrieving, updating, and deleting person records.

use crate::auth;
use crate::db::DB;
use crate::error::{Error, Result};
use crate::{db_span, log_error};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use tracing::{debug, error, info};

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
    /// The person's optional display name.
    pub name: Option<String>,
    /// The verification status of the person (unverified, email, sms, identity).
    #[serde(default = "default_verification_status")]
    pub verification_status: String,
    /// The detailed profile information for the person.
    #[serde(default)]
    pub profile: Option<Profile>,
}

fn default_verification_status() -> String {
    "unverified".to_string()
}

/// Represents the detailed profile of a person.
/// Corresponds to the flexible `profile` object in the `person` table.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    pub name: Option<String>,
    pub avatar: Option<String>, // Direct URL to profile image
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

    /// Finds a person by their username.
    ///
    /// # Arguments
    /// * `username` - The username to search for.
    ///
    /// # Returns
    /// A `Result` containing an `Option<Person>`. Returns `Some(Person)` if found,
    /// `None` if not found, or an `Error` if the database operation fails.
    pub async fn find_by_username(username: &str) -> Result<Option<Self>> {
        use tracing::debug;

        let sql = "SELECT * FROM person WHERE username = string::lowercase($username)";
        debug!("Executing query: {} with username: '{}'", sql, username);

        let mut response = DB
            .query(sql)
            .bind(("username", username.to_string()))
            .await?;

        debug!(
            "Query executed successfully, attempting to extract results: {:?}",
            response
        );

        // Try to get the raw response first to see what we're getting
        let persons: Vec<Person> = match response.take::<Vec<Person>>(0) {
            Ok(p) => {
                debug!("Successfully extracted {} person records", p.len());
                p
            }
            Err(e) => {
                debug!("Failed to extract person records: {:?}", e);
                return Err(e.into());
            }
        };

        let result = persons.into_iter().next();
        debug!("Returning result: {:?}", result.is_some());
        Ok(result)
    }

    /// Finds a person by their ID.
    ///
    /// # Arguments
    /// * `id` - The ID to search for (can be with or without "person:" prefix).
    ///
    /// # Returns
    /// A `Result` containing an `Option<Person>`. Returns `Some(Person)` if found,
    /// `None` if not found, or an `Error` if the database operation fails.
    pub async fn find_by_id(id: &str) -> Result<Option<Self>> {
        use tracing::{debug, info_span};

        let span = info_span!(
            "find_person_by_id",
            id = %id,
            record_id = tracing::field::Empty,
        );
        let _enter = span.enter();

        // Build the full record ID if needed
        let record_id = if id.starts_with("person:") {
            id.to_string()
        } else {
            format!("person:{}", id)
        };

        span.record("record_id", &record_id);
        debug!(
            record_id = %record_id,
            "Using DB.select to fetch person"
        );

        // Query directly using the record ID
        // In SurrealDB, we can query a specific record directly
        let sql = format!("SELECT * FROM {}", record_id);
        debug!(
            sql = %sql,
            "Executing direct record query"
        );

        let mut response = DB.query(&sql).await?;

        debug!("Query executed, extracting results");

        // Extract the person from the response
        let persons: Vec<Person> = match response.take::<Vec<Person>>(0) {
            Ok(p) => {
                debug!(count = p.len(), "Extracted person records");
                p
            }
            Err(e) => {
                debug!(error = ?e, "Failed to extract person records");
                return Err(e.into());
            }
        };

        let result = persons.into_iter().next();
        debug!(found = result.is_some(), "Query complete");
        Ok(result)
    }

    /// Finds a person by their email.
    ///
    /// # Arguments
    /// * `email` - The email to search for.
    ///
    /// # Returns
    /// A `Result` containing an `Option<Person>`. Returns `Some(Person)` if found,
    /// `None` if not found, or an `Error` if the database operation fails.
    pub async fn find_by_email(email: &str) -> Result<Option<Self>> {
        let sql = "SELECT * FROM person WHERE email = $email";
        let mut response = DB.query(sql).bind(("email", email.to_string())).await?;

        let persons: Vec<Person> = response.take(0)?;
        Ok(persons.into_iter().next())
    }

    /// Finds a person by either username or email.
    /// Used for login where user can provide either.
    ///
    /// # Arguments
    /// * `identifier` - Can be either a username or email.
    ///
    /// # Returns
    /// A `Result` containing an `Option<Person>`. Returns `Some(Person)` if found,
    /// `None` if not found, or an `Error` if the database operation fails.
    pub async fn find_by_identifier(identifier: &str) -> Result<Option<Self>> {
        let sql = "SELECT * FROM person WHERE username = string::lowercase($identifier) OR email = $identifier";
        let mut response = DB
            .query(sql)
            .bind(("identifier", identifier.to_string()))
            .await?;

        let persons: Vec<Person> = response.take(0)?;
        Ok(persons.into_iter().next())
    }

    /// Authenticates a user with username/email and password.
    /// This is used after the JWT token validation to verify the user still exists.
    ///
    /// # Arguments
    /// * `identifier` - Username or email
    /// * `password` - The password to verify
    ///
    /// # Returns
    /// A `Result` containing an `Option<Person>` if authentication succeeds.
    pub async fn authenticate(identifier: &str, password: &str) -> Result<Option<Self>> {
        let sql = "SELECT * FROM person WHERE (username = string::lowercase($identifier) OR email = $identifier) AND crypto::argon2::compare(password, $password)";
        let mut response = DB
            .query(sql)
            .bind(("identifier", identifier.to_string()))
            .bind(("password", password.to_string()))
            .await?;

        let persons: Vec<Person> = response.take(0)?;
        Ok(persons.into_iter().next())
    }

    /// Retrieves all persons from the database.
    /// Use with caution on large datasets.
    ///
    /// # Returns
    /// A `Result` containing a `Vec<Person>` with all person records.
    pub async fn get_all() -> Result<Vec<Self>> {
        let sql = "SELECT * FROM person";
        let mut response = DB.query(sql).await?;
        let persons: Vec<Person> = response.take(0)?;
        Ok(persons)
    }

    /// Retrieves persons with pagination.
    ///
    /// # Arguments
    /// * `limit` - Maximum number of records to return
    /// * `offset` - Number of records to skip
    ///
    /// # Returns
    /// A `Result` containing a `Vec<Person>` with the requested page of records.
    pub async fn get_paginated(limit: usize, offset: usize) -> Result<Vec<Self>> {
        let sql = "SELECT * FROM person LIMIT $limit START $offset";
        let mut response = DB
            .query(sql)
            .bind(("limit", limit))
            .bind(("offset", offset))
            .await?;

        let persons: Vec<Person> = response.take(0)?;
        Ok(persons)
    }

    /// Searches for persons by skill.
    ///
    /// # Arguments
    /// * `skill` - The skill to search for
    ///
    /// # Returns
    /// A `Result` containing a `Vec<Person>` with matching records.
    pub async fn find_by_skill(skill: &str) -> Result<Vec<Self>> {
        let sql = "SELECT * FROM person WHERE profile.skills CONTAINS $skill";
        let mut response = DB.query(sql).bind(("skill", skill.to_string())).await?;

        let persons: Vec<Person> = response.take(0)?;
        Ok(persons)
    }

    /// Searches for persons by location.
    ///
    /// # Arguments
    /// * `location` - The location to search for
    ///
    /// # Returns
    /// A `Result` containing a `Vec<Person>` with matching records.
    pub async fn find_by_location(location: &str) -> Result<Vec<Self>> {
        let sql = "SELECT * FROM person WHERE profile.location CONTAINS $location";
        let mut response = DB
            .query(sql)
            .bind(("location", location.to_string()))
            .await?;

        let persons: Vec<Person> = response.take(0)?;
        Ok(persons)
    }

    /// Creates a simplified version of the Person for session/auth purposes.
    /// This excludes sensitive data like password and detailed profile info.
    pub fn to_session_user(&self) -> SessionUser {
        SessionUser {
            id: self.id.to_string(),
            username: self.username.clone(),
            email: self.email.clone(),
            name: self
                .profile
                .as_ref()
                .and_then(|p| p.name.clone())
                .unwrap_or_else(|| self.username.clone()),
        }
    }

    /// Get initials for display (for avatar fallback)
    pub fn get_initials(&self) -> String {
        let display_name = self
            .name
            .clone()
            .or_else(|| self.profile.as_ref().and_then(|p| p.name.clone()))
            .unwrap_or_else(|| self.username.clone());

        let parts: Vec<&str> = display_name.split_whitespace().collect();

        if parts.len() >= 2 {
            // Use first letter of first and last name
            let first = parts[0].chars().next().unwrap_or('?');
            let last = parts[parts.len() - 1].chars().next().unwrap_or('?');
            format!("{}{}", first, last).to_uppercase()
        } else if !parts.is_empty() {
            // Use first two letters of single name
            let chars: Vec<char> = parts[0].chars().take(2).collect();
            if chars.len() == 2 {
                format!("{}{}", chars[0], chars[1]).to_uppercase()
            } else if chars.len() == 1 {
                format!("{}", chars[0]).to_uppercase()
            } else {
                "??".to_string()
            }
        } else {
            "??".to_string()
        }
    }

    /// Get the avatar URL if one exists
    pub fn get_avatar_url(&self) -> Option<String> {
        self.profile.as_ref().and_then(|p| p.avatar.clone())
    }

    /// Get the display name for the person
    /// Priority: person.name -> profile.name -> username
    pub fn get_display_name(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.profile.as_ref().and_then(|p| p.name.clone()))
            .unwrap_or_else(|| self.username.clone())
    }

    /// Updates a user's profile information.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user to update
    /// * `name` - Optional new display name
    /// * `headline` - Optional new headline
    /// * `bio` - Optional new bio
    /// * `location` - Optional new location
    /// * `website` - Optional new website
    /// * `skills` - Optional comma-separated list of skills
    /// * `languages` - Optional comma-separated list of languages
    /// * `availability` - Optional availability status
    ///
    /// # Returns
    /// * `Result<Option<Person>>` - The updated person record if successful
    pub async fn update_profile(
        user_id: &str,
        name: Option<String>,
        headline: Option<String>,
        bio: Option<String>,
        location: Option<String>,
        website: Option<String>,
        skills: Option<String>,
        languages: Option<String>,
        availability: Option<String>,
    ) -> Result<Option<Self>> {
        let _span = db_span!("Person::update_profile", user_id);

        // Security: Only update the profile for the authenticated user's ID
        // The user_id parameter should always be the authenticated user's ID from the middleware
        // First, fetch the existing person record for this specific user
        let mut person = match Self::find_by_id(user_id).await? {
            Some(p) => {
                // Verify ownership: The fetched person's ID must match the requested user_id
                // This is implicit since find_by_id already filters by user_id, but we can be explicit
                if p.id.to_string() != user_id && !user_id.starts_with("person:") {
                    // Also check if user_id is missing the "person:" prefix
                    let full_id = format!("person:{}", user_id);
                    if p.id.to_string() != full_id {
                        error!(
                            "User {} attempted to update profile for different user",
                            user_id
                        );
                        return Err(Error::Forbidden);
                    }
                }
                p
            }
            None => {
                debug!("Person not found for update: {}", user_id);
                return Ok(None);
            }
        };

        // Initialize profile if it doesn't exist
        if person.profile.is_none() {
            person.profile = Some(Profile {
                name: None, // Explicitly set to None to avoid database errors
                avatar: None,
                headline: None,
                bio: None,
                location: None,
                website: None,
                phone: None,
                is_public: false,
                height_mm: None,
                weight_kg: None,
                body_type: None,
                hair_color: None,
                eye_color: None,
                gender: None,
                ethnicity: Vec::new(),
                age_range: None,
                skills: Vec::new(),
                unions: Vec::new(),
                languages: Vec::new(),
                availability: None,
                experience: Vec::new(),
                education: Vec::new(),
                awards: Vec::new(),
                reels: Vec::new(),
                media_other: Vec::new(),
                resume: None,
                social_links: Vec::new(),
            });
        }

        // Update the person name field if provided (at root level, not in profile)
        if let Some(n) = name.clone() {
            person.name = if n.is_empty() { None } else { Some(n) };
        }

        // Update the profile fields if provided
        if let Some(profile) = &mut person.profile {
            // Keep profile.name synchronized with person.name for backward compatibility
            if let Some(n) = name {
                profile.name = if n.is_empty() { None } else { Some(n) };
            }
            if let Some(h) = headline {
                profile.headline = if h.is_empty() { None } else { Some(h) };
            }
            if let Some(b) = bio {
                profile.bio = if b.is_empty() { None } else { Some(b) };
            }
            if let Some(l) = location {
                profile.location = if l.is_empty() { None } else { Some(l) };
            }
            if let Some(w) = website {
                profile.website = if w.is_empty() { None } else { Some(w) };
            }
            if let Some(s) = skills {
                profile.skills = if s.is_empty() {
                    Vec::new()
                } else {
                    s.split(',')
                        .map(|skill| skill.trim().to_string())
                        .filter(|skill| !skill.is_empty())
                        .collect()
                };
            }
            if let Some(l) = languages {
                profile.languages = if l.is_empty() {
                    Vec::new()
                } else {
                    l.split(',')
                        .map(|lang| lang.trim().to_string())
                        .filter(|lang| !lang.is_empty())
                        .collect()
                };
            }
            if let Some(a) = availability {
                profile.availability = if a.is_empty() { None } else { Some(a) };
            }
        }

        // Update the name and profile fields in the database
        // Use MERGE to update just these fields without affecting other fields like password
        let query = "UPDATE $id MERGE { name: $name, profile: $profile } RETURN AFTER";
        let mut response = DB
            .query(query)
            .bind(("id", person.id.clone()))
            .bind(("name", person.name.clone()))
            .bind(("profile", person.profile.clone()))
            .await
            .map_err(|e| {
                log_error!(e, "Failed to update person profile");
                Error::from(e)
            })?;

        let updated: Option<Person> = response.take(0).map_err(|e| {
            log_error!(e, "Failed to extract updated person from response");
            Error::from(e)
        })?;

        Ok(updated)
    }
}

impl Person {
    /// Signs up a new user by creating a person record with hashed password.
    ///
    /// # Arguments
    /// * `username` - The username for the new user
    /// * `email` - The email for the new user
    /// * `password` - The password to be hashed and stored
    ///
    /// # Returns
    /// A `Result` containing the JWT token string if successful.
    pub async fn signup(username: String, email: String, password: String) -> Result<String> {
        use crate::auth;
        use crate::db::DB;
        use tracing::debug;

        // Hash the password using SurrealDB-compatible Argon2id
        let password_hash = auth::hash_password(&password)?;

        // Check if user already exists
        if Self::find_by_username(&username).await?.is_some() {
            return Err(Error::Conflict("Username already exists".to_string()));
        }
        if Self::find_by_email(&email).await?.is_some() {
            return Err(Error::Conflict("Email already exists".to_string()));
        }

        // Create the person record with unverified status
        let sql = "CREATE person SET username = $username, email = $email, password = $password, name = $name, verification_status = $verification_status";
        let mut response = DB
            .query(sql)
            .bind(("username", username.clone()))
            .bind(("email", email.clone()))
            .bind(("password", password_hash))
            .bind(("name", None::<String>))
            .bind(("verification_status", "unverified"))
            .await?;

        // Get the created person
        let persons: Vec<Person> = response.take(0)?;
        let person = persons
            .into_iter()
            .next()
            .ok_or_else(|| Error::Internal("Failed to create user".to_string()))?;

        debug!("Created new user: {} with id: {}", username, person.id);

        // Generate verification code and send email
        use crate::services::email::EmailService;
        use crate::services::verification::{CodeType, VerificationService};

        // Generate verification code
        let verification_code =
            VerificationService::create_verification_code(&person.id, CodeType::EmailVerification)
                .await
                .map_err(|e| {
                    Error::Internal(format!("Failed to create verification code: {}", e))
                })?;

        // Send verification email (non-blocking, log error if it fails)
        if let Ok(email_service) = EmailService::from_env() {
            let email_clone = email.clone();
            tokio::spawn(async move {
                if let Err(e) = email_service
                    .send_verification_email(&email_clone, None, &verification_code)
                    .await
                {
                    error!(
                        "Failed to send verification email to {}: {}",
                        email_clone, e
                    );
                } else {
                    info!("Verification email sent to {}", email_clone);
                }
            });
        } else {
            error!("Email service not configured - skipping verification email");
        }

        // Generate JWT token
        let token = auth::create_jwt(&person.id.to_string(), &username, &email)?;

        Ok(token)
    }

    /// Signs in a user by verifying their password.
    ///
    /// # Arguments
    /// * `identifier` - Username
    /// * `password` - The password to verify
    ///
    /// # Returns
    /// A `Result` containing the JWT token string if successful.
    pub async fn signin(identifier: String, password: String) -> Result<String> {
        // Find the user by username or email, including the password field
        // Note: password field must be explicitly requested in SurrealDB
        let sql = "SELECT *, password FROM person WHERE username = string::lowercase($identifier)";
        let mut response = DB
            .query(sql)
            .bind(("identifier", identifier.clone()))
            .await?;

        // Define a struct that includes the password field
        #[derive(serde::Deserialize)]
        struct PersonWithPassword {
            id: surrealdb::RecordId,
            username: String,
            email: String,
            password: String,
            verification_status: String,
        }

        let persons: Vec<PersonWithPassword> = response.take(0)?;
        let person_with_password = persons
            .into_iter()
            .next()
            .ok_or_else(|| Error::Unauthorized)?;

        // Verify the password
        if !auth::verify_password(&password, &person_with_password.password)? {
            debug!("Invalid password for user: {}", identifier);
            return Err(Error::Unauthorized);
        }

        // Check email verification status
        if person_with_password.verification_status == "unverified" {
            debug!("User email not verified: {}", identifier);
            return Err(Error::Validation(
                "Your email address has not been verified. Please check your email for the verification code.".to_string()
            ));
        }

        debug!(
            "User authenticated successfully: {}",
            person_with_password.username
        );

        // Generate JWT token
        let token = auth::create_jwt(
            &person_with_password.id.to_string(),
            &person_with_password.username,
            &person_with_password.email,
        )?;

        Ok(token)
    }

    /// Invalidates the current authentication session.
    /// Used for logout functionality.
    ///
    /// **WARNING**: This method changes the global DB connection context!
    /// Do not use with a singleton DB connection. It will invalidate the root
    /// authentication and break all subsequent database queries.
    ///
    /// # Deprecated
    /// This method should not be used when using a singleton root DB connection.
    /// Session management should be handled via JWT cookies instead.
    ///
    /// # Returns
    /// A `Result` indicating success or failure.
    #[deprecated(note = "Changes global DB connection context. Use cookie-based sessions instead.")]
    pub async fn invalidate_session() -> Result<()> {
        match DB.invalidate().await {
            Ok(_) => Ok(()),
            Err(e) => {
                log_error!(e, "Failed to invalidate session");
                Err(e.into())
            }
        }
    }

    /// Authenticates with an existing JWT token.
    /// Used to validate tokens from cookies.
    ///
    /// **WARNING**: This method changes the global DB connection context!
    /// Do not use with a singleton DB connection. It will change the authentication
    /// from root to the user context, and users don't have permissions to query
    /// the person table.
    ///
    /// # Deprecated
    /// This method should not be used when using a singleton root DB connection.
    /// JWT validation should be done by decoding the token instead.
    ///
    /// # Arguments
    /// * `token` - The JWT token to authenticate with
    ///
    /// # Returns
    /// A `Result` indicating success or failure.
    #[deprecated(note = "Changes global DB connection context. Decode JWT instead.")]
    pub async fn authenticate_token(token: &str) -> Result<()> {
        match DB.authenticate(token).await {
            Ok(_) => Ok(()),
            Err(e) => {
                log_error!(e, "Failed to authenticate token");
                Err(e.into())
            }
        }
    }
}

// -----------------------------------------------------------------------------
// API Data Structures
// -----------------------------------------------------------------------------

/// Simplified user representation for session/authentication purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUser {
    pub id: String,
    pub username: String,
    pub email: String,
    pub name: String,
}

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
    pub email: String,
    pub password: String,
    pub redirect_to: Option<String>,
}
