use once_cell::sync::Lazy;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws};
use tokio::sync::Mutex;

pub static TEST_DB: Lazy<Arc<Mutex<Option<Surreal<Client>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Test database configuration
pub struct TestConfig {
    pub db_url: String,
    pub db_user: String,
    pub db_pass: String,
    pub db_ns: String,
    pub db_name: String,
    pub minio_endpoint: String,
    pub minio_access_key: String,
    pub minio_secret_key: String,
    pub minio_bucket: String,
}

impl Default for TestConfig {
    fn default() -> Self {
        // Load from environment or use test defaults
        Self {
            db_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "ws://localhost:8100/rpc".to_string()),
            db_user: std::env::var("DATABASE_USER").unwrap_or_else(|_| "root".to_string()),
            db_pass: std::env::var("DATABASE_PASS").unwrap_or_else(|_| "root".to_string()),
            db_ns: std::env::var("DATABASE_NS").unwrap_or_else(|_| "slatehub-test".to_string()),
            db_name: std::env::var("DATABASE_DB").unwrap_or_else(|_| "test".to_string()),
            minio_endpoint: std::env::var("MINIO_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9100".to_string()),
            minio_access_key: std::env::var("MINIO_ACCESS_KEY")
                .unwrap_or_else(|_| "slatehub-test".to_string()),
            minio_secret_key: std::env::var("MINIO_SECRET_KEY")
                .unwrap_or_else(|_| "slatehub-test123".to_string()),
            minio_bucket: std::env::var("MINIO_BUCKET")
                .unwrap_or_else(|_| "slatehub-test-media".to_string()),
        }
    }
}

/// Setup test database connection and initialize schema
pub async fn setup_test_db() -> Result<Surreal<Client>, Box<dyn std::error::Error>> {
    let config = TestConfig::default();

    // Connect to SurrealDB
    let db = Surreal::new::<Ws>(&config.db_url).await?;

    // Sign in as root user
    db.signin(surrealdb::opt::auth::Root {
        username: &config.db_user,
        password: &config.db_pass,
    })
    .await?;

    // Select namespace and database
    db.use_ns(&config.db_ns).use_db(&config.db_name).await?;

    // Clean the database
    clean_database(&db).await?;

    // Initialize schema
    initialize_schema(&db).await?;

    // Store in global for access in tests
    let mut global_db = TEST_DB.lock().await;
    *global_db = Some(db.clone());

    Ok(db)
}

/// Clean all data from the test database
pub async fn clean_database(db: &Surreal<Client>) -> Result<(), Box<dyn std::error::Error>> {
    // Remove all tables
    let tables = vec![
        "users",
        "profiles",
        "organizations",
        "projects",
        "posts",
        "comments",
        "media",
        "roles",
        "permissions",
        "organization_members",
        "project_members",
        "follows",
        "likes",
        "notifications",
        "messages",
        "activities",
    ];

    for table in tables {
        let query = format!("DELETE {};", table);
        db.query(&query).await?;
    }

    Ok(())
}

/// Initialize database schema for tests
pub async fn initialize_schema(db: &Surreal<Client>) -> Result<(), Box<dyn std::error::Error>> {
    // Read schema file if it exists
    let schema_path = std::path::Path::new("../db/schema.surql");
    if schema_path.exists() {
        let schema = std::fs::read_to_string(schema_path)?;

        // Split schema into individual statements and execute
        let statements: Vec<&str> = schema.split(';').filter(|s| !s.trim().is_empty()).collect();

        for statement in statements {
            let query = format!("{};", statement.trim());
            if !query.starts_with("--") && !query.trim().is_empty() {
                db.query(&query).await?;
            }
        }
    } else {
        // Define minimal test schema
        db.query(
            r#"
            -- Users table
            DEFINE TABLE users SCHEMAFULL;
            DEFINE FIELD email ON TABLE users TYPE string ASSERT string::is::email($value);
            DEFINE FIELD password_hash ON TABLE users TYPE string;
            DEFINE FIELD username ON TABLE users TYPE string;
            DEFINE FIELD created_at ON TABLE users TYPE datetime DEFAULT time::now();
            DEFINE FIELD updated_at ON TABLE users TYPE datetime DEFAULT time::now();
            DEFINE FIELD is_active ON TABLE users TYPE bool DEFAULT true;
            DEFINE INDEX idx_users_email ON TABLE users COLUMNS email UNIQUE;
            DEFINE INDEX idx_users_username ON TABLE users COLUMNS username UNIQUE;

            -- Profiles table
            DEFINE TABLE profiles SCHEMAFULL;
            DEFINE FIELD user_id ON TABLE profiles TYPE record<users>;
            DEFINE FIELD display_name ON TABLE profiles TYPE string;
            DEFINE FIELD bio ON TABLE profiles TYPE option<string>;
            DEFINE FIELD avatar_url ON TABLE profiles TYPE option<string>;
            DEFINE FIELD created_at ON TABLE profiles TYPE datetime DEFAULT time::now();
            DEFINE FIELD updated_at ON TABLE profiles TYPE datetime DEFAULT time::now();
            DEFINE INDEX idx_profiles_user ON TABLE profiles COLUMNS user_id UNIQUE;

            -- Organizations table
            DEFINE TABLE organizations SCHEMAFULL;
            DEFINE FIELD name ON TABLE organizations TYPE string;
            DEFINE FIELD slug ON TABLE organizations TYPE string;
            DEFINE FIELD description ON TABLE organizations TYPE option<string>;
            DEFINE FIELD owner_id ON TABLE organizations TYPE record<users>;
            DEFINE FIELD created_at ON TABLE organizations TYPE datetime DEFAULT time::now();
            DEFINE FIELD updated_at ON TABLE organizations TYPE datetime DEFAULT time::now();
            DEFINE INDEX idx_orgs_slug ON TABLE organizations COLUMNS slug UNIQUE;

            -- Projects table
            DEFINE TABLE projects SCHEMAFULL;
            DEFINE FIELD title ON TABLE projects TYPE string;
            DEFINE FIELD description ON TABLE projects TYPE option<string>;
            DEFINE FIELD organization_id ON TABLE projects TYPE option<record<organizations>>;
            DEFINE FIELD creator_id ON TABLE projects TYPE record<users>;
            DEFINE FIELD status ON TABLE projects TYPE string DEFAULT 'draft';
            DEFINE FIELD created_at ON TABLE projects TYPE datetime DEFAULT time::now();
            DEFINE FIELD updated_at ON TABLE projects TYPE datetime DEFAULT time::now();
        "#,
        )
        .await?;
    }

    Ok(())
}

/// Teardown test database and clean up resources
pub async fn teardown_test_db() -> Result<(), Box<dyn std::error::Error>> {
    let mut global_db = TEST_DB.lock().await;

    if let Some(db) = global_db.as_ref() {
        clean_database(db).await?;
    }

    *global_db = None;
    Ok(())
}

/// Setup MinIO test bucket
pub async fn setup_test_minio() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();

    // Create AWS S3 config for MinIO
    let aws_config = aws_config::ConfigLoader::default()
        .endpoint_url(&config.minio_endpoint)
        .credentials_provider(aws_config::credentials::SharedCredentialsProvider::new(
            aws_sdk_s3::config::Credentials::new(
                &config.minio_access_key,
                &config.minio_secret_key,
                None,
                None,
                "minio",
            ),
        ))
        .region(aws_config::Region::new("us-east-1"))
        .load()
        .await;

    let s3_client = aws_sdk_s3::Client::new(&aws_config);

    // Create test bucket if it doesn't exist
    match s3_client
        .create_bucket()
        .bucket(&config.minio_bucket)
        .send()
        .await
    {
        Ok(_) => println!("Created test bucket: {}", config.minio_bucket),
        Err(e) => {
            // Bucket might already exist, which is fine
            if !e.to_string().contains("BucketAlreadyExists")
                && !e.to_string().contains("BucketAlreadyOwnedByYou")
            {
                return Err(Box::new(e));
            }
        }
    }

    Ok(())
}

/// Clean up MinIO test bucket
pub async fn cleanup_test_minio() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();

    // Create AWS S3 config for MinIO
    let aws_config = aws_config::ConfigLoader::default()
        .endpoint_url(&config.minio_endpoint)
        .credentials_provider(aws_config::credentials::SharedCredentialsProvider::new(
            aws_sdk_s3::config::Credentials::new(
                &config.minio_access_key,
                &config.minio_secret_key,
                None,
                None,
                "minio",
            ),
        ))
        .region(aws_config::Region::new("us-east-1"))
        .load()
        .await;

    let s3_client = aws_sdk_s3::Client::new(&aws_config);

    // List and delete all objects in the bucket
    let objects = s3_client
        .list_objects_v2()
        .bucket(&config.minio_bucket)
        .send()
        .await?;

    if let Some(contents) = objects.contents() {
        for object in contents {
            if let Some(key) = &object.key {
                s3_client
                    .delete_object()
                    .bucket(&config.minio_bucket)
                    .key(key)
                    .send()
                    .await?;
            }
        }
    }

    Ok(())
}

/// Run a test with automatic setup and teardown
pub async fn with_test_db<F, Fut, T>(test_fn: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(Surreal<Client>) -> Fut,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    // Setup
    let db = setup_test_db().await?;
    setup_test_minio().await?;

    // Run test
    let result = test_fn(db).await;

    // Teardown
    teardown_test_db().await?;
    cleanup_test_minio().await?;

    result
}

/// Test fixture for creating a test user
pub async fn create_test_user(
    db: &Surreal<Client>,
    email: &str,
    username: &str,
    password_hash: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let query = r#"
        CREATE users SET
            email = $email,
            username = $username,
            password_hash = $password_hash,
            is_active = true
    "#;

    let mut response = db
        .query(query)
        .bind(("email", email))
        .bind(("username", username))
        .bind(("password_hash", password_hash))
        .await?;

    let user: Option<serde_json::Value> = response.take(0)?;
    let user_id = user
        .and_then(|u| u.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("Failed to create user")?
        .to_string();

    Ok(user_id)
}

/// Test fixture for creating a test organization
pub async fn create_test_org(
    db: &Surreal<Client>,
    name: &str,
    slug: &str,
    owner_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let query = r#"
        CREATE organizations SET
            name = $name,
            slug = $slug,
            owner_id = type::thing('users', $owner_id)
    "#;

    let mut response = db
        .query(query)
        .bind(("name", name))
        .bind(("slug", slug))
        .bind(("owner_id", owner_id))
        .await?;

    let org: Option<serde_json::Value> = response.take(0)?;
    let org_id = org
        .and_then(|o| o.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("Failed to create organization")?
        .to_string();

    Ok(org_id)
}

/// Test fixture for creating a test project
pub async fn create_test_project(
    db: &Surreal<Client>,
    title: &str,
    creator_id: &str,
    org_id: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let query = if let Some(org) = org_id {
        format!(
            r#"
            CREATE projects SET
                title = $title,
                creator_id = type::thing('users', $creator_id),
                organization_id = type::thing('organizations', '{}'),
                status = 'active'
        "#,
            org
        )
    } else {
        r#"
            CREATE projects SET
                title = $title,
                creator_id = type::thing('users', $creator_id),
                status = 'active'
        "#
        .to_string()
    };

    let mut response = db
        .query(&query)
        .bind(("title", title))
        .bind(("creator_id", creator_id))
        .await?;

    let project: Option<serde_json::Value> = response.take(0)?;
    let project_id = project
        .and_then(|p| p.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("Failed to create project")?
        .to_string();

    Ok(project_id)
}
