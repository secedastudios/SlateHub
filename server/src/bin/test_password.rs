//! Test binary for verifying password compatibility with existing SurrealDB passwords
//!
//! Usage: cargo run --bin test_password <username> <password>

use slatehub::auth;
use slatehub::config::Config;
use slatehub::db::DB;
use surrealdb::opt::auth::Root;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    dotenv::dotenv().ok();

    // Initialize logging
    slatehub::logging::init();

    // Get arguments from command line
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <username> <password>", args[0]);
        eprintln!("Example: {} chris mypassword", args[0]);
        std::process::exit(1);
    }
    let username = &args[1];
    let password = &args[2];

    info!("Testing password verification for username: {}", username);

    // Load configuration
    let config = Config::from_env()?;
    debug!("Configuration loaded");

    // Connect to database
    let db_url = format!("{}:{}", config.database.host, config.database.port);
    info!("Connecting to database at: {}", db_url);
    DB.connect::<surrealdb::engine::remote::ws::Ws>(&db_url)
        .await?;
    info!("Database connection established");

    // Sign in to database as root
    debug!("Authenticating with database as root");
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

    // Test 1: Fetch the password hash from database
    info!("\n=== Test 1: Fetch password hash from database ===");
    let sql = "SELECT id, username, email, password FROM person WHERE username = string::lowercase($username)";
    info!("Query: {}", sql);
    info!("Parameter: username = {}", username);

    let mut response = DB
        .query(sql)
        .bind(("username", username.to_string()))
        .await?;

    #[derive(Debug, serde::Deserialize)]
    struct UserRecord {
        id: surrealdb::RecordId,
        username: String,
        email: String,
        password: String,
    }

    let users: Vec<UserRecord> = response.take(0)?;

    if users.is_empty() {
        error!("❌ No user found with username: {}", username);
        std::process::exit(1);
    }

    let user = &users[0];
    info!("✅ Found user:");
    info!("  ID: {}", user.id);
    info!("  Username: {}", user.username);
    info!("  Email: {}", user.email);
    info!("  Password hash: {}", user.password);

    // Test 2: Analyze the password hash format
    info!("\n=== Test 2: Analyze password hash format ===");
    if user.password.starts_with("$argon2id$") {
        info!("✅ Password uses Argon2id algorithm (SurrealDB standard)");

        // Parse the parameters from the hash
        if let Some(params_match) = user.password.split('$').nth(3) {
            info!("  Parameters: {}", params_match);
            // Should be something like "m=19456,t=2,p=1"
        }
    } else {
        error!("❌ Unexpected password hash format");
    }

    // Test 3: Verify the password using our auth module
    info!("\n=== Test 3: Verify password using our auth module ===");
    info!("Testing password: '{}'", password);

    match auth::verify_password(password, &user.password) {
        Ok(true) => {
            info!("✅ Password verification SUCCESSFUL!");
            info!("Our Argon2 implementation is compatible with SurrealDB!");
        }
        Ok(false) => {
            error!("❌ Password verification FAILED!");
            error!("The password is incorrect.");
        }
        Err(e) => {
            error!("❌ Error during password verification: {}", e);
            error!("This might indicate incompatible hash formats.");
        }
    }

    // Test 4: Test creating a new hash and verifying it
    info!("\n=== Test 4: Test our password hashing ===");
    let test_password = "test123";
    info!("Creating hash for test password: '{}'", test_password);

    match auth::hash_password(test_password) {
        Ok(hash) => {
            info!("✅ Generated hash: {}", hash);

            // Verify it matches SurrealDB format
            if hash.starts_with("$argon2id$") && hash.contains("$m=19456,t=2,p=1$") {
                info!("✅ Hash format matches SurrealDB exactly!");
            } else {
                error!("⚠️  Hash format differs from SurrealDB standard");
            }

            // Verify we can verify our own hash
            match auth::verify_password(test_password, &hash) {
                Ok(true) => info!("✅ Self-verification successful"),
                Ok(false) => error!("❌ Self-verification failed"),
                Err(e) => error!("❌ Self-verification error: {}", e),
            }
        }
        Err(e) => {
            error!("❌ Failed to create hash: {}", e);
        }
    }

    // Test 5: Test with Person::signin method
    info!("\n=== Test 5: Test Person::signin method ===");
    use slatehub::models::person::Person;

    match Person::signin(username.to_string(), password.to_string()).await {
        Ok(token) => {
            info!("✅ Person::signin successful!");
            info!(
                "  JWT token (first 50 chars): {}...",
                &token[..50.min(token.len())]
            );

            // Try to decode the JWT
            match auth::decode_jwt(&token) {
                Ok(claims) => {
                    info!("✅ JWT decoded successfully:");
                    info!("  User ID: {}", claims.sub);
                    info!("  Username: {}", claims.username);
                    info!("  Email: {}", claims.email);
                }
                Err(e) => {
                    error!("❌ Failed to decode JWT: {}", e);
                }
            }
        }
        Err(e) => {
            error!("❌ Person::signin failed: {}", e);
        }
    }

    Ok(())
}
