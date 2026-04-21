use std::sync::Arc;
use std::time::Duration;

use crate::config::mcp_search_weights;
use crate::db::DB;
use crate::services::embedding::generate_embedding_async;
use crate::services::search::{
    SearchParams, search_jobs as svc_search_jobs, search_locations as svc_search_locations,
    search_organizations as svc_search_organizations, search_people as svc_search_people,
    search_productions as svc_search_productions,
};
use crate::services::search_log::log_search;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

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

impl Default for SlateHubMcp {
    fn default() -> Self {
        Self::new()
    }
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

use crate::services::search_utils::{extract_location, normalize_query, parse_query};

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
    async fn get_profile(&self, Parameters(params): Parameters<GetProfileParams>) -> String {
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
    async fn browse_credits(&self, Parameters(params): Parameters<BrowseCreditsParams>) -> String {
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
            .with_description(
                "Creative networking platform for the film, TV, and content creation industry",
            )
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
        let mut parsed = parse_query(&params.query);

        // Explicit MCP params override parsed values
        if let Some(ref loc) = params.location {
            parsed.location = Some(loc.clone());
        }

        let cleaned_query = parsed.cleaned.clone();
        let query_embedding = generate_embedding_async(&cleaned_query).await.ok();
        let weights = mcp_search_weights();

        let search_params = SearchParams {
            query: &cleaned_query,
            embedding: query_embedding.as_ref(),
            weights,
            limit,
            offset: 0,
        };

        let results = svc_search_people(&search_params, &parsed, params.skill.as_deref())
            .await
            .map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "people", Some(results.len()));

        if results.is_empty() {
            return Ok("No people found matching your search.".to_string());
        }

        let mut out = format!("Found {} people:\n\n", results.len());
        for r in &results {
            out.push_str(&format!("- **{}**", r.name));
            if let Some(ref headline) = r.headline
                && !headline.is_empty()
            {
                out.push_str(&format!(" — {}", headline));
            }
            out.push('\n');
            if let Some(ref location) = r.location
                && !location.is_empty()
            {
                out.push_str(&format!("  Location: {}\n", location));
            }
            if !r.skills.is_empty() {
                out.push_str(&format!("  Skills: {}\n", r.skills.join(", ")));
            }
            if let Some(ref avatar_url) = r.avatar_url
                && !avatar_url.is_empty()
            {
                out.push_str(&format!("  Photo: {}{}\n", self.app_url, avatar_url));
            }
            out.push_str(&format!("  Profile: {}/{}\n", self.app_url, r.username));
            if let Some(ref embedding_text) = r.embedding_text
                && !embedding_text.is_empty()
            {
                out.push_str(&format!("  Summary: {}\n", embedding_text));
            }
            out.push('\n');
        }
        Ok(out)
    }

    async fn do_search_productions(
        &self,
        params: SearchProductionsParams,
    ) -> Result<String, String> {
        let limit = clamp_limit(params.limit);

        // Don't extract location — canonical search_productions has no location param.
        // The location terms stay in the query for text/vector matching.
        let cleaned_query = normalize_query(&params.query);

        let query_embedding = generate_embedding_async(&cleaned_query).await.ok();
        let weights = mcp_search_weights();

        let search_params = SearchParams {
            query: &cleaned_query,
            embedding: query_embedding.as_ref(),
            weights,
            limit,
            offset: 0,
        };

        let results = svc_search_productions(&search_params, params.status.as_deref())
            .await
            .map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "productions", Some(results.len()));

        if results.is_empty() {
            return Ok("No productions found matching your search.".to_string());
        }

        let mut out = format!("Found {} productions:\n\n", results.len());
        for r in &results {
            out.push_str(&format!("- **{}**", r.title));
            if !r.status.is_empty() {
                out.push_str(&format!(" [{}]", r.status));
            }
            out.push('\n');
            if let Some(ref location) = r.location
                && !location.is_empty()
            {
                out.push_str(&format!("  Location: {}\n", location));
            }
            if let Some(ref description) = r.description
                && !description.is_empty()
            {
                let desc: String = description.chars().take(200).collect();
                let desc = if desc.len() < description.len() {
                    format!("{}...", desc)
                } else {
                    desc
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            out.push_str(&format!("  URL: {}/productions/{}\n", self.app_url, r.slug));
            out.push('\n');
        }
        Ok(out)
    }

    async fn do_search_organizations(
        &self,
        params: SearchOrganizationsParams,
    ) -> Result<String, String> {
        let limit = clamp_limit(params.limit);

        let (parsed_location, cleaned_query) = if params.location.is_none() {
            extract_location(&params.query)
        } else {
            (None, params.query.clone())
        };
        let cleaned_query = normalize_query(&cleaned_query);
        let effective_location = params.location.as_ref().or(parsed_location.as_ref());

        let query_embedding = generate_embedding_async(&cleaned_query).await.ok();
        let weights = mcp_search_weights();

        let search_params = SearchParams {
            query: &cleaned_query,
            embedding: query_embedding.as_ref(),
            weights,
            limit,
            offset: 0,
        };

        let results =
            svc_search_organizations(&search_params, effective_location.map(|s| s.as_str()))
                .await
                .map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "organizations", Some(results.len()));

        if results.is_empty() {
            return Ok("No organizations found matching your search.".to_string());
        }

        let mut out = format!("Found {} organizations:\n\n", results.len());
        for r in &results {
            out.push_str(&format!("- **{}**\n", r.name));
            if let Some(ref location) = r.location
                && !location.is_empty()
            {
                out.push_str(&format!("  Location: {}\n", location));
            }
            if let Some(ref description) = r.description
                && !description.is_empty()
            {
                let desc: String = description.chars().take(200).collect();
                let desc = if desc.len() < description.len() {
                    format!("{}...", desc)
                } else {
                    desc
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            out.push_str(&format!("  URL: {}/orgs/{}\n", self.app_url, r.slug));
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
        let weights = mcp_search_weights();

        let search_params = SearchParams {
            query: &cleaned_query,
            embedding: query_embedding.as_ref(),
            weights,
            limit,
            offset: 0,
        };

        let results = svc_search_locations(
            &search_params,
            effective_city.map(|s| s.as_str()),
            params.state.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "locations", Some(results.len()));

        if results.is_empty() {
            return Ok("No locations found matching your search.".to_string());
        }

        let mut out = format!("Found {} locations:\n\n", results.len());
        for r in &results {
            out.push_str(&format!("- **{}**\n", r.name));
            let city = &r.city;
            let state = &r.state;
            if !city.is_empty() || !state.is_empty() {
                out.push_str(&format!(
                    "  Location: {}\n",
                    [city.as_str(), state.as_str()]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if let Some(ref description) = r.description
                && !description.is_empty()
            {
                let desc: String = description.chars().take(200).collect();
                let desc = if desc.len() < description.len() {
                    format!("{}...", desc)
                } else {
                    desc
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            out.push_str(&format!("  URL: {}/locations/{}\n", self.app_url, r.key));
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
        let weights = mcp_search_weights();
        let open_only = params.open_only.unwrap_or(true);

        let search_params = SearchParams {
            query: &cleaned_query,
            embedding: query_embedding.as_ref(),
            weights,
            limit,
            offset: 0,
        };

        let results = svc_search_jobs(
            &search_params,
            effective_location.map(|s| s.as_str()),
            open_only,
        )
        .await
        .map_err(|e| e.to_string())?;

        log_search(&params.query, "mcp", "jobs", Some(results.len()));

        if results.is_empty() {
            return Ok("No job postings found matching your search.".to_string());
        }

        let mut out = format!("Found {} job postings:\n\n", results.len());
        for r in &results {
            out.push_str(&format!("- **{}**", r.title));
            out.push('\n');
            if let Some(ref location) = r.location
                && !location.is_empty()
            {
                out.push_str(&format!("  Location: {}\n", location));
            }
            if !r.poster_name.is_empty() {
                out.push_str(&format!(
                    "  Posted by: {} ({})\n",
                    r.poster_name, r.poster_type
                ));
            }
            if r.role_count > 0 {
                out.push_str(&format!("  Roles: {}\n", r.role_count));
            }
            if !r.description.is_empty() {
                let desc: String = r.description.chars().take(200).collect();
                let desc = if desc.len() < r.description.len() {
                    format!("{}...", desc)
                } else {
                    desc
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            if let Some(ref embedding_text) = r.embedding_text
                && !embedding_text.is_empty()
            {
                out.push_str(&format!("  Summary: {}\n", embedding_text));
            }
            // Extract key from id (format: "job_posting:key")
            let key = r.id.strip_prefix("job_posting:").unwrap_or(&r.id);
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
                FROM person WHERE username = $username LIMIT 1",
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
        if let Some(aar) = row["acting_age_range"].as_object()
            && let (Some(min), Some(max)) = (
                aar.get("min").and_then(|v| v.as_i64()),
                aar.get("max").and_then(|v| v.as_i64()),
            )
        {
            out.push_str(&format!("Can play ages: {}-{}\n", min, max));
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
        if let Some(photos) = row["photos"].as_array()
            && !photos.is_empty()
        {
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

        // Reels
        if let Some(reels) = row["reels"].as_array()
            && !reels.is_empty()
        {
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

        // Social links
        if let Some(links) = row["social_links"].as_array()
            && !links.is_empty()
        {
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

        let mut cred_response = DB.query(&credits_sql).await.map_err(|e| e.to_string())?;

        let credits: Vec<serde_json::Value> = cred_response.take(0).map_err(|e| e.to_string())?;

        let mut out = format!("**{}**", name);
        if !headline.is_empty() {
            out.push_str(&format!(" — {}", headline));
        }
        out.push('\n');
        if !location.is_empty() {
            out.push_str(&format!("Location: {}\n", location));
        }
        out.push_str(&format!("Profile: {}/{}\n\n", self.app_url, username));

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
                    out.push_str(&format!("  URL: {}/productions/{}\n", self.app_url, slug));
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
