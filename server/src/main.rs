use slatehub::config::Config;
use slatehub::db::{DB, ensure_db_initialized};
use slatehub::services::s3::init_s3;
use surrealdb::{engine::remote::ws::Ws, opt::auth::Root};
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file first, before logging initialization
    dotenv::dotenv().ok();

    // Initialize logging (will now pick up RUST_LOG and LOG_FORMAT from .env)
    slatehub::logging::init();

    info!("Starting SlateHub server...");

    // Initialize templates
    debug!("Initializing template system");
    if let Err(e) = slatehub::templates::init() {
        error!("Failed to initialize templates: {}", e);
        return Err(e.into());
    }
    info!("Templates initialized successfully");

    // Load configuration from environment variables
    debug!("Loading configuration from environment");
    let config = match Config::from_env() {
        Ok(cfg) => {
            info!("Configuration loaded successfully");
            cfg
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            return Err(e.into());
        }
    };

    // Connect to database using configuration
    let db_url = config.database.connection_url();
    info!("Connecting to database at: {}", db_url);

    match DB.connect::<Ws>(&db_url).await {
        Ok(_) => info!("Database connection established"),
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(e.into());
        }
    }

    // Sign in to database using configured credentials
    debug!("Authenticating with database");
    match DB
        .signin(Root {
            username: &config.database.username,
            password: &config.database.password,
        })
        .await
    {
        Ok(_) => info!("Database authentication successful"),
        Err(e) => {
            error!("Database authentication failed: {}", e);
            return Err(e.into());
        }
    }

    // Use configured namespace and database
    debug!(
        "Setting namespace: {} and database: {}",
        config.database.namespace, config.database.name
    );
    match DB
        .use_ns(&config.database.namespace)
        .use_db(&config.database.name)
        .await
    {
        Ok(_) => info!(
            "Using namespace: {} and database: {}",
            config.database.namespace, config.database.name
        ),
        Err(e) => {
            error!("Failed to set namespace/database: {}", e);
            return Err(e.into());
        }
    }

    // Verify database is properly initialized and ready
    debug!("Verifying database initialization");
    match ensure_db_initialized().await {
        Ok(_) => info!("Database initialization verified"),
        Err(e) => {
            error!("Database initialization verification failed: {}", e);
            return Err(e.into());
        }
    }

    // Initialize S3/MinIO service
    debug!("Initializing S3 service");
    match init_s3().await {
        Ok(_) => info!("S3 service initialized successfully"),
        Err(e) => {
            error!("Failed to initialize S3 service: {}", e);
            // Continue without S3 - profile images won't work but app can run
            error!("Warning: Profile image uploads will not work without S3 service");
        }
    }

    // Create the application
    debug!("Building application routes");
    let app = slatehub::routes::app();
    info!("Application routes configured");

    // Bind to configured server address
    let server_addr = config.server.socket_addr()?;
    info!("Starting server on: {}", server_addr);

    let listener = match tokio::net::TcpListener::bind(server_addr).await {
        Ok(l) => {
            info!("Server successfully bound to {}", server_addr);
            l
        }
        Err(e) => {
            error!("Failed to bind to {}: {}", server_addr, e);
            return Err(e.into());
        }
    };

    info!("SlateHub server is ready to accept connections");

    // Run the server
    match axum::serve(listener, app).await {
        Ok(_) => {
            info!("Server shutdown gracefully");
            Ok(())
        }
        Err(e) => {
            error!("Server error: {}", e);
            Err(e.into())
        }
    }
}
