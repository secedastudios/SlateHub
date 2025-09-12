use std::sync::LazyLock;
use surrealdb::{Surreal, engine::remote::ws::Client, engine::remote::ws::Ws, opt::auth::Root};
use tracing::{info, debug, error};

// Simple database connection
pub static DB: LazyLock<Surreal<Client>> = LazyLock::new(|| {
    Surreal::init()
});

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting organization types test...");

    // Connect to database
    let db_url = "localhost:8000";
    info!("Connecting to database at: {}", db_url);

    DB.connect::<Ws>(db_url).await?;
    info!("Connected to database");

    // Sign in
    DB.signin(Root {
        username: "root",
        password: "root",
    }).await?;
    info!("Signed in to database");

    // Select namespace and database
    DB.use_ns("slatehub").use_db("main").await?;
    info!("Using namespace: slatehub, database: main");

    // Test 1: Query with meta::id()
    info!("Test 1: Query with meta::id()");
    let sql = "SELECT meta::id(id) as id, name FROM organization_type ORDER BY name";
    let mut response = DB.query(sql).await?;

    let records: Vec<std::collections::BTreeMap<String, serde_json::Value>> =
        response.take(0).unwrap_or_default();

    info!("Found {} records with meta::id()", records.len());
    for (i, record) in records.iter().take(3).enumerate() {
        info!("  Record {}: {:?}", i + 1, record);
    }

    // Test 2: Query without meta::id()
    info!("\nTest 2: Query without meta::id()");
    let sql2 = "SELECT id, name FROM organization_type ORDER BY name";
    let mut response2 = DB.query(sql2).await?;

    let records2: Vec<std::collections::BTreeMap<String, serde_json::Value>> =
        response2.take(0).unwrap_or_default();

    info!("Found {} records without meta::id()", records2.len());
    for (i, record) in records2.iter().take(3).enumerate() {
        info!("  Record {}: {:?}", i + 1, record);
    }

    // Test 3: Extract as tuples
    info!("\nTest 3: Extracting as tuples");
    let mut types = Vec::new();
    for record in records {
        if let (Some(id_val), Some(name_val)) = (record.get("id"), record.get("name")) {
            if let (Some(id), Some(name)) = (id_val.as_str(), name_val.as_str()) {
                types.push((id.to_string(), name.to_string()));
            }
        }
    }

    info!("Extracted {} tuples", types.len());
    for (i, (id, name)) in types.iter().take(5).enumerate() {
        info!("  {}: {} - {}", i + 1, id, name);
    }

    // Test 4: Try different extraction approaches
    info!("\nTest 4: Direct struct extraction test");

    #[derive(Debug, serde::Deserialize)]
    struct OrgType {
        id: String,
        name: String,
    }

    let sql3 = "SELECT meta::id(id) as id, name FROM organization_type ORDER BY name";
    let mut response3 = DB.query(sql3).await?;

    match response3.take::<Vec<OrgType>>(0) {
        Ok(org_types) => {
            info!("Direct struct extraction succeeded! Found {} types", org_types.len());
            for (i, org_type) in org_types.iter().take(5).enumerate() {
                info!("  {}: {:?}", i + 1, org_type);
            }
        }
        Err(e) => {
            error!("Direct struct extraction failed: {}", e);
        }
    }

    info!("\nAll tests completed!");

    if types.is_empty() {
        error!("No organization types found! Database may need initialization.");
        error!("Run: make db-init");
    } else {
        info!("SUCCESS: Found {} organization types", types.len());
    }

    Ok(())
}
