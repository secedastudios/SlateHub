use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use regex::Regex;
use crate::config::mcp_search_weights;
use crate::db::DB;
use crate::services::embedding::generate_embedding_async;
use crate::services::search_log::log_search;

// ---------------------------------------------------------------------------
// Tool parameter types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchPeopleParams {
    /// Natural language search query, e.g. "cinematographers in Berlin" or "female actors ages 20-30"
    pub query: String,
    /// Filter by location (city, state, or country). Applied as a hard filter.
    #[schemars(default)]
    pub location: Option<String>,
    /// Filter by skill or role, e.g. "cinematographer", "editor", "actor"
    #[schemars(default)]
    pub skill: Option<String>,
    /// Maximum number of results (default 50, max 100)
    #[schemars(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchProductionsParams {
    /// Natural language search query, e.g. "horror films shooting in LA"
    pub query: String,
    /// Filter by location
    #[schemars(default)]
    pub location: Option<String>,
    /// Filter by status: "Pre-Production", "In Production", "Post-Production", "Completed"
    #[schemars(default)]
    pub status: Option<String>,
    /// Maximum number of results (default 50, max 100)
    #[schemars(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchOrganizationsParams {
    /// Natural language search query, e.g. "post-production studios in London"
    pub query: String,
    /// Filter by location
    #[schemars(default)]
    pub location: Option<String>,
    /// Maximum number of results (default 50, max 100)
    #[schemars(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchLocationsParams {
    /// Natural language search query, e.g. "sound stages with parking"
    pub query: String,
    /// Filter by city
    #[schemars(default)]
    pub city: Option<String>,
    /// Filter by state/region
    #[schemars(default)]
    pub state: Option<String>,
    /// Maximum number of results (default 50, max 100)
    #[schemars(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchJobsParams {
    /// Natural language search query, e.g. "camera operator jobs"
    pub query: String,
    /// Filter by location
    #[schemars(default)]
    pub location: Option<String>,
    /// Only show open jobs (default true)
    #[schemars(default)]
    pub open_only: Option<bool>,
    /// Maximum number of results (default 50, max 100)
    #[schemars(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetProfileParams {
    /// Username of the person to view
    pub username: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BrowseCreditsParams {
    /// Username of the person to browse credits for
    pub username: String,
}

// ---------------------------------------------------------------------------
// MCP Server
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SlateHubMcp {
    tool_router: ToolRouter<Self>,
    app_url: String,
}

impl SlateHubMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            app_url: crate::config::app_url(),
        }
    }
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(50).min(100)
}

#[derive(Debug, Default)]
struct ParsedQuery {
    location: Option<String>,
    gender: Option<String>,
    age_min: Option<i32>,
    age_max: Option<i32>,
    hair_color: Option<String>,
    eye_color: Option<String>,
    body_type: Option<String>,
    cleaned: String,
}

/// Parse natural language query into structured filters + cleaned search text.
/// Handles: "blonde female actors ages 20-30 in Berlin", "bald men with blue eyes in LA"
fn parse_query(query: &str) -> ParsedQuery {
    let mut cleaned = query.to_string();
    let mut parsed = ParsedQuery::default();

    // Location: "in <city/region>" at end of query (must be parsed first before other removals)
    let loc_re = Regex::new(r"(?i)\bin\s+(.+)$").unwrap();
    if let Some(caps) = loc_re.captures(&cleaned) {
        parsed.location = caps.get(1).map(|m| m.as_str().trim().to_string());
        cleaned = loc_re.replace(&cleaned, "").to_string();
    }

    // Age range: "age(s) 20-30", "ages 20 to 30", "age range 25-35"
    let age_re = Regex::new(r"(?i)\bage(?:s|\s+range)?\s+(\d+)\s*[-–to]+\s*(\d+)").unwrap();
    if let Some(caps) = age_re.captures(&cleaned) {
        parsed.age_min = caps.get(1).and_then(|m| m.as_str().parse().ok());
        parsed.age_max = caps.get(2).and_then(|m| m.as_str().parse().ok());
        cleaned = age_re.replace(&cleaned, "").to_string();
    }

    // Gender: "male", "female", "non-binary", "men", "women", "man", "woman"
    let gender_re = Regex::new(r"(?i)\b(male|female|non[- ]?binary|men|women|man|woman)\b").unwrap();
    if let Some(m) = gender_re.find(&cleaned) {
        let g = m.as_str().to_lowercase();
        parsed.gender = Some(match g.as_str() {
            "male" | "man" | "men" => "Male".to_string(),
            "female" | "woman" | "women" => "Female".to_string(),
            _ => "Non-Binary".to_string(),
        });
        cleaned = gender_re.replace(&cleaned, "").to_string();
    }

    // Hair color: "blonde hair", "brown-haired", "with red hair", "bald"
    let hair_re = Regex::new(
        r"(?i)\b(black|brown|blonde|blond|red|gray|grey|white|bald)(?:[- ]?haired|\s+hair)?\b"
    ).unwrap();
    if let Some(m) = hair_re.find(&cleaned) {
        let h = m.as_str().to_lowercase();
        parsed.hair_color = Some(match h.as_str() {
            s if s.contains("black") => "Black",
            s if s.contains("brown") => "Brown",
            s if s.contains("blond") => "Blonde",
            s if s.contains("red") => "Red",
            s if s.contains("gray") || s.contains("grey") => "Gray",
            s if s.contains("white") => "White",
            s if s.contains("bald") => "Bald",
            _ => "Other",
        }.to_string());
        cleaned = hair_re.replace(&cleaned, "").to_string();
    }

    // Eye color: "blue eyes", "brown-eyed", "with green eyes"
    let eye_re = Regex::new(
        r"(?i)\b(?:with\s+)?(brown|blue|green|hazel|gray|grey|black)(?:[- ]?eyed|\s+eyes?)\b"
    ).unwrap();
    if let Some(caps) = eye_re.captures(&cleaned) {
        let e = caps.get(1).unwrap().as_str().to_lowercase();
        parsed.eye_color = Some(match e.as_str() {
            "brown" => "Brown",
            "blue" => "Blue",
            "green" => "Green",
            "hazel" => "Hazel",
            "gray" | "grey" => "Gray",
            "black" => "Black",
            _ => "Other",
        }.to_string());
        cleaned = eye_re.replace(&cleaned, "").to_string();
    }

    // Body type: "athletic", "slim", "muscular", "petite", "plus size", "curvy"
    let body_re = Regex::new(
        r"(?i)\b(athletic|average|slim|slender|curvy|muscular|petite|plus[- ]?size|tall)\b"
    ).unwrap();
    if let Some(m) = body_re.find(&cleaned) {
        let b = m.as_str().to_lowercase();
        parsed.body_type = Some(match b.as_str() {
            "athletic" => "Athletic",
            "average" => "Average",
            "slim" => "Slim",
            "slender" => "Slender",
            "curvy" => "Curvy",
            "muscular" => "Muscular",
            "petite" => "Petite",
            s if s.contains("plus") => "Plus Size",
            "tall" => "Tall",
            _ => "Other",
        }.to_string());
        cleaned = body_re.replace(&cleaned, "").to_string();
    }

    // Clean up filler words left behind
    let filler_re = Regex::new(r"(?i)\b(with|and|who|are|is|that|the|a|an)\b").unwrap();
    cleaned = filler_re.replace_all(&cleaned, "").to_string();

    // Normalize role plurals (depluralize only, don't cross-map)
    cleaned = crate::services::search_utils::normalize_query(&cleaned);

    // Collapse whitespace
    cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    parsed.cleaned = cleaned;
    parsed
}

/// Simple location-only extraction for non-people searches.
fn extract_location(query: &str) -> (Option<String>, String) {
    let loc_re = Regex::new(r"(?i)\bin\s+(.+)$").unwrap();
    if let Some(caps) = loc_re.captures(query) {
        let location = caps.get(1).map(|m| m.as_str().trim().to_string());
        let cleaned = loc_re.replace(query, "").to_string();
        let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
        (location, cleaned)
    } else {
        (None, query.to_string())
    }
}

fn normalize_query(query: &str) -> String {
    crate::services::search_utils::normalize_query(query)
}

#[tool_router]
impl SlateHubMcp {
    /// Search for people (actors, crew, filmmakers, creators) on SlateHub.
    /// Supports natural language queries with optional hard filters for location and skill.
    /// Returns name, headline, location, skills, and profile URL.
    #[tool(
        name = "search_people",
        description = "Search for people (actors, crew, filmmakers, creators) on SlateHub. Supports natural language queries like 'cinematographers in Berlin' with optional hard filters for location and skill."
    )]
    async fn search_people(&self, Parameters(params): Parameters<SearchPeopleParams>) -> String {
        match self.do_search_people(params).await {
            Ok(result) => result,
            Err(e) => format!("Error searching people: {}", e),
        }
    }

    /// Search for film/TV productions on SlateHub.
    /// Find projects by description, location, or status.
    #[tool(
        name = "search_productions",
        description = "Search for film, TV, and content productions on SlateHub. Find projects by description, genre, location, or status."
    )]
    async fn search_productions(
        &self,
        Parameters(params): Parameters<SearchProductionsParams>,
    ) -> String {
        match self.do_search_productions(params).await {
            Ok(result) => result,
            Err(e) => format!("Error searching productions: {}", e),
        }
    }

    /// Search for organizations (studios, agencies, production companies) on SlateHub.
    #[tool(
        name = "search_organizations",
        description = "Search for organizations (studios, agencies, production companies) on SlateHub. Find companies by name, services, or location."
    )]
    async fn search_organizations(
        &self,
        Parameters(params): Parameters<SearchOrganizationsParams>,
    ) -> String {
        match self.do_search_organizations(params).await {
            Ok(result) => result,
            Err(e) => format!("Error searching organizations: {}", e),
        }
    }

    /// Search for filming locations on SlateHub.
    #[tool(
        name = "search_locations",
        description = "Search for filming locations on SlateHub. Find venues, studios, and outdoor locations by description, amenities, city, or state."
    )]
    async fn search_locations(
        &self,
        Parameters(params): Parameters<SearchLocationsParams>,
    ) -> String {
        match self.do_search_locations(params).await {
            Ok(result) => result,
            Err(e) => format!("Error searching locations: {}", e),
        }
    }

    /// Search for job postings on SlateHub.
    #[tool(
        name = "search_jobs",
        description = "Search for job postings in the film, TV, and content creation industry on SlateHub. Filter by role, location, and status."
    )]
    async fn search_jobs(&self, Parameters(params): Parameters<SearchJobsParams>) -> String {
        match self.do_search_jobs(params).await {
            Ok(result) => result,
            Err(e) => format!("Error searching jobs: {}", e),
        }
    }

    /// Get a person's full profile details on SlateHub.
    #[tool(
        name = "get_profile",
        description = "Get a person's full profile on SlateHub by username. Returns detailed info: bio, skills, physical attributes, photos, reels, social links, and credits. Use this after search_people to get full details on specific candidates."
    )]
    async fn get_profile(
        &self,
        Parameters(params): Parameters<GetProfileParams>,
    ) -> String {
        match self.do_get_profile(params).await {
            Ok(result) => result,
            Err(e) => format!("Error fetching profile: {}", e),
        }
    }

    /// Browse a person's credits and production history on SlateHub.
    #[tool(
        name = "browse_credits",
        description = "Browse a person's credits and production involvement history on SlateHub. Provide a username to see their roles across productions."
    )]
    async fn browse_credits(
        &self,
        Parameters(params): Parameters<BrowseCreditsParams>,
    ) -> String {
        match self.do_browse_credits(params).await {
            Ok(result) => result,
            Err(e) => format!("Error browsing credits: {}", e),
        }
    }
}

#[tool_handler]
impl ServerHandler for SlateHubMcp {
    fn get_info(&self) -> ServerInfo {
        use rmcp::model::{Icon, Implementation};

        let mut server_impl = Implementation::new("slatehub", env!("CARGO_PKG_VERSION"))
            .with_title("SlateHub")
            .with_description("Creative networking platform for the film, TV, and content creation industry")
            .with_icons(vec![
                Icon::new(format!("{}/favicon.svg", self.app_url))
                    .with_mime_type("image/svg+xml")
                    .with_sizes(vec!["any".to_string()]),
                Icon::new(format!("{}/apple-touch-icon.png", self.app_url))
                    .with_mime_type("image/png")
                    .with_sizes(vec!["180x180".to_string()]),
            ]);
        server_impl.website_url = Some("https://slatehub.com".to_string());

        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(server_impl)
            .with_instructions(concat!(
                "SlateHub MCP Server — read-only access to a creative networking platform for the film, TV, ",
                "and content creation industry. You have access to profiles of actors, crew, filmmakers, ",
                "and other creative professionals, plus productions, organizations, filming locations, and jobs.\n\n",
                "## Intended Use\n",
                "You are a casting and crew search assistant. Help users find talent and crew for their projects.\n\n",
                "## Search Strategy\n",
                "- search_people returns rich profile summaries including skills, physical attributes, location, and bio.\n",
                "- Use get_profile to drill into a specific person after identifying promising candidates.\n",
                "- Use browse_credits to see someone's production history and roles.\n",
                "- Results include profile photo URLs — reference these when comparing candidates visually.\n",
                "- Search returns more results than needed. Use your judgment to filter and rank for the user's specific requirements.\n",
                "- For casting queries, pay attention to: acting age range, physical attributes (height, build, hair, eyes), ",
                "location, languages, and ethnicity fields.\n",
                "- For crew queries, focus on: headline (primary role), skills, location, and experience in bio.\n\n",
                "## Workflow Example\n",
                "1. search_people with a broad query to find candidates\n",
                "2. Review the embedding_text summaries returned with each result\n",
                "3. get_profile for the most promising matches to see full details and photos\n",
                "4. browse_credits to verify experience level\n",
                "5. Present curated shortlist to the user with reasoning\n\n",
                "## Tips\n",
                "- Natural language works: 'female cinematographers in Berlin who speak German'\n",
                "- Physical attribute filters: 'tall athletic male actors ages 25-35 with brown hair'\n",
                "- The 'skill' parameter matches against headline and skills array\n",
                "- All profile URLs follow the pattern: https://slatehub.com/username\n",
                "- Photo URLs are relative paths starting with /api/media/ — prepend https://slatehub.com to make them absolute",
            ))
    }
}

// ---------------------------------------------------------------------------
// Search implementations — layered search pattern:
//   1. Hard structural filters (location, skill, status) as WHERE clauses
//   2. Soft semantic scoring via vector similarity + text matching
// ---------------------------------------------------------------------------

impl SlateHubMcp {
    async fn do_search_people(&self, params: SearchPeopleParams) -> Result<String, String> {
        let limit = clamp_limit(params.limit);

        // Parse natural language query into structured filters
        let parsed = parse_query(&params.query);
        let cleaned_query = parsed.cleaned.clone();

        // Explicit params override parsed values
        let effective_location = params.location.as_ref().or(parsed.location.as_ref());

        let query_embedding = generate_embedding_async(&cleaned_query)
            .await
            .ok();

        let mut where_clauses = Vec::new();

        if let Some(loc) = effective_location {
            let escaped = loc.replace('\'', "''");
            where_clauses.push(format!(
                "(string::lowercase(profile.location ?? '') CONTAINS string::lowercase('{escaped}') \
                 OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase('{escaped}'))"
            ));
        }

        if let Some(ref skill) = params.skill {
            where_clauses.push(format!(
                "(string::lowercase(profile.headline ?? '') CONTAINS string::lowercase('{sk}') \
                 OR '{sk_lower}' INSIDE array::map(profile.skills ?? [], |$v| string::lowercase($v)))",
                sk = skill.replace('\'', "''"),
                sk_lower = skill.to_lowercase().replace('\'', "''"),
            ));
        }

        if let Some(ref gender) = parsed.gender {
            where_clauses.push(format!(
                "string::lowercase(profile.gender ?? '') = string::lowercase('{}')",
                gender.replace('\'', "''")
            ));
        }

        if let (Some(age_min), Some(age_max)) = (parsed.age_min, parsed.age_max) {
            where_clauses.push(format!(
                "profile.acting_age_range.min <= {} AND profile.acting_age_range.max >= {}",
                age_max, age_min
            ));
        }

        if let Some(ref hair) = parsed.hair_color {
            where_clauses.push(format!(
                "string::lowercase(profile.hair_color ?? '') = string::lowercase('{}')",
                hair.replace('\'', "''")
            ));
        }

        if let Some(ref eyes) = parsed.eye_color {
            where_clauses.push(format!(
                "string::lowercase(profile.eye_color ?? '') = string::lowercase('{}')",
                eyes.replace('\'', "''")
            ));
        }

        if let Some(ref body) = parsed.body_type {
            where_clauses.push(format!(
                "string::lowercase(profile.body_type ?? '') = string::lowercase('{}')",
                body.replace('\'', "''")
            ));
        }

        let has_hard_filters = !where_clauses.is_empty();
        let hard_filter = if has_hard_filters {
            format!("AND {}", where_clauses.join(" AND "))
        } else {
            String::new()
        };

        let query_lower = cleaned_query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];
        let w = mcp_search_weights();

        // Only skip the text gate when hard filters exist AND the cleaned query is empty
        // (e.g., just physical attributes or location with no role term).
        let text_vector_gate = if has_hard_filters && query_lower.trim().is_empty() {
            "true".to_string()
        } else {
            format!("(
                    string::lowercase(name ?? '') CONTAINS $query_lower
                    OR string::lowercase(username ?? '') CONTAINS $query_lower
                    OR string::lowercase(profile.headline ?? '') CONTAINS $query_lower
                    OR string::lowercase(profile.bio ?? '') CONTAINS $query_lower
                    OR string::lowercase(profile.location ?? '') CONTAINS $query_lower
                    OR string::lowercase(embedding_text ?? '') CONTAINS $query_lower
                    OR (embedding IS NOT NONE AND $has_embedding = true
                        AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
                )", threshold = w.vector_threshold)
        };

        let sql = format!(
            "SELECT
                <string> id AS id,
                name,
                username,
                profile.headline AS headline,
                profile.location AS location,
                profile.skills AS skills,
                profile.avatar AS avatar_url,
                embedding_text,
                <float> (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                    + (IF string::lowercase(username ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                    + (IF string::lowercase(profile.headline ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                    + (IF string::lowercase(profile.location ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                        ELSE 0
                    END)
                ) AS score
            FROM person
            WHERE
                {text_vector_gate}
                {hard_filter}
            ORDER BY score DESC
            LIMIT $limit",
            w_name = w.name_match,
            w_headline = w.headline_match,
            w_location = w.location_match,
            w_vector = w.vector_multiplier,
        );

        let has_embedding = query_embedding.is_some();
        let mut response = DB
            .query(&sql)
            .bind(("query_lower", query_lower))
            .bind(("has_embedding", has_embedding))
            .bind(("query_embedding", query_embedding.unwrap_or(empty_emb)))
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "people", Some(rows.len()));

        if rows.is_empty() {
            return Ok("No people found matching your search.".to_string());
        }

        let mut out = format!("Found {} people:\n\n", rows.len());
        for row in &rows {
            let name = row["name"].as_str().unwrap_or("Unknown");
            let username = row["username"].as_str().unwrap_or("");
            let headline = row["headline"].as_str().unwrap_or("");
            let location = row["location"].as_str().unwrap_or("");
            let avatar_url = row["avatar_url"].as_str().unwrap_or("");
            let embedding_text = row["embedding_text"].as_str().unwrap_or("");
            let skills: Vec<&str> = row["skills"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            out.push_str(&format!("- **{}**", name));
            if !headline.is_empty() {
                out.push_str(&format!(" — {}", headline));
            }
            out.push('\n');
            if !location.is_empty() {
                out.push_str(&format!("  Location: {}\n", location));
            }
            if !skills.is_empty() {
                out.push_str(&format!("  Skills: {}\n", skills.join(", ")));
            }
            if !avatar_url.is_empty() {
                out.push_str(&format!("  Photo: {}{}\n", self.app_url, avatar_url));
            }
            out.push_str(&format!("  Profile: {}/{}\n", self.app_url, username));
            if !embedding_text.is_empty() {
                out.push_str(&format!("  Summary: {}\n", embedding_text));
            }
            out.push('\n');
        }
        Ok(out)
    }

    async fn do_search_productions(&self, params: SearchProductionsParams) -> Result<String, String> {
        let limit = clamp_limit(params.limit);

        let (parsed_location, cleaned_query) = if params.location.is_none() {
            extract_location(&params.query)
        } else {
            (None, params.query.clone())
        };
        let cleaned_query = normalize_query(&cleaned_query);
        let effective_location = params.location.as_ref().or(parsed_location.as_ref());

        let query_embedding = generate_embedding_async(&cleaned_query).await.ok();

        let mut where_clauses = Vec::new();

        if let Some(loc) = effective_location {
            let escaped = loc.replace('\'', "''");
            where_clauses.push(format!(
                "(string::lowercase(location ?? '') CONTAINS string::lowercase('{escaped}') \
                 OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase('{escaped}'))"
            ));
        }

        if let Some(ref status) = params.status {
            where_clauses.push(format!(
                "string::lowercase(status ?? '') = string::lowercase('{}')",
                status.replace('\'', "''")
            ));
        }

        let has_hard_filters = !where_clauses.is_empty();
        let hard_filter = if has_hard_filters {
            format!("AND {}", where_clauses.join(" AND "))
        } else {
            String::new()
        };

        let query_lower = cleaned_query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];
        let w = mcp_search_weights();

        let text_vector_gate = if has_hard_filters && query_lower.trim().is_empty() {
            "true".to_string()
        } else {
            format!("(
                    string::lowercase(title ?? '') CONTAINS $query_lower
                    OR string::lowercase(description ?? '') CONTAINS $query_lower
                    OR string::lowercase(location ?? '') CONTAINS $query_lower
                    OR (embedding IS NOT NONE AND $has_embedding = true
                        AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
                )", threshold = w.vector_threshold)
        };

        let sql = format!(
            "SELECT
                <string> id AS id,
                title,
                slug,
                status,
                description,
                location,
                poster_photo,
                header_photo,
                embedding_text,
                <float> (
                    (IF string::lowercase(title ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                        ELSE 0
                    END)
                ) AS score
            FROM production
            WHERE
                {text_vector_gate}
                {hard_filter}
            ORDER BY score DESC
            LIMIT $limit",
            w_name = w.name_match,
            w_headline = w.headline_match,
            w_location = w.location_match,
            w_vector = w.vector_multiplier,
        );

        let has_embedding = query_embedding.is_some();
        let mut response = DB
            .query(&sql)
            .bind(("query_lower", query_lower))
            .bind(("has_embedding", has_embedding))
            .bind(("query_embedding", query_embedding.unwrap_or(empty_emb)))
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "productions", Some(rows.len()));

        if rows.is_empty() {
            return Ok("No productions found matching your search.".to_string());
        }

        let mut out = format!("Found {} productions:\n\n", rows.len());
        for row in &rows {
            let title = row["title"].as_str().unwrap_or("Untitled");
            let slug = row["slug"].as_str().unwrap_or("");
            let status = row["status"].as_str().unwrap_or("");
            let location = row["location"].as_str().unwrap_or("");
            let description = row["description"].as_str().unwrap_or("");

            out.push_str(&format!("- **{}**", title));
            if !status.is_empty() {
                out.push_str(&format!(" [{}]", status));
            }
            out.push('\n');
            if !location.is_empty() {
                out.push_str(&format!("  Location: {}\n", location));
            }
            if !description.is_empty() {
                let desc: String = description.chars().take(200).collect();
                let desc = if desc.len() < description.len() {
                    format!("{}...", desc)
                } else {
                    desc
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            out.push_str(&format!(
                "  URL: {}/productions/{}\n",
                self.app_url, slug
            ));
            out.push('\n');
        }
        Ok(out)
    }

    async fn do_search_organizations(&self, params: SearchOrganizationsParams) -> Result<String, String> {
        let limit = clamp_limit(params.limit);

        let (parsed_location, cleaned_query) = if params.location.is_none() {
            extract_location(&params.query)
        } else {
            (None, params.query.clone())
        };
        let cleaned_query = normalize_query(&cleaned_query);
        let effective_location = params.location.as_ref().or(parsed_location.as_ref());

        let query_embedding = generate_embedding_async(&cleaned_query).await.ok();

        let has_hard_filters = effective_location.is_some();
        let hard_filter = if let Some(loc) = effective_location {
            let escaped = loc.replace('\'', "''");
            format!(
                "AND (string::lowercase(location ?? '') CONTAINS string::lowercase('{escaped}') \
                 OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase('{escaped}'))"
            )
        } else {
            String::new()
        };

        let query_lower = cleaned_query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];
        let w = mcp_search_weights();

        let text_vector_gate = if has_hard_filters && query_lower.trim().is_empty() {
            "true".to_string()
        } else {
            format!("(
                    string::lowercase(name ?? '') CONTAINS $query_lower
                    OR string::lowercase(description ?? '') CONTAINS $query_lower
                    OR string::lowercase(location ?? '') CONTAINS $query_lower
                    OR (embedding IS NOT NONE AND $has_embedding = true
                        AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
                )", threshold = w.vector_threshold)
        };

        let sql = format!(
            "SELECT
                <string> id AS id,
                name,
                slug,
                description,
                location,
                logo,
                embedding_text,
                <float> (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                        ELSE 0
                    END)
                ) AS score
            FROM organization
            WHERE
                {text_vector_gate}
                {hard_filter}
            ORDER BY score DESC
            LIMIT $limit",
            w_name = w.name_match,
            w_headline = w.headline_match,
            w_location = w.location_match,
            w_vector = w.vector_multiplier,
        );

        let has_embedding = query_embedding.is_some();
        let mut response = DB
            .query(&sql)
            .bind(("query_lower", query_lower))
            .bind(("has_embedding", has_embedding))
            .bind(("query_embedding", query_embedding.unwrap_or(empty_emb)))
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "organizations", Some(rows.len()));

        if rows.is_empty() {
            return Ok("No organizations found matching your search.".to_string());
        }

        let mut out = format!("Found {} organizations:\n\n", rows.len());
        for row in &rows {
            let name = row["name"].as_str().unwrap_or("Unknown");
            let slug = row["slug"].as_str().unwrap_or("");
            let description = row["description"].as_str().unwrap_or("");
            let location = row["location"].as_str().unwrap_or("");

            out.push_str(&format!("- **{}**\n", name));
            if !location.is_empty() {
                out.push_str(&format!("  Location: {}\n", location));
            }
            if !description.is_empty() {
                let desc: String = description.chars().take(200).collect();
                let desc = if desc.len() < description.len() {
                    format!("{}...", desc)
                } else {
                    desc
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            out.push_str(&format!("  URL: {}/orgs/{}\n", self.app_url, slug));
            out.push('\n');
        }
        Ok(out)
    }

    async fn do_search_locations(&self, params: SearchLocationsParams) -> Result<String, String> {
        let limit = clamp_limit(params.limit);

        let (parsed_city, cleaned_query) = if params.city.is_none() {
            extract_location(&params.query)
        } else {
            (None, params.query.clone())
        };
        let cleaned_query = normalize_query(&cleaned_query);
        let effective_city = params.city.as_ref().or(parsed_city.as_ref());

        let query_embedding = generate_embedding_async(&cleaned_query).await.ok();

        let mut where_clauses = Vec::new();

        if let Some(city) = effective_city {
            where_clauses.push(format!(
                "string::lowercase(city ?? '') CONTAINS string::lowercase('{}')",
                city.replace('\'', "''")
            ));
        }

        if let Some(ref state) = params.state {
            where_clauses.push(format!(
                "string::lowercase(state ?? '') CONTAINS string::lowercase('{}')",
                state.replace('\'', "''")
            ));
        }

        let has_hard_filters = !where_clauses.is_empty();
        let hard_filter = if has_hard_filters {
            format!("AND {}", where_clauses.join(" AND "))
        } else {
            String::new()
        };

        let query_lower = cleaned_query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];
        let w = mcp_search_weights();

        let text_vector_gate = if has_hard_filters && query_lower.trim().is_empty() {
            "true".to_string()
        } else {
            format!("(
                string::lowercase(name ?? '') CONTAINS $query_lower
                OR string::lowercase(city ?? '') CONTAINS $query_lower
                OR string::lowercase(state ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
            )", threshold = w.vector_threshold)
        };

        let sql = format!(
            "SELECT
                <string> id AS id,
                name,
                <string> meta::id(id) AS key,
                address,
                city,
                state,
                description,
                profile_photo,
                embedding_text,
                <float> (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                    + (IF string::lowercase(city ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                    + (IF string::lowercase(state ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                        ELSE 0
                    END)
                ) AS score
            FROM location
            WHERE is_public = true AND {text_vector_gate}
            {hard_filter}
            ORDER BY score DESC
            LIMIT $limit",
            w_name = w.name_match,
            w_headline = w.headline_match,
            w_location = w.location_match,
            w_vector = w.vector_multiplier,
        );

        let has_embedding = query_embedding.is_some();
        let mut response = DB
            .query(&sql)
            .bind(("query_lower", query_lower))
            .bind(("has_embedding", has_embedding))
            .bind(("query_embedding", query_embedding.unwrap_or(empty_emb)))
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "locations", Some(rows.len()));

        if rows.is_empty() {
            return Ok("No locations found matching your search.".to_string());
        }

        let mut out = format!("Found {} locations:\n\n", rows.len());
        for row in &rows {
            let name = row["name"].as_str().unwrap_or("Unknown");
            let key = row["key"].as_str().unwrap_or("");
            let city = row["city"].as_str().unwrap_or("");
            let state = row["state"].as_str().unwrap_or("");
            let description = row["description"].as_str().unwrap_or("");

            out.push_str(&format!("- **{}**\n", name));
            if !city.is_empty() || !state.is_empty() {
                out.push_str(&format!(
                    "  Location: {}\n",
                    [city, state]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !description.is_empty() {
                let desc: String = description.chars().take(200).collect();
                let desc = if desc.len() < description.len() {
                    format!("{}...", desc)
                } else {
                    desc
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            out.push_str(&format!(
                "  URL: {}/locations/{}\n",
                self.app_url, key
            ));
            out.push('\n');
        }
        Ok(out)
    }

    async fn do_search_jobs(&self, params: SearchJobsParams) -> Result<String, String> {
        let limit = clamp_limit(params.limit);

        let (parsed_location, cleaned_query) = if params.location.is_none() {
            extract_location(&params.query)
        } else {
            (None, params.query.clone())
        };
        let cleaned_query = normalize_query(&cleaned_query);
        let effective_location = params.location.as_ref().or(parsed_location.as_ref());

        let query_embedding = generate_embedding_async(&cleaned_query).await.ok();

        let open_only = params.open_only.unwrap_or(true);

        let mut where_clauses = Vec::new();

        if open_only {
            where_clauses.push("status = 'open'".to_string());
        }

        if let Some(loc) = effective_location {
            let escaped = loc.replace('\'', "''");
            where_clauses.push(format!(
                "(string::lowercase(location ?? '') CONTAINS string::lowercase('{escaped}') \
                 OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase('{escaped}'))"
            ));
        }

        let has_hard_filters = !where_clauses.is_empty();
        let hard_filter = if has_hard_filters {
            format!("AND {}", where_clauses.join(" AND "))
        } else {
            String::new()
        };

        let query_lower = cleaned_query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];
        let w = mcp_search_weights();

        let text_vector_gate = if has_hard_filters && query_lower.trim().is_empty() {
            "true".to_string()
        } else {
            format!("(
                    string::lowercase(title ?? '') CONTAINS $query_lower
                    OR string::lowercase(description ?? '') CONTAINS $query_lower
                    OR string::lowercase(location ?? '') CONTAINS $query_lower
                    OR (embedding IS NOT NONE AND $has_embedding = true
                        AND vector::similarity::cosine(embedding, $query_embedding) > {threshold})
                )", threshold = w.vector_threshold)
        };

        let sql = format!(
            "SELECT
                <string> id AS id,
                <string> meta::id(id) AS key,
                title,
                description,
                location,
                status,
                embedding_text,
                <float> (
                    (IF string::lowercase(title ?? '') CONTAINS $query_lower THEN {w_name} ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN {w_headline} ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN {w_location} ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * {w_vector}
                        ELSE 0
                    END)
                ) AS score
            FROM job_posting
            WHERE
                {text_vector_gate}
                {hard_filter}
            ORDER BY score DESC
            LIMIT $limit",
            w_name = w.name_match,
            w_headline = w.headline_match,
            w_location = w.location_match,
            w_vector = w.vector_multiplier,
        );

        let has_embedding = query_embedding.is_some();
        let mut response = DB
            .query(&sql)
            .bind(("query_lower", query_lower))
            .bind(("has_embedding", has_embedding))
            .bind(("query_embedding", query_embedding.unwrap_or(empty_emb)))
            .bind(("limit", limit as i64))
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "jobs", Some(rows.len()));

        if rows.is_empty() {
            return Ok("No job postings found matching your search.".to_string());
        }

        let mut out = format!("Found {} job postings:\n\n", rows.len());
        for row in &rows {
            let title = row["title"].as_str().unwrap_or("Untitled");
            let key = row["key"].as_str().unwrap_or("");
            let location = row["location"].as_str().unwrap_or("");
            let status = row["status"].as_str().unwrap_or("");
            let description = row["description"].as_str().unwrap_or("");

            out.push_str(&format!("- **{}**", title));
            if !status.is_empty() {
                out.push_str(&format!(" [{}]", status));
            }
            out.push('\n');
            if !location.is_empty() {
                out.push_str(&format!("  Location: {}\n", location));
            }
            if !description.is_empty() {
                let desc: String = description.chars().take(200).collect();
                let desc = if desc.len() < description.len() {
                    format!("{}...", desc)
                } else {
                    desc
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            out.push_str(&format!("  URL: {}/jobs/{}\n", self.app_url, key));
            out.push('\n');
        }
        Ok(out)
    }

    async fn do_get_profile(&self, params: GetProfileParams) -> Result<String, String> {
        let username = params.username.trim().to_string();

        let mut response = DB
            .query(
                "SELECT
                    <string> id AS id,
                    name,
                    username,
                    profile.headline AS headline,
                    profile.bio AS bio,
                    profile.location AS location,
                    profile.skills AS skills,
                    profile.avatar AS avatar_url,
                    profile.photos AS photos,
                    profile.gender AS gender,
                    profile.height_mm AS height_mm,
                    profile.body_type AS body_type,
                    profile.hair_color AS hair_color,
                    profile.eye_color AS eye_color,
                    profile.ethnicity AS ethnicity,
                    profile.acting_age_range AS acting_age_range,
                    profile.acting_ethnicities AS acting_ethnicities,
                    profile.nationality AS nationality,
                    profile.languages AS languages,
                    profile.unions AS unions,
                    profile.availability AS availability,
                    profile.website AS website,
                    profile.reels AS reels,
                    profile.social_links AS social_links,
                    embedding_text
                FROM person WHERE username = $username LIMIT 1"
            )
            .bind(("username", username.clone()))
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<serde_json::Value> = response.take(0).map_err(|e| e.to_string())?;
        let row = match rows.first() {
            Some(r) => r,
            None => return Ok(format!("No person found with username '{}'.", username)),
        };

        let name = row["name"].as_str().unwrap_or("Unknown");
        let mut out = format!("# {} (@{})\n\n", name, username);

        if let Some(h) = row["headline"].as_str().filter(|s| !s.is_empty()) {
            out.push_str(&format!("**{}**\n\n", h));
        }

        // Photo
        if let Some(avatar) = row["avatar_url"].as_str().filter(|s| !s.is_empty()) {
            out.push_str(&format!("Profile photo: {}{}\n", self.app_url, avatar));
        }

        // Basic info
        if let Some(loc) = row["location"].as_str().filter(|s| !s.is_empty()) {
            out.push_str(&format!("Location: {}\n", loc));
        }
        if let Some(avail) = row["availability"].as_str().filter(|s| !s.is_empty()) {
            out.push_str(&format!("Availability: {}\n", avail));
        }
        if let Some(nat) = row["nationality"].as_str().filter(|s| !s.is_empty()) {
            out.push_str(&format!("Nationality: {}\n", nat));
        }
        if let Some(g) = row["gender"].as_str().filter(|s| !s.is_empty()) {
            out.push_str(&format!("Gender: {}\n", g));
        }
        out.push('\n');

        // Physical attributes
        let mut phys = Vec::new();
        if let Some(h) = row["height_mm"].as_i64() {
            let cm = h;
            let feet = cm / 30;
            let inches = (cm % 30) * 12 / 30;
            phys.push(format!("Height: {}cm ({}'{}\")", cm, feet, inches));
        }
        if let Some(bt) = row["body_type"].as_str().filter(|s| !s.is_empty()) {
            phys.push(format!("Build: {}", bt));
        }
        if let Some(hc) = row["hair_color"].as_str().filter(|s| !s.is_empty()) {
            phys.push(format!("Hair: {}", hc));
        }
        if let Some(ec) = row["eye_color"].as_str().filter(|s| !s.is_empty()) {
            phys.push(format!("Eyes: {}", ec));
        }
        if !phys.is_empty() {
            out.push_str(&format!("**Physical:** {}\n", phys.join(" | ")));
        }

        // Ethnicity
        if let Some(eth) = row["ethnicity"].as_array() {
            let vals: Vec<&str> = eth.iter().filter_map(|v| v.as_str()).collect();
            if !vals.is_empty() {
                out.push_str(&format!("Ethnicity: {}\n", vals.join(", ")));
            }
        }

        // Acting range
        if let Some(aar) = row["acting_age_range"].as_object() {
            if let (Some(min), Some(max)) = (aar.get("min").and_then(|v| v.as_i64()), aar.get("max").and_then(|v| v.as_i64())) {
                out.push_str(&format!("Can play ages: {}-{}\n", min, max));
            }
        }
        if let Some(ae) = row["acting_ethnicities"].as_array() {
            let vals: Vec<&str> = ae.iter().filter_map(|v| v.as_str()).collect();
            if !vals.is_empty() {
                out.push_str(&format!("Can portray: {}\n", vals.join(", ")));
            }
        }
        out.push('\n');

        // Skills
        if let Some(skills) = row["skills"].as_array() {
            let vals: Vec<&str> = skills.iter().filter_map(|v| v.as_str()).collect();
            if !vals.is_empty() {
                out.push_str(&format!("**Skills:** {}\n", vals.join(", ")));
            }
        }

        // Languages
        if let Some(langs) = row["languages"].as_array() {
            let vals: Vec<&str> = langs.iter().filter_map(|v| v.as_str()).collect();
            if !vals.is_empty() {
                out.push_str(&format!("**Languages:** {}\n", vals.join(", ")));
            }
        }

        // Unions
        if let Some(unions) = row["unions"].as_array() {
            let vals: Vec<&str> = unions.iter().filter_map(|v| v.as_str()).collect();
            if !vals.is_empty() {
                out.push_str(&format!("**Unions:** {}\n", vals.join(", ")));
            }
        }
        out.push('\n');

        // Bio
        if let Some(bio) = row["bio"].as_str().filter(|s| !s.is_empty()) {
            out.push_str(&format!("**Bio:**\n{}\n\n", bio));
        }

        // Gallery photos
        if let Some(photos) = row["photos"].as_array() {
            if !photos.is_empty() {
                out.push_str("**Gallery:**\n");
                for photo in photos {
                    if let Some(url) = photo["url"].as_str() {
                        let caption = photo["caption"].as_str().unwrap_or("");
                        if !caption.is_empty() {
                            out.push_str(&format!("- {}{} ({})\n", self.app_url, url, caption));
                        } else {
                            out.push_str(&format!("- {}{}\n", self.app_url, url));
                        }
                    }
                }
                out.push('\n');
            }
        }

        // Reels
        if let Some(reels) = row["reels"].as_array() {
            if !reels.is_empty() {
                out.push_str("**Reels:**\n");
                for reel in reels {
                    let title = reel["title"].as_str().unwrap_or("Untitled");
                    let url = reel["url"].as_str().unwrap_or("");
                    if !url.is_empty() {
                        out.push_str(&format!("- {} — {}\n", title, url));
                    }
                }
                out.push('\n');
            }
        }

        // Social links
        if let Some(links) = row["social_links"].as_array() {
            if !links.is_empty() {
                out.push_str("**Links:**\n");
                for link in links {
                    let platform = link["platform"].as_str().unwrap_or("link");
                    let url = link["url"].as_str().unwrap_or("");
                    if !url.is_empty() {
                        out.push_str(&format!("- {}: {}\n", platform, url));
                    }
                }
                out.push('\n');
            }
        }

        if let Some(website) = row["website"].as_str().filter(|s| !s.is_empty()) {
            out.push_str(&format!("Website: {}\n", website));
        }

        out.push_str(&format!("\nProfile URL: {}/{}\n", self.app_url, username));

        Ok(out)
    }

    async fn do_browse_credits(&self, params: BrowseCreditsParams) -> Result<String, String> {
        let username = params.username.trim().to_string();

        // First get the person's name and basic info
        let mut response = DB
            .query(
                "SELECT <string> id AS id, name, username, profile.headline AS headline, profile.location AS location \
                 FROM person WHERE username = $username LIMIT 1"
            )
            .bind(("username", username.clone()))
            .await
            .map_err(|e| e.to_string())?;

        let person: Option<serde_json::Value> = response.take(0).map_err(|e| e.to_string())?;

        let person = match person {
            Some(p) => p,
            None => return Ok(format!("No person found with username '{}'.", username)),
        };

        let name = person["name"].as_str().unwrap_or(&username);
        let headline = person["headline"].as_str().unwrap_or("");
        let location = person["location"].as_str().unwrap_or("");
        let person_id = person["id"].as_str().unwrap_or("");

        // Get their production involvements via the involvement relation
        // involvement is: person -> involvement -> production, with role field
        let credits_sql = format!(
            "SELECT \
                <string> out AS production_id, \
                out.title AS title, \
                out.slug AS slug, \
                out.status AS status, \
                role \
             FROM involvement WHERE in = {person_id} ORDER BY out.start_date DESC"
        );

        let mut cred_response = DB
            .query(&credits_sql)
            .await
            .map_err(|e| e.to_string())?;

        let credits: Vec<serde_json::Value> =
            cred_response.take(0).map_err(|e| e.to_string())?;

        let mut out = format!("**{}**", name);
        if !headline.is_empty() {
            out.push_str(&format!(" — {}", headline));
        }
        out.push('\n');
        if !location.is_empty() {
            out.push_str(&format!("Location: {}\n", location));
        }
        out.push_str(&format!(
            "Profile: {}/{}\n\n",
            self.app_url, username
        ));

        if credits.is_empty() {
            out.push_str("No production credits found.");
        } else {
            out.push_str(&format!("**Credits ({}):**\n\n", credits.len()));
            for credit in &credits {
                let title = credit["title"].as_str().unwrap_or("Untitled");
                let slug = credit["slug"].as_str().unwrap_or("");
                let role = credit["role"].as_str().unwrap_or("Unknown Role");
                let status = credit["status"].as_str().unwrap_or("");

                out.push_str(&format!("- **{}** as {}", title, role));
                if !status.is_empty() {
                    out.push_str(&format!(" [{}]", status));
                }
                out.push('\n');
                if !slug.is_empty() {
                    out.push_str(&format!(
                        "  URL: {}/productions/{}\n",
                        self.app_url, slug
                    ));
                }
            }
        }

        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Create the MCP service for mounting in Axum
// ---------------------------------------------------------------------------

pub fn create_mcp_service() -> StreamableHttpService<SlateHubMcp, LocalSessionManager> {
    let ct = CancellationToken::new();

    StreamableHttpService::new(
        || Ok(SlateHubMcp::new()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig {
            stateful_mode: false,
            json_response: true,
            sse_keep_alive: Some(Duration::from_secs(15)),
            cancellation_token: ct,
            ..Default::default()
        },
    )
}
