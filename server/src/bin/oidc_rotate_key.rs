//! Rotates the OIDC signing key.
//!
//! Usage: `cargo run --bin oidc_rotate_key -- [--grace-days N]`
//!
//! Generates a new ed25519 keypair, marks the previously-active key inactive
//! with `not_after = now + grace_days` so the JWKS keeps publishing it during
//! the overlap window for clients still validating tokens it signed.

use slatehub::config::Config;
use slatehub::db::DB;
use slatehub::services::oidc_keys::rotate_signing_key;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    slatehub::logging::init();
    let config = Config::from_env()?;

    let mut grace_days: i64 = 7;
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--grace-days" {
            grace_days = iter
                .next()
                .ok_or("--grace-days requires a value")?
                .parse()?;
        } else {
            return Err(format!("unknown arg: {arg}").into());
        }
    }

    DB.connect::<Ws>(&config.database.connection_url()).await?;
    DB.signin(Root {
        username: config.database.username.clone(),
        password: config.database.password.clone(),
    })
    .await?;
    DB.use_ns(&config.database.namespace)
        .use_db(&config.database.name)
        .await?;

    let new_kid = rotate_signing_key(grace_days).await?;
    println!("Rotated. New active kid: {new_kid} (grace: {grace_days} days)");
    Ok(())
}
