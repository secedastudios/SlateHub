use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, info};

/// Global embedding service instance
/// Uses BGE-Large-EN-v1.5 for high-accuracy semantic search (1024 dimensions)
static EMBEDDER: LazyLock<Arc<Mutex<Option<TextEmbedding>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

/// Initialize the embedding service
/// This should be called once at application startup
pub async fn init_embedding_service() -> Result<()> {
    info!("Initializing embedding service with BGE-Large-EN-v1.5 model");

    let embedder = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGELargeENV15))?;

    let mut global_embedder = EMBEDDER.lock().unwrap();
    *global_embedder = Some(embedder);

    info!("Embedding service initialized successfully");
    Ok(())
}

/// Generate embedding for a single text
pub fn generate_embedding(text: &str) -> Result<Vec<f32>> {
    let embedder = EMBEDDER.lock().unwrap();

    match embedder.as_ref() {
        Some(e) => {
            debug!(
                "Generating embedding for text: {}",
                &text[..text.len().min(100)]
            );
            let embeddings = e.embed(vec![text.to_string()], None)?;
            Ok(embeddings.into_iter().next().unwrap())
        }
        None => Err(anyhow::anyhow!(
            "Embedding service not initialized. Call init_embedding_service() first."
        )),
    }
}

/// Generate embeddings for multiple texts in batch (more efficient)
pub fn generate_embeddings_batch(texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
    let embedder = EMBEDDER.lock().unwrap();

    match embedder.as_ref() {
        Some(e) => {
            debug!("Generating {} embeddings in batch", texts.len());
            let embeddings = e.embed(texts, None)?;
            Ok(embeddings)
        }
        None => Err(anyhow::anyhow!(
            "Embedding service not initialized. Call init_embedding_service() first."
        )),
    }
}

/// Build optimized text for person/actor embedding
/// Focuses on: role type, skills, physical attributes, location, experience
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

    if !ethnicity.is_empty() {
        parts.push(format!("Ethnicity: {}", ethnicity.join(", ")));
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

    parts.join(". ")
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

    parts.join(". ")
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

    parts.join(". ")
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

    parts.join(". ")
}
