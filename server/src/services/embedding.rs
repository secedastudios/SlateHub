use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::OnceLock;
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{debug, info, warn};

/// Global embedding service instance — written once at startup, read concurrently forever after.
/// No Mutex needed: OnceLock guarantees safe one-time init, and TextEmbedding::embed takes &self.
static EMBEDDER: OnceLock<TextEmbedding> = OnceLock::new();

/// Initialize the embedding service
/// This should be called once at application startup
pub async fn init_embedding_service() -> Result<()> {
    info!("Initializing embedding service with BGE-Large-EN-v1.5 model");

    let embedder = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGELargeENV15))?;

    EMBEDDER.set(embedder).map_err(|_| anyhow::anyhow!("Embedding service already initialized"))?;

    info!("Embedding service initialized successfully");
    Ok(())
}

/// Generate embedding for a single text (blocking — use generate_embedding_async from async contexts)
pub fn generate_embedding(text: &str) -> Result<Vec<f32>> {
    let embedder = EMBEDDER.get().ok_or_else(|| {
        anyhow::anyhow!("Embedding service not initialized. Call init_embedding_service() first.")
    })?;

    debug!(
        "Generating embedding for text: {}",
        text.chars().take(100).collect::<String>()
    );
    let embeddings = embedder.embed(vec![text.to_string()], None)?;
    Ok(embeddings.into_iter().next().unwrap())
}

/// Async-safe embedding generation — runs on a blocking thread to avoid starving the async runtime
pub async fn generate_embedding_async(text: &str) -> Result<Vec<f32>> {
    let text = text.to_string();
    tokio::task::spawn_blocking(move || generate_embedding(&text)).await?
}

/// Fire-and-forget: generate embedding and write it to the record in the background.
/// Durable: writes a `pending_embedding` record before spawning, deletes it on completion.
/// On server restart, `backfill_pending_embeddings()` re-processes any remaining records.
pub fn spawn_embedding_update(record_id: RecordId, embedding_text: String) {
    tokio::spawn(async move {
        let db = &crate::db::DB;

        // Write pending record for durability — if server crashes, this survives
        if let Err(e) = db
            .query("INSERT INTO pending_embedding (target, embedding_text) VALUES ($target, $text) ON DUPLICATE KEY UPDATE embedding_text = $text")
            .bind(("target", record_id.clone()))
            .bind(("text", embedding_text.clone()))
            .await
        {
            warn!(record_id = ?record_id, error = %e, "Failed to write pending_embedding record");
            // Still attempt the embedding — just won't be durable
        }

        process_single_embedding(db, record_id, embedding_text).await;
    });
}

/// Process a single embedding: generate vector, update target record, remove pending record.
async fn process_single_embedding(
    db: &surrealdb::Surreal<surrealdb::engine::remote::ws::Client>,
    record_id: RecordId,
    embedding_text: String,
) {
    let text_clone = embedding_text.clone();
    let rid_clone = record_id.clone();
    let embedding = match tokio::task::spawn_blocking(move || generate_embedding(&text_clone))
        .await
    {
        Ok(Ok(emb)) => emb,
        Ok(Err(e)) => {
            warn!(record_id = ?rid_clone, error = %e, "Background embedding failed");
            return;
        }
        Err(e) => {
            warn!(record_id = ?rid_clone, error = %e, "Background embedding task panicked");
            return;
        }
    };

    if let Err(e) = db
        .query("UPDATE $id SET embedding = $embedding, embedding_text = $embedding_text")
        .bind(("id", record_id.clone()))
        .bind(("embedding", embedding))
        .bind(("embedding_text", embedding_text))
        .await
    {
        warn!(record_id = ?record_id, error = %e, "Background embedding DB update failed");
        return;
    }

    // Success — remove the pending record
    if let Err(e) = db
        .query("DELETE FROM pending_embedding WHERE target = $target")
        .bind(("target", record_id.clone()))
        .await
    {
        warn!(record_id = ?record_id, error = %e, "Failed to delete pending_embedding record");
    } else {
        debug!(record_id = ?record_id, "Background embedding updated");
    }
}

/// Process any pending embeddings left over from a previous server run.
/// Call this once at startup after `init_embedding_service()`.
pub async fn backfill_pending_embeddings() {
    let db = &crate::db::DB;

    #[derive(Debug, serde::Deserialize, SurrealValue)]
    struct PendingRow {
        target: RecordId,
        embedding_text: String,
    }

    let mut resp = match db.query("SELECT target, embedding_text FROM pending_embedding").await {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "Failed to query pending_embedding table");
            return;
        }
    };

    let rows: Vec<PendingRow> = match resp.take(0) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "Failed to deserialize pending_embedding rows");
            return;
        }
    };

    if rows.is_empty() {
        info!("No pending embeddings to backfill");
        return;
    }

    info!("Backfilling {} pending embeddings", rows.len());
    for row in rows {
        info!(target = ?row.target, "Processing pending embedding");
        process_single_embedding(db, row.target, row.embedding_text).await;
    }
    info!("Pending embedding backfill complete");
}

/// Generate embeddings for multiple texts in batch (more efficient)
pub fn generate_embeddings_batch(texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
    let embedder = EMBEDDER.get().ok_or_else(|| {
        anyhow::anyhow!("Embedding service not initialized. Call init_embedding_service() first.")
    })?;

    debug!("Generating {} embeddings in batch", texts.len());
    let embeddings = embedder.embed(texts, None)?;
    Ok(embeddings)
}

/// Build optimized text for person/actor embedding
/// Focuses on: role type, skills, physical attributes, location, experience
#[allow(clippy::too_many_arguments)]
pub fn build_person_embedding_text(
    name: &str,
    headline: Option<&str>,
    bio: Option<&str>,
    skills: &[String],
    location: Option<&str>,
    age_range: Option<(i32, i32)>,
    gender: Option<&str>,
    ethnicity: &[String],
    height_cm: Option<i32>,
    body_type: Option<&str>,
    hair_color: Option<&str>,
    eye_color: Option<&str>,
    languages: &[String],
    unions: &[String],
    experience: &[String], // descriptions of past work
    acting_age_range: Option<(i32, i32)>,
    acting_ethnicities: &[String],
    nationality: Option<&str>,
) -> String {
    let mut parts = Vec::new();

    // Name and headline for identity
    parts.push(format!("Name: {}", name));
    if let Some(h) = headline {
        parts.push(format!("Role: {}", h));
    }

    // Physical characteristics (important for casting)
    if let Some(g) = gender {
        parts.push(format!("Gender: {}", g));
    }

    if let Some((min, max)) = age_range {
        parts.push(format!("Age range: {}-{} years old", min, max));
    }

    if let Some((min, max)) = acting_age_range {
        parts.push(format!("Can play ages: {}-{}", min, max));
    }

    if !ethnicity.is_empty() {
        parts.push(format!("Ethnicity: {}", ethnicity.join(", ")));
    }

    if !acting_ethnicities.is_empty() {
        parts.push(format!("Can portray: {}", acting_ethnicities.join(", ")));
    }

    if let Some(nat) = nationality {
        parts.push(format!("Nationality: {}", nat));
    }

    if let Some(height) = height_cm {
        let feet = height / 30; // rough conversion
        let inches = (height % 30) * 12 / 30;
        parts.push(format!("Height: {} cm ({}'{}\")", height, feet, inches));
    }

    if let Some(bt) = body_type {
        parts.push(format!("Build: {}", bt));
    }

    if let Some(hc) = hair_color {
        parts.push(format!("Hair: {}", hc));
    }

    if let Some(ec) = eye_color {
        parts.push(format!("Eyes: {}", ec));
    }

    // Location for geographical search
    if let Some(loc) = location {
        parts.push(format!("Location: {}", loc));
    }

    // Skills and abilities (critical for matching)
    if !skills.is_empty() {
        parts.push(format!("Skills and abilities: {}", skills.join(", ")));
    }

    // Languages for multilingual projects
    if !languages.is_empty() {
        parts.push(format!("Languages: {}", languages.join(", ")));
    }

    // Union status for professional requirements
    if !unions.is_empty() {
        parts.push(format!("Union membership: {}", unions.join(", ")));
    }

    // Bio for additional context
    if let Some(b) = bio {
        parts.push(format!("Background: {}", b));
    }

    // Experience descriptions for semantic matching
    if !experience.is_empty() {
        parts.push(format!("Experience: {}", experience.join(". ")));
    }

    parts.join(". ").to_lowercase()
}

/// Build optimized text for organization embedding
/// Focuses on: services, industry specialization, location, size
pub fn build_organization_embedding_text(
    name: &str,
    org_type: &str,
    description: Option<&str>,
    services: &[String],
    location: Option<&str>,
    founded_year: Option<i32>,
    employees_count: Option<i32>,
) -> String {
    let mut parts = Vec::new();

    // Name and type
    parts.push(format!("Organization: {}", name));
    parts.push(format!("Type: {}", org_type));

    // Location
    if let Some(loc) = location {
        parts.push(format!("Location: {}", loc));
    }

    // Services and capabilities (critical for matching)
    if !services.is_empty() {
        parts.push(format!("Services: {}", services.join(", ")));
    }

    // Company size context
    if let Some(year) = founded_year {
        use chrono::Datelike;
        let age = chrono::Utc::now().year() - year;
        parts.push(format!("Established {} years ago (founded {})", age, year));
    }

    if let Some(count) = employees_count {
        let size = match count {
            0..=10 => "small",
            11..=50 => "medium",
            51..=200 => "large",
            _ => "enterprise",
        };
        parts.push(format!("{} company with {} employees", size, count));
    }

    // Description for detailed context
    if let Some(desc) = description {
        parts.push(format!("Description: {}", desc));
    }

    parts.join(". ").to_lowercase()
}

/// Build optimized text for location embedding
/// Focuses on: type of space, amenities, capacity, accessibility, atmosphere
pub fn build_location_embedding_text(
    name: &str,
    description: Option<&str>,
    city: &str,
    state: &str,
    country: &str,
    amenities: &[String],
    restrictions: &[String],
    max_capacity: Option<i32>,
    parking_info: Option<&str>,
) -> String {
    let mut parts = Vec::new();

    // Name and location
    parts.push(format!("Location: {}", name));
    parts.push(format!("Located in {}, {}, {}", city, state, country));

    // Description is critical for understanding the space type and atmosphere
    if let Some(desc) = description {
        parts.push(format!("Description: {}", desc));
    }

    // Amenities and features (important for production needs)
    if !amenities.is_empty() {
        parts.push(format!("Amenities and features: {}", amenities.join(", ")));
    }

    // Capacity for crew/cast size planning
    if let Some(cap) = max_capacity {
        parts.push(format!("Maximum capacity: {} people", cap));
    }

    // Parking for logistics
    if let Some(parking) = parking_info {
        parts.push(format!("Parking: {}", parking));
    }

    // Restrictions that might affect suitability
    if !restrictions.is_empty() {
        parts.push(format!("Restrictions: {}", restrictions.join(", ")));
    }

    parts.join(". ").to_lowercase()
}

/// Build optimized text for production embedding
/// Focuses on: genre, type, description, requirements, timeline
pub fn build_production_embedding_text(
    title: &str,
    production_type: &str,
    status: &str,
    description: Option<&str>,
    location: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> String {
    let mut parts = Vec::new();

    // Title and type
    parts.push(format!("Production: {}", title));
    parts.push(format!("Type: {}", production_type));
    parts.push(format!("Status: {}", status));

    // Timeline
    if let Some(start) = start_date {
        if let Some(end) = end_date {
            parts.push(format!("Scheduled from {} to {}", start, end));
        } else {
            parts.push(format!("Starts on {}", start));
        }
    }

    // Location
    if let Some(loc) = location {
        parts.push(format!("Filming location: {}", loc));
    }

    // Description is critical for understanding the project
    if let Some(desc) = description {
        parts.push(format!("Description: {}", desc));
    }

    parts.join(". ").to_lowercase()
}
