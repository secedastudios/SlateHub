use slatehub::config::Config;
use slatehub::db::DB;
use surrealdb::{engine::remote::ws::Ws, opt::auth::Root};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment variables
    let config = Config::from_env()?;

    // Connect to database using configuration
    let db_url = config.database.connection_url();
    println!("Connecting to database at: {}", db_url);
    DB.connect::<Ws>(&db_url).await?;

    // Sign in to database using configured credentials
    DB.signin(Root {
        username: &config.database.username,
        password: &config.database.password,
    })
    .await?;

    // Use configured namespace and database
    DB.use_ns(&config.database.namespace)
        .use_db(&config.database.name)
        .await?;
    println!(
        "Using namespace: {} and database: {}",
        config.database.namespace, config.database.name
    );

    // Create the application
    let app = slatehub::routes::app();

    // Bind to configured server address
    let server_addr = config.server.socket_addr()?;
    println!("Server listening on: {}", server_addr);

    let listener = tokio::net::TcpListener::bind(server_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
