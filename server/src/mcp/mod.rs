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
use crate::db::DB;
use crate::services::embedding::generate_embedding_async;

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
    /// Maximum number of results (default 20, max 50)
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
    /// Maximum number of results (default 20, max 50)
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
    /// Maximum number of results (default 20, max 50)
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
    /// Maximum number of results (default 20, max 50)
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
    /// Maximum number of results (default 20, max 50)
    #[schemars(default)]
    pub limit: Option<usize>,
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
    limit.unwrap_or(20).min(50)
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
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "SlateHub MCP Server — read-only access to the SlateHub creative networking platform. \
                 Search for people (actors, crew, filmmakers), productions, organizations, filming locations, \
                 and job postings in the film, TV, and content creation industry. \
                 Use the search tools with natural language queries. \
                 Hard filters (location, skill, status) are applied as WHERE clauses for precision, \
                 while the query text drives semantic similarity scoring."
            )
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
        let query_embedding = generate_embedding_async(&params.query)
            .await
            .ok();

        let mut where_clauses = Vec::new();

        if let Some(ref loc) = params.location {
            where_clauses.push(format!(
                "string::lowercase(profile.location ?? '') CONTAINS string::lowercase('{}')",
                loc.replace('\'', "''")
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

        let hard_filter = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("AND {}", where_clauses.join(" AND "))
        };

        let query_lower = params.query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];

        let sql = format!(
            "SELECT
                <string> id AS id,
                name,
                username,
                profile.headline AS headline,
                profile.location AS location,
                profile.skills AS skills,
                profile.avatar AS avatar_url,
                <float> (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(username ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(profile.headline ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF string::lowercase(profile.location ?? '') CONTAINS $query_lower THEN 10 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 50
                        ELSE 0
                    END)
                ) AS score
            FROM person
            WHERE
                (
                    string::lowercase(name ?? '') CONTAINS $query_lower
                    OR string::lowercase(username ?? '') CONTAINS $query_lower
                    OR string::lowercase(profile.headline ?? '') CONTAINS $query_lower
                    OR string::lowercase(profile.bio ?? '') CONTAINS $query_lower
                    OR string::lowercase(profile.location ?? '') CONTAINS $query_lower
                    OR (embedding IS NOT NONE AND $has_embedding = true
                        AND vector::similarity::cosine(embedding, $query_embedding) > 0.45)
                )
                {hard_filter}
            ORDER BY score DESC
            LIMIT $limit"
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

        if rows.is_empty() {
            return Ok("No people found matching your search.".to_string());
        }

        let mut out = format!("Found {} people:\n\n", rows.len());
        for row in &rows {
            let name = row["name"].as_str().unwrap_or("Unknown");
            let username = row["username"].as_str().unwrap_or("");
            let headline = row["headline"].as_str().unwrap_or("");
            let location = row["location"].as_str().unwrap_or("");
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
            out.push_str(&format!("  Profile: {}/{}\n", self.app_url, username));
            out.push('\n');
        }
        Ok(out)
    }

    async fn do_search_productions(&self, params: SearchProductionsParams) -> Result<String, String> {
        let limit = clamp_limit(params.limit);
        let query_embedding = generate_embedding_async(&params.query).await.ok();

        let mut where_clauses = Vec::new();

        if let Some(ref loc) = params.location {
            where_clauses.push(format!(
                "string::lowercase(location ?? '') CONTAINS string::lowercase('{}')",
                loc.replace('\'', "''")
            ));
        }

        if let Some(ref status) = params.status {
            where_clauses.push(format!(
                "string::lowercase(status ?? '') = string::lowercase('{}')",
                status.replace('\'', "''")
            ));
        }

        let hard_filter = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("AND {}", where_clauses.join(" AND "))
        };

        let query_lower = params.query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];

        let sql = format!(
            "SELECT
                <string> id AS id,
                title,
                slug,
                status,
                description,
                location,
                <float> (
                    (IF string::lowercase(title ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS score
            FROM production
            WHERE
                (
                    string::lowercase(title ?? '') CONTAINS $query_lower
                    OR string::lowercase(description ?? '') CONTAINS $query_lower
                    OR string::lowercase(location ?? '') CONTAINS $query_lower
                    OR (embedding IS NOT NONE AND $has_embedding = true
                        AND vector::similarity::cosine(embedding, $query_embedding) > 0.45)
                )
                {hard_filter}
            ORDER BY score DESC
            LIMIT $limit"
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
                let desc = if description.len() > 200 {
                    format!("{}...", &description[..200])
                } else {
                    description.to_string()
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
        let query_embedding = generate_embedding_async(&params.query).await.ok();

        let hard_filter = if let Some(ref loc) = params.location {
            format!(
                "AND string::lowercase(location ?? '') CONTAINS string::lowercase('{}')",
                loc.replace('\'', "''")
            )
        } else {
            String::new()
        };

        let query_lower = params.query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];

        let sql = format!(
            "SELECT
                <string> id AS id,
                name,
                slug,
                description,
                location,
                <float> (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS score
            FROM organization
            WHERE
                (
                    string::lowercase(name ?? '') CONTAINS $query_lower
                    OR string::lowercase(description ?? '') CONTAINS $query_lower
                    OR string::lowercase(location ?? '') CONTAINS $query_lower
                    OR (embedding IS NOT NONE AND $has_embedding = true
                        AND vector::similarity::cosine(embedding, $query_embedding) > 0.45)
                )
                {hard_filter}
            ORDER BY score DESC
            LIMIT $limit"
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
                let desc = if description.len() > 200 {
                    format!("{}...", &description[..200])
                } else {
                    description.to_string()
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
        let query_embedding = generate_embedding_async(&params.query).await.ok();

        let mut where_clauses = Vec::new();

        if let Some(ref city) = params.city {
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

        let hard_filter = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("AND {}", where_clauses.join(" AND "))
        };

        let query_lower = params.query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];

        let sql = format!(
            "SELECT
                <string> id AS id,
                name,
                <string> meta::id(id) AS key,
                address,
                city,
                state,
                description,
                <float> (
                    (IF string::lowercase(name ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(city ?? '') CONTAINS $query_lower THEN 30 ELSE 0 END)
                    + (IF string::lowercase(state ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN 10 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS score
            FROM location
            WHERE is_public = true AND (
                string::lowercase(name ?? '') CONTAINS $query_lower
                OR string::lowercase(city ?? '') CONTAINS $query_lower
                OR string::lowercase(state ?? '') CONTAINS $query_lower
                OR string::lowercase(description ?? '') CONTAINS $query_lower
                OR (embedding IS NOT NONE AND $has_embedding = true
                    AND vector::similarity::cosine(embedding, $query_embedding) > 0.45)
            )
            {hard_filter}
            ORDER BY score DESC
            LIMIT $limit"
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
                let desc = if description.len() > 200 {
                    format!("{}...", &description[..200])
                } else {
                    description.to_string()
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
        let query_embedding = generate_embedding_async(&params.query).await.ok();

        let open_only = params.open_only.unwrap_or(true);

        let mut where_clauses = Vec::new();

        if open_only {
            where_clauses.push("status = 'open'".to_string());
        }

        if let Some(ref loc) = params.location {
            where_clauses.push(format!(
                "string::lowercase(location ?? '') CONTAINS string::lowercase('{}')",
                loc.replace('\'', "''")
            ));
        }

        let hard_filter = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("AND {}", where_clauses.join(" AND "))
        };

        let query_lower = params.query.to_lowercase();
        let empty_emb: Vec<f32> = vec![];

        let sql = format!(
            "SELECT
                <string> id AS id,
                <string> meta::id(id) AS key,
                title,
                description,
                location,
                status,
                <float> (
                    (IF string::lowercase(title ?? '') CONTAINS $query_lower THEN 50 ELSE 0 END)
                    + (IF string::lowercase(description ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF string::lowercase(location ?? '') CONTAINS $query_lower THEN 20 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 30
                        ELSE 0
                    END)
                ) AS score
            FROM job_posting
            WHERE
                (
                    string::lowercase(title ?? '') CONTAINS $query_lower
                    OR string::lowercase(description ?? '') CONTAINS $query_lower
                    OR string::lowercase(location ?? '') CONTAINS $query_lower
                    OR (embedding IS NOT NONE AND $has_embedding = true
                        AND vector::similarity::cosine(embedding, $query_embedding) > 0.45)
                )
                {hard_filter}
            ORDER BY score DESC
            LIMIT $limit"
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
                let desc = if description.len() > 200 {
                    format!("{}...", &description[..200])
                } else {
                    description.to_string()
                };
                out.push_str(&format!("  Description: {}\n", desc));
            }
            out.push_str(&format!("  URL: {}/jobs/{}\n", self.app_url, key));
            out.push('\n');
        }
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
