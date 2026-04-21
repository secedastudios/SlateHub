use once_cell::sync::OnceCell;
use slatehub::db::DB;
use surrealdb::{engine::remote::ws::Ws, opt::auth::Root};
use tokio::runtime::Runtime;

static RT: OnceCell<Runtime> = OnceCell::new();

/// Get the shared tokio runtime for all integration tests.
/// This ensures the WebSocket connection to SurrealDB outlives any single test.
pub fn runtime() -> &'static Runtime {
    RT.get_or_init(|| Runtime::new().expect("Failed to create test runtime"))
}

/// Connect the global DB singleton to the test SurrealDB instance.
/// Safe to call multiple times — reconnects if the connection was lost.
async fn connect_db() {
    // Always try to reconnect — previous runtime may have dropped the WS connection
    if DB.connect::<Ws>("localhost:8100").await.is_ok() {
        DB.signin(Root {
            username: "root".to_string(),
            password: "root".to_string(),
        })
        .await
        .expect("Failed to authenticate with test DB");

        DB.use_ns("slatehub-test")
            .use_db("test")
            .await
            .expect("Failed to select test namespace/database");
    }
}

/// Set up the test DB connection. Call at the start of each test.
pub fn setup_test_db() {
    runtime().block_on(connect_db());
}

/// Clean a table between tests
pub fn clean_table(table: &str) {
    runtime().block_on(async {
        let query = format!("DELETE FROM {table}");
        DB.query(&query)
            .await
            .unwrap_or_else(|e| panic!("Failed to clean table {table}: {e}"));
    });
}

/// Run an async test closure on the shared runtime
pub fn run<F: std::future::Future<Output = ()>>(f: F) {
    runtime().block_on(f);
}
