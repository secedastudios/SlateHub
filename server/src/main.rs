use slatehub::db::DB;
//use slatehub::error::Error;
use surrealdb::engine::remote::ws::Ws;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    DB.connect::<Ws>("localhost:8000").await?;

    let app = slatehub::routes::app();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())

}
