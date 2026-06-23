//! Shared harness for the integration-test binaries.
//!
//! Each test binary gets its own process (and therefore its own copy of the
//! global [`DB`] singleton); within a binary, tests run sequentially via
//! `--test-threads=1` and share one tokio runtime so the WebSocket
//! connection to the test SurrealDB (localhost:8100, started by
//! `make test-services`) outlives any single test.
//!
//! Connection setup retries: a SurrealDB WS session can die out from under
//! a live client (host sleep/wake pausing the Docker VM, socket drops with
//! the SDK auto-reconnecting onto a stale session id — symptom:
//! `Session not found: <uuid>`). One failed attempt is therefore never
//! grounds to fail a test; we re-connect and re-authenticate up to
//! [`CONNECT_ATTEMPTS`] times.

use once_cell::sync::OnceCell;
use slatehub::db::DB;
use std::sync::LazyLock;
use surrealdb::{engine::remote::ws::Ws, opt::auth::Root};
use tokio::runtime::Runtime;

static RT: OnceCell<Runtime> = OnceCell::new();

/// Attempts made to establish (or re-establish) an authenticated session
/// before giving up.
const CONNECT_ATTEMPTS: u32 = 3;

/// Serializes connection setup. The crate pins `RUST_TEST_THREADS=1`
/// (.cargo/config.toml), but if anyone overrides that, concurrent
/// `connect()` calls on the shared client would replace each other's
/// sessions mid-auth ("Session not found" storms). One thread sets up;
/// the rest wait and reuse the live session.
static SETUP_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Get the shared tokio runtime for all integration tests.
/// This ensures the WebSocket connection to SurrealDB outlives any single test.
pub fn runtime() -> &'static Runtime {
    RT.get_or_init(|| Runtime::new().expect("Failed to create test runtime"))
}

/// One full connect → signin → use_ns/use_db attempt. Any step may fail
/// with a stale-session error after the previous attempt's socket died;
/// the caller retries the whole sequence so auth always lands on the
/// session that connect() just established.
async fn try_connect() -> Result<(), surrealdb::Error> {
    // A second setup_test_db() within the same binary hits the already-live
    // connection; that's fine — fall through and re-auth on it (idempotent).
    if let Err(e) = DB.connect::<Ws>("localhost:8100").await
        && !e.to_string().contains("Already connected")
    {
        return Err(e);
    }
    DB.signin(Root {
        username: "root".to_string(),
        password: "root".to_string(),
    })
    .await?;
    DB.use_ns("slatehub-test").use_db("test").await?;
    Ok(())
}

/// Connect the global DB singleton to the test SurrealDB instance.
/// Safe to call multiple times and from multiple threads — setup is
/// serialized, and an already-authenticated session is reused via a
/// cheap liveness probe instead of a disruptive reconnect.
async fn connect_db() {
    let _guard = SETUP_LOCK.lock().await;

    // Session already alive (set up by an earlier test or another thread
    // that held the lock first)? Don't touch it — reconnecting would
    // invalidate it for everyone sharing the client.
    if DB.query("RETURN 1").await.is_ok() {
        return;
    }

    let mut last_err = None;
    for attempt in 1..=CONNECT_ATTEMPTS {
        match try_connect().await {
            Ok(()) => return,
            Err(e) => {
                eprintln!("test DB connect attempt {attempt}/{CONNECT_ATTEMPTS} failed: {e}");
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(300 * attempt as u64)).await;
            }
        }
    }
    panic!(
        "Failed to connect/authenticate to the test DB after {CONNECT_ATTEMPTS} attempts \
         (is it running? `make test-services test-db-init`): {:?}",
        last_err
    );
}

/// Set up the test DB connection. Call at the start of each test.
pub fn setup_test_db() {
    runtime().block_on(connect_db());
}

/// Clean a table between tests.
///
/// Retries once through a full reconnect if the first DELETE fails — the
/// session may have died since the last test touched the connection.
pub fn clean_table(table: &str) {
    runtime().block_on(async {
        let query = format!("DELETE FROM {table}");
        if let Err(first) = DB.query(&query).await {
            eprintln!("clean_table({table}) failed ({first}); reconnecting and retrying");
            connect_db().await;
            DB.query(&query)
                .await
                .unwrap_or_else(|e| panic!("Failed to clean table {table} after reconnect: {e}"));
        }
    });
}

/// Run an async test closure on the shared runtime
pub fn run<F: std::future::Future<Output = ()>>(f: F) {
    runtime().block_on(f);
}
