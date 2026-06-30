use slatehub::config::Config;
use slatehub::db::{DB, ensure_db_initialized};
use slatehub::services::embedding::init_embedding_service;
use slatehub::services::oidc_keys::ensure_signing_key;
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

    info!("Database Config:");
    info!("  User: {}", config.database.username);
    info!(
        "  Password: {}",
        if config.database.password.is_empty() {
            "<empty>"
        } else {
            "********"
        }
    );
    info!("  Namespace: {}", config.database.namespace);
    info!("  Database: {}", config.database.name);

    info!("Connecting to database at: {}", db_url);

    let max_retries = 10;
    let mut retry_count = 0;

    loop {
        match DB.connect::<Ws>(&db_url).await {
            Ok(_) => {
                info!("Database connection established");
                break;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    error!(
                        "Failed to connect to database after {} attempts: {}",
                        max_retries, e
                    );
                    return Err(e.into());
                }
                error!(
                    "Failed to connect to database (attempt {}/{}): {}. Retrying in 2 seconds...",
                    retry_count, max_retries, e
                );
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }

    // Sign in to database using configured credentials
    debug!("Authenticating with database");
    match DB
        .signin(Root {
            username: config.database.username.clone(),
            password: config.database.password.clone(),
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

    // Ensure an OIDC signing key exists (generates one on first boot)
    debug!("Ensuring OIDC signing key");
    if let Err(e) = ensure_signing_key().await {
        error!("Failed to ensure OIDC signing key: {}", e);
        return Err(e.into());
    }

    // Start SSF / CAEP / RISC delivery worker.
    slatehub::services::oidc_events::spawn_delivery_worker();

    // Initialize S3 service
    debug!("Initializing S3 service");
    match init_s3().await {
        Ok(_) => info!("S3 service initialized successfully"),
        Err(e) => {
            error!("Failed to initialize S3 service: {}", e);
            // Continue without S3 - profile images won't work but app can run
            error!("Warning: Profile image uploads will not work without S3 service");
        }
    }

    // Initialize embedding service for semantic search
    debug!("Initializing embedding service");
    match init_embedding_service().await {
        Ok(_) => {
            info!("Embedding service initialized successfully");
            // Process any embeddings that were pending when the server last stopped
            slatehub::services::embedding::backfill_pending_embeddings().await;
        }
        Err(e) => {
            error!("Failed to initialize embedding service: {}", e);
            error!("Warning: Semantic search will not work without embedding service");
            // Continue without embeddings - search won't work but app can run
        }
    }

    // Log Listmonk wiring status (no init needed — service is built per-call from env)
    slatehub::services::listmonk::log_status();

    // Log Stripe wiring status
    slatehub::services::stripe::StripeService::log_status();

    // Seed any feature_flag rows missing from the DB (defaults to 'off').
    slatehub::services::feature_flag::register_flags().await;

    // Daily job: refund any verification_payment rows that have been in
    // `paid` state for >24h without becoming `verified`. The webhook
    // handles the happy path; this catches user-abandoned sessions.
    tokio::spawn(async {
        // Run once on startup (catches anything stranded across restarts),
        // then daily.
        slatehub::services::stripe::refund_stale_payments(24).await;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
            info!("Running stale-payment refund sweep");
            slatehub::services::stripe::refund_stale_payments(24).await;
        }
    });

    // Start system stats tracking
    slatehub::stats::init();

    // Start daily activity cleanup (90-day retention)
    tokio::spawn(async {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
            info!("Running activity event cleanup");
            slatehub::models::activity::ActivityModel::cleanup(90).await;
        }
    });

    // Start daily cleanup of unverified accounts older than 5 days
    tokio::spawn(async {
        // Run once on startup, then daily
        slatehub::models::person::Person::cleanup_unverified(5).await;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
            info!("Running unverified account cleanup");
            slatehub::models::person::Person::cleanup_unverified(5).await;
        }
    });

    // Start daily profile-completion reminder sweep: nudge verified-but-empty
    // accounts to finish their profile, and (when enabled) remove the ones that
    // never do, so the directory stays a real listing of talent and crew.
    tokio::spawn(async {
        use slatehub::services::profile_reminders::{self, ReminderConfig};
        let cfg = ReminderConfig::from_env();
        profile_reminders::run(&cfg).await;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
            info!("Running profile-completion reminder sweep");
            profile_reminders::run(&cfg).await;
        }
    });

    // Start live notification stream
    info!("Starting notification live stream");
    slatehub::services::notification_stream::init().await;

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

    // Run the server. `into_make_service_with_connect_info` exposes the socket
    // peer address to handlers (via `ConnectInfo<SocketAddr>`) so signup IP
    // resolution has a real fallback when proxy headers are absent.
    match axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    {
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
