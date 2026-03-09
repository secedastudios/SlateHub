//! CLI tool to rebuild all vector embeddings for semantic search.
//!
//! Connects to SurrealDB, fetches all records from each entity table,
//! builds embedding text, generates embeddings, and updates each record.
//!
//! Usage: cargo run --bin rebuild-embeddings
//!   or:  make rebuild-embeddings

use slatehub::config::Config;
use slatehub::db::DB;
use slatehub::services::embedding::{
    build_location_embedding_text, build_organization_embedding_text,
    build_person_embedding_text, build_production_embedding_text, generate_embedding,
    init_embedding_service,
};
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;
use surrealdb::types::SurrealValue;

// ── Lightweight DB structs for each entity (only fields needed for embedding) ──

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct PersonRow {
    id: String,
    name: Option<String>,
    username: Option<String>,
    profile: Option<PersonProfileRow>,
}

#[derive(Debug, Clone, serde::Deserialize, SurrealValue)]
struct PersonProfileRow {
    headline: Option<String>,
    bio: Option<String>,
    location: Option<String>,
    skills: Option<Vec<String>>,
    gender: Option<String>,
    ethnicity: Option<Vec<String>>,
    age_range: Option<AgeRangeRow>,
    height_mm: Option<i32>,
    body_type: Option<String>,
    hair_color: Option<String>,
    eye_color: Option<String>,
    languages: Option<Vec<String>>,
    unions: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize, SurrealValue)]
struct AgeRangeRow {
    min: i32,
    max: i32,
}

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct OrgRow {
    id: String,
    name: Option<String>,
    org_type: Option<String>,
    description: Option<String>,
    services: Option<Vec<String>>,
    location: Option<String>,
    founded_year: Option<i32>,
    employees_count: Option<i32>,
}

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct LocationRow {
    id: String,
    name: Option<String>,
    description: Option<String>,
    city: Option<String>,
    state: Option<String>,
    country: Option<String>,
    amenities: Option<Vec<String>>,
    restrictions: Option<Vec<String>>,
    max_capacity: Option<i32>,
    parking_info: Option<String>,
}

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct ProductionRow {
    id: String,
    title: Option<String>,
    #[serde(rename = "type")]
    #[surreal(rename = "type")]
    production_type: Option<String>,
    status: Option<String>,
    description: Option<String>,
    location: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
}

/// Update a record's embedding and embedding_text fields.
/// `raw_id` is the full record ID string from SurrealDB (e.g. "person:abc123").
async fn update_embedding(
    raw_id: String,
    embedding: Vec<f32>,
    embedding_text: String,
) -> Result<(), surrealdb::Error> {
    let query = format!(
        "UPDATE {} SET embedding = $embedding, embedding_text = $embedding_text",
        raw_id
    );

    DB.query(&query)
        .bind(("embedding", embedding))
        .bind(("embedding_text", embedding_text))
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    slatehub::logging::init();

    let config = Config::from_env()?;

    // Connect to DB
    let db_url = config.database.connection_url();
    println!("Connecting to database at: {}", db_url);
    DB.connect::<Ws>(&db_url).await?;
    DB.signin(Root {
        username: config.database.username.clone(),
        password: config.database.password.clone(),
    })
    .await?;
    DB.use_ns(&config.database.namespace)
        .use_db(&config.database.name)
        .await?;
    println!("Connected to database.");

    // Init embedding model
    println!("Loading embedding model (BGE-Large-EN-v1.5)... this may take a moment.");
    init_embedding_service().await?;
    println!("Embedding model loaded.\n");

    let mut total_updated = 0u32;
    let mut total_failed = 0u32;

    // ── People ──
    {
        println!("=== Rebuilding person embeddings ===");
        let mut resp = DB
            .query("SELECT <string> id AS id, name, username, profile FROM person")
            .await?;
        let people: Vec<PersonRow> = resp.take(0)?;
        let count = people.len();
        println!("Found {} person records", count);

        for person in people {
            let display_name = person
                .name
                .as_deref()
                .unwrap_or(person.username.as_deref().unwrap_or("unknown"))
                .to_string();

            let embedding_text = if let Some(profile) = &person.profile {
                build_person_embedding_text(
                    &display_name,
                    profile.headline.as_deref(),
                    profile.bio.as_deref(),
                    &profile.skills.clone().unwrap_or_default(),
                    profile.location.as_deref(),
                    profile.age_range.as_ref().map(|ar| (ar.min, ar.max)),
                    profile.gender.as_deref(),
                    &profile.ethnicity.clone().unwrap_or_default(),
                    profile.height_mm,
                    profile.body_type.as_deref(),
                    profile.hair_color.as_deref(),
                    profile.eye_color.as_deref(),
                    &profile.languages.clone().unwrap_or_default(),
                    &profile.unions.clone().unwrap_or_default(),
                    &[],
                )
            } else {
                build_person_embedding_text(
                    &display_name,
                    None, None, &[], None, None, None, &[], None, None, None, None, &[], &[], &[],
                )
            };

            match generate_embedding(&embedding_text) {
                Ok(emb) => {
                    if let Err(e) = update_embedding(person.id.clone(), emb, embedding_text).await {
                        eprintln!("  Failed to update {}: {}", display_name, e);
                        total_failed += 1;
                    } else {
                        total_updated += 1;
                    }
                }
                Err(e) => {
                    eprintln!("  Failed to generate embedding for {}: {}", display_name, e);
                    total_failed += 1;
                }
            }
        }
        println!("  Done: {} people processed\n", count);
    }

    // ── Organizations ──
    {
        println!("=== Rebuilding organization embeddings ===");
        let mut resp = DB
            .query("SELECT <string> id AS id, name, type.name AS org_type, description, services, location, founded_year, employees_count FROM organization")
            .await?;
        let orgs: Vec<OrgRow> = resp.take(0)?;
        let count = orgs.len();
        println!("Found {} organization records", count);

        for org in orgs {
            let name = org.name.as_deref().unwrap_or("unknown").to_string();
            let embedding_text = build_organization_embedding_text(
                &name,
                org.org_type.as_deref().unwrap_or(""),
                org.description.as_deref(),
                &org.services.unwrap_or_default(),
                org.location.as_deref(),
                org.founded_year,
                org.employees_count,
            );

            match generate_embedding(&embedding_text) {
                Ok(emb) => {
                    if let Err(e) = update_embedding(org.id.clone(), emb, embedding_text).await {
                        eprintln!("  Failed to update org {}: {}", name, e);
                        total_failed += 1;
                    } else {
                        total_updated += 1;
                    }
                }
                Err(e) => {
                    eprintln!("  Failed to generate embedding for org {}: {}", name, e);
                    total_failed += 1;
                }
            }
        }
        println!("  Done: {} organizations processed\n", count);
    }

    // ── Locations ──
    {
        println!("=== Rebuilding location embeddings ===");
        let mut resp = DB
            .query("SELECT <string> id AS id, name, description, city, state, country, amenities, restrictions, max_capacity, parking_info FROM location")
            .await?;
        let locations: Vec<LocationRow> = resp.take(0)?;
        let count = locations.len();
        println!("Found {} location records", count);

        for loc in locations {
            let name = loc.name.as_deref().unwrap_or("unknown").to_string();
            let embedding_text = build_location_embedding_text(
                &name,
                loc.description.as_deref(),
                loc.city.as_deref().unwrap_or(""),
                loc.state.as_deref().unwrap_or(""),
                loc.country.as_deref().unwrap_or(""),
                &loc.amenities.unwrap_or_default(),
                &loc.restrictions.unwrap_or_default(),
                loc.max_capacity,
                loc.parking_info.as_deref(),
            );

            match generate_embedding(&embedding_text) {
                Ok(emb) => {
                    if let Err(e) = update_embedding(loc.id.clone(), emb, embedding_text).await {
                        eprintln!("  Failed to update location {}: {}", name, e);
                        total_failed += 1;
                    } else {
                        total_updated += 1;
                    }
                }
                Err(e) => {
                    eprintln!("  Failed to generate embedding for location {}: {}", name, e);
                    total_failed += 1;
                }
            }
        }
        println!("  Done: {} locations processed\n", count);
    }

    // ── Productions ──
    {
        println!("=== Rebuilding production embeddings ===");
        let mut resp = DB
            .query("SELECT <string> id AS id, title, type, status, description, location, <string> start_date AS start_date, <string> end_date AS end_date FROM production")
            .await?;
        let productions: Vec<ProductionRow> = resp.take(0)?;
        let count = productions.len();
        println!("Found {} production records", count);

        for prod in productions {
            let title = prod.title.as_deref().unwrap_or("unknown").to_string();
            let embedding_text = build_production_embedding_text(
                &title,
                prod.production_type.as_deref().unwrap_or(""),
                prod.status.as_deref().unwrap_or(""),
                prod.description.as_deref(),
                prod.location.as_deref(),
                prod.start_date.as_deref(),
                prod.end_date.as_deref(),
            );

            match generate_embedding(&embedding_text) {
                Ok(emb) => {
                    if let Err(e) = update_embedding(prod.id.clone(), emb, embedding_text).await {
                        eprintln!("  Failed to update production {}: {}", title, e);
                        total_failed += 1;
                    } else {
                        total_updated += 1;
                    }
                }
                Err(e) => {
                    eprintln!("  Failed to generate embedding for production {}: {}", title, e);
                    total_failed += 1;
                }
            }
        }
        println!("  Done: {} productions processed\n", count);
    }

    println!("========================================");
    println!(
        "Embedding rebuild complete: {} updated, {} failed",
        total_updated, total_failed
    );

    Ok(())
}
