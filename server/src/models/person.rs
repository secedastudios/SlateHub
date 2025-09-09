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

    /// Finds a person by their username.
    ///
    /// # Arguments
    /// * `username` - The username to search for.
    ///
    /// # Returns
    /// A `Result` containing an `Option<Person>`. Returns `Some(Person)` if found,
    /// `None` if not found, or an `Error` if the database operation fails.
    pub async fn find_by_username(username: &str) -> Result<Option<Self>> {
        let sql = "SELECT * FROM person WHERE username = $username";
        let mut response = DB
            .query(sql)
            .bind(("username", username.to_string()))
            .await?;

        let persons: Vec<Person> = response.take(0)?;
        Ok(persons.into_iter().next())
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
        let sql = "SELECT * FROM person WHERE username = $identifier OR email = $identifier";
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
        let sql = "SELECT * FROM person WHERE (username = $identifier OR email = $identifier) AND crypto::argon2::compare(password, $password)";
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
}

impl Person {
    /// Signs up a new user using SurrealDB's authentication system.
    /// This creates a new person record with hashed password.
    ///
    /// # Arguments
    /// * `username` - The username for the new user
    /// * `email` - The email for the new user
    /// * `password` - The password to be hashed and stored
    ///
    /// # Returns
    /// A `Result` containing the JWT token string if successful.
    pub async fn signup(username: String, email: String, password: String) -> Result<String> {
        use crate::db::DB;
        use std::collections::HashMap;
        use std::env;
        use surrealdb::opt::auth::Record;

        // Get namespace and database from environment
        let namespace = env::var("DB_NAMESPACE").unwrap_or_else(|_| "slatehub".to_string());
        let database = env::var("DB_NAME").unwrap_or_else(|_| "main".to_string());

        let mut vars = HashMap::new();
        vars.insert("user".to_string(), username);
        vars.insert("email".to_string(), email);
        vars.insert("pass".to_string(), password);

        match DB
            .signup(Record {
                namespace: &namespace,
                database: &database,
                access: "user",
                params: vars,
            })
            .await
        {
            Ok(jwt) => Ok(jwt.into_insecure_token()),
            Err(e) => {
                log_error!(e, "Failed to signup user");
                Err(e.into())
            }
        }
    }

    /// Signs in a user using SurrealDB's authentication system.
    ///
    /// # Arguments
    /// * `identifier` - Username or email
    /// * `password` - The password to verify
    ///
    /// # Returns
    /// A `Result` containing the JWT token string if successful.
    pub async fn signin(identifier: String, password: String) -> Result<String> {
        use crate::db::DB;
        use std::collections::HashMap;
        use std::env;
        use surrealdb::opt::auth::Record;

        // Get namespace and database from environment
        let namespace = env::var("DB_NAMESPACE").unwrap_or_else(|_| "slatehub".to_string());
        let database = env::var("DB_NAME").unwrap_or_else(|_| "main".to_string());

        let mut params = HashMap::new();
        params.insert("user".to_string(), identifier);
        params.insert("pass".to_string(), password);

        match DB
            .signin(Record {
                namespace: &namespace,
                database: &database,
                access: "user",
                params,
            })
            .await
        {
            Ok(jwt) => Ok(jwt.into_insecure_token()),
            Err(e) => {
                log_error!(e, "Failed to signin user");
                Err(e.into())
            }
        }
    }

    /// Invalidates the current authentication session.
    /// Used for logout functionality.
    ///
    /// # Returns
    /// A `Result` indicating success or failure.
    pub async fn invalidate_session() -> Result<()> {
        use crate::db::DB;

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
    /// # Arguments
    /// * `token` - The JWT token to authenticate with
    ///
    /// # Returns
    /// A `Result` indicating success or failure.
    pub async fn authenticate_token(token: &str) -> Result<()> {
        use crate::db::DB;

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
    pub user: String, // Can be username or email
    pub pass: String,
}
