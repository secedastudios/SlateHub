//! Test binary for debugging user database queries
//!
//! Usage: cargo run --bin test_user <username>

use slatehub::config::Config;
use slatehub::db::DB;
use slatehub::models::person::Person;
use std::env;
use surrealdb::opt::auth::Root;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    dotenv::dotenv().ok();

    // Initialize logging
    slatehub::logging::init();

    // Get username from command line
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <username>", args[0]);
        eprintln!("Example: {} chris", args[0]);
        std::process::exit(1);
    }
    let username = &args[1];

    info!("Testing user lookup for username: {}", username);

    // Load configuration
    let config = Config::from_env()?;
    debug!("Configuration loaded");

    // Connect to database
    let db_url = format!("{}:{}", config.database.host, config.database.port);
    info!("Connecting to database at: {}", db_url);
    DB.connect::<surrealdb::engine::remote::ws::Ws>(&db_url)
        .await?;
    info!("Database connection established");

    // Sign in to database
    debug!("Authenticating with database");
    DB.signin(Root {
        username: &config.database.username,
        password: &config.database.password,
    })
    .await?;
    info!("Database authentication successful");

    // Use configured namespace and database
    debug!(
        "Setting namespace: {} and database: {}",
        config.database.namespace, config.database.name
    );
    DB.use_ns(&config.database.namespace)
        .use_db(&config.database.name)
        .await?;
    info!(
        "Using namespace: {} and database: {}",
        config.database.namespace, config.database.name
    );

    // Test 1: Direct SQL query with lowercase
    info!("\n=== Test 1: Direct SQL query with string::lowercase ===");
    let sql = "SELECT * FROM person WHERE username = string::lowercase($username)";
    info!("Query: {}", sql);
    info!("Parameter: username = {}", username);

    let mut response = DB
        .query(sql)
        .bind(("username", username.to_string()))
        .await?;

    // Try to deserialize as Person structs
    match response.take::<Vec<Person>>(0) {
        Ok(persons) => {
            info!(
                "✅ Successfully deserialized {} Person records",
                persons.len()
            );
            for (i, person) in persons.iter().enumerate() {
                info!(
                    "Person {}: id={}, username={}, email={}",
                    i, person.id, person.username, person.email
                );
            }
        }
        Err(e) => {
            error!("❌ Failed to deserialize as Person: {}", e);
            // Try to get raw values instead
            info!("Attempting to get raw values...");
        }
    }

    // Test 2: Direct SQL query without lowercase
    info!("\n=== Test 2: Direct SQL query WITHOUT string::lowercase ===");
    let sql2 = "SELECT * FROM person WHERE username = $username";
    info!("Query: {}", sql2);
    info!("Parameter: username = {}", username);

    let mut response2 = DB
        .query(sql2)
        .bind(("username", username.to_lowercase()))
        .await?;

    // Try to deserialize as Person structs
    match response2.take::<Vec<Person>>(0) {
        Ok(persons) => {
            info!(
                "✅ Successfully deserialized {} Person records",
                persons.len()
            );
            for (i, person) in persons.iter().enumerate() {
                info!(
                    "Person {}: id={}, username={}, email={}",
                    i, person.id, person.username, person.email
                );
            }
        }
        Err(e) => {
            error!("❌ Failed to deserialize as Person: {}", e);
        }
    }

    // Test 3: Using Person::find_by_username
    info!("\n=== Test 3: Using Person::find_by_username ===");
    match Person::find_by_username(username).await {
        Ok(Some(person)) => {
            info!("✅ Found user!");
            info!("  ID: {}", person.id);
            info!("  Username: {}", person.username);
            info!("  Email: {}", person.email);
            if let Some(profile) = &person.profile {
                if let Some(name) = &profile.name {
                    info!("  Name: {}", name);
                }
            }
        }
        Ok(None) => {
            error!("❌ No user found with username: {}", username);
        }
        Err(e) => {
            error!("❌ Error finding user: {}", e);
        }
    }

    // Test 4: List all users to verify data exists
    info!("\n=== Test 4: List all users in database ===");
    let sql3 = "SELECT * FROM person";
    let mut response3 = DB.query(sql3).await?;

    // Try full Person deserialization first
    match response3.take::<Vec<Person>>(0) {
        Ok(persons) => {
            info!("Total users in database: {}", persons.len());
            for (i, person) in persons.iter().enumerate() {
                info!(
                    "User {}: id={}, username={}, email={}",
                    i, person.id, person.username, person.email
                );
            }
        }
        Err(e) => {
            error!("Failed to deserialize Person records: {}", e);
            info!("This might indicate a schema mismatch between Person struct and database");
        }
    }

    // Test 5: Test with both lowercase and original
    info!("\n=== Test 5: Comparison test ===");
    info!("Original username: '{}'", username);
    info!("Lowercase username: '{}'", username.to_lowercase());

    // Try exact match with lowercase
    let sql4 = "SELECT id, username, email FROM person WHERE username = $username";
    let mut response4 = DB
        .query(sql4)
        .bind(("username", username.to_lowercase()))
        .await?;

    match response4.take::<Vec<Person>>(0) {
        Ok(persons) => {
            info!("Exact match with lowercase: {} results", persons.len());
            if !persons.is_empty() {
                info!("✅ Found user with exact lowercase match!");
            }
        }
        Err(e) => {
            error!("Failed to deserialize: {}", e);
        }
    }

    // Test 6: Debug - Check if profile field is causing issues
    info!("\n=== Test 6: Query without profile field ===");
    let sql5 =
        "SELECT id, username, email FROM person WHERE username = string::lowercase($username)";
    let mut response5 = DB
        .query(sql5)
        .bind(("username", username.to_string()))
        .await?;

    // Define a simple struct for basic fields
    #[derive(Debug, serde::Deserialize)]
    struct BasicPerson {
        id: surrealdb::RecordId,
        username: String,
        email: String,
    }

    match response5.take::<Vec<BasicPerson>>(0) {
        Ok(persons) => {
            info!("✅ Basic query successful: {} results", persons.len());
            for person in persons {
                info!("  Found: {:?}", person);
            }
        }
        Err(e) => {
            error!("❌ Even basic query failed: {}", e);
        }
    }

    Ok(())
}
