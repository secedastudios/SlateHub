use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, Request},
    http::{HeaderMap, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use tracing::{debug, error, info};

use crate::{
    db::DB,
    error::Error,
    middleware::UserExtractor,
    models::analytics::AnalyticsModel,
    models::involvement::InvolvementModel,
    models::likes::LikesModel,
    models::person::Person,
    record_id_ext::RecordIdExt,
    services::embedding::generate_embedding_async,
    services::search_log::log_search,
    services::search_utils::normalize_query,
    social_platforms,
    templates::{
        BaseContext, DateRange, Education, InvolvementDisplay, PeopleTemplate, PersonCard,
        PhotoDisplay, ProfileData, ProfileTemplate, ReelDisplay, SocialLinkDisplay, User,
    },
    video_platforms,
};
use surrealdb::types::RecordId;

const PAGE_SIZE: usize = 20;

pub fn router() -> Router {
    Router::new()
        .route("/people", get(people))
        .route("/api/people/more-sse", get(people_more_sse))
        // User profile route - must be last to avoid conflicts with other routes
        .route("/{username}", get(user_profile))
}


/// List of reserved routes that should not be treated as usernames
const RESERVED_ROUTES: &[&str] = &[
    "about",
    "account",
    "admin",
    "api",
    "auth",
    "contact",
    "dashboard",
    "get-verified",
    "help",
    "home",
    "likes",
    "login",
    "logout",
    "messages",
    "org",
    "orgs",
    "people",
    "profile",
    "project",
    "projects",
    "search",
    "settings",
    "signup",
    "static",
    "support",
    "terms",
    "privacy",
];

/// Convert stored photos to display format
fn to_photo_displays(photos: &[crate::models::person::Photo]) -> Vec<PhotoDisplay> {
    photos
        .iter()
        .map(|photo| PhotoDisplay {
            url: photo.url.clone(),
            thumbnail_url: photo.thumbnail_url.clone(),
            caption: photo.caption.clone(),
        })
        .collect()
}

/// Convert stored reels to display format with computed URLs
fn to_reel_displays(reels: &[crate::models::person::Reel]) -> Vec<ReelDisplay> {
    reels
        .iter()
        .map(|reel| ReelDisplay {
            url: reel.url.clone(),
            title: reel.title.clone(),
            platform: reel.platform.clone(),
            video_id: reel.video_id.clone(),
            thumbnail_url: video_platforms::thumbnail_url(&reel.platform, &reel.video_id),
            embed_url: video_platforms::embed_url(&reel.platform, &reel.video_id),
            platform_name: video_platforms::platform_name(&reel.platform).to_string(),
        })
        .collect()
}

/// Convert stored social links to display format with platform metadata
fn to_social_link_displays(links: &[crate::models::person::SocialLink]) -> Vec<SocialLinkDisplay> {
    links
        .iter()
        .map(|link| {
            let platform = social_platforms::find_platform(&link.platform);
            SocialLinkDisplay {
                platform: link.platform.clone(),
                url: link.url.clone(),
                name: platform.name.to_string(),
                icon_svg: platform.icon_svg.to_string(),
            }
        })
        .collect()
}

/// Handler for viewing a user's public profile at /{username}
/// Uses the same ProfileTemplate as the authenticated profile view
async fn user_profile(
    Path(username): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Attempting to view public profile: {}", username);

    // Check if this is a reserved route
    if RESERVED_ROUTES.contains(&username.as_str()) {
        debug!("Username {} is a reserved route", username);
        return Err(Error::NotFound);
    }

    // Get current user if authenticated
    let current_user = request.get_user();
    let is_own_profile = current_user
        .as_ref()
        .map(|u| u.username == username)
        .unwrap_or(false);

    // Fetch the user's profile data using the Person model
    let profile_user = match Person::find_by_username(&username).await? {
        Some(p) => p,
        None => {
            info!("User profile not found for username: {}", username);
            return Err(Error::NotFound);
        }
    };

    // Record profile view (fire-and-forget, skip own profile)
    if !is_own_profile {
        let pid = profile_user.id.clone();
        let viewer_rid = current_user.as_ref().and_then(|u| {
            if u.id.starts_with("person:") {
                RecordId::parse_simple(&u.id).ok()
            } else {
                Some(RecordId::new("person", u.id.as_str()))
            }
        });
        let referrer = headers
            .get(header::REFERER)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let user_agent = headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        tokio::spawn(async move {
            let _ = AnalyticsModel::record_view(
                &pid,
                viewer_rid.as_ref(),
                referrer.as_deref(),
                user_agent.as_deref(),
            )
            .await;
        });
    }

    // Build base context
    let mut base = BaseContext::new().with_page("profile");
    let mut is_liked = false;
    if let Some(ref user) = current_user {
        base = base.with_user(User::from_session_user(&user).await);

        // Check if current user has liked this profile
        if !is_own_profile {
            let person_rid = if user.id.starts_with("person:") {
                RecordId::parse_simple(&user.id).ok()
            } else {
                Some(RecordId::new("person", user.id.as_str()))
            };
            if let Some(rid) = person_rid {
                is_liked = LikesModel::is_liked(&rid, &profile_user.id)
                    .await
                    .unwrap_or(false);
            }
        }
    }

    // Convert Person model to ProfileData (same structure as /profile/{username} used)
    let profile = profile_user.profile.as_ref();
    let profile_data = ProfileData {
        id: profile_user.id.to_raw_string(),
        name: profile_user.get_display_name(),
        username: profile_user.username.clone(),
        email: profile_user.email.clone(),
        avatar: profile_user.get_avatar_url(),
        initials: profile_user.get_initials(),
        headline: profile.and_then(|p| p.headline.clone()),
        bio: profile.and_then(|p| p.bio.clone()),
        location: profile.and_then(|p| p.location.clone()),
        website: profile.and_then(|p| p.website.clone()),
        skills: profile.map(|p| p.skills.clone()).unwrap_or_default(),
        languages: profile.map(|p| p.languages.clone()).unwrap_or_default(),
        availability: profile.and_then(|p| p.availability.clone()),
        involvements: {
            let pid = profile_user.id.to_raw_string();
            match InvolvementModel::get_for_person(&pid).await {
                Ok(invs) => invs
                    .into_iter()
                    .map(|inv| InvolvementDisplay {
                        involvement_id: inv.id.to_raw_string(),
                        role: inv.role,
                        relation_type: inv.relation_type,
                        department: inv.department,
                        verification_status: inv.verification_status,
                        production_title: inv.production_title,
                        production_slug: inv.production_slug,
                        production_type: inv.production_type,
                        poster_url: inv.poster_url,
                        tmdb_url: inv.tmdb_url,
                        release_date: inv.release_date,
                        media_type: inv.media_type,
                        is_claimed: inv.is_claimed,
                    })
                    .collect(),
                Err(e) => {
                    error!("Failed to fetch involvements for {}: {}", username, e);
                    vec![]
                }
            }
        },
        education: profile
            .map(|p| p.education.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|e| Education {
                institution: e.institution,
                degree: e.degree,
                field: e.field,
                dates: e.dates.map(|d| DateRange {
                    start: d.start,
                    end: d.end,
                }),
            })
            .collect(),
        social_links: to_social_link_displays(
            &profile.map(|p| p.social_links.clone()).unwrap_or_default(),
        ),
        reels: to_reel_displays(
            &profile.map(|p| p.reels.clone()).unwrap_or_default(),
        ),
        photos: to_photo_displays(
            &profile.map(|p| p.photos.clone()).unwrap_or_default(),
        ),
        is_own_profile,
        is_public: profile.map(|p| p.is_public).unwrap_or(false),
        verification_status: profile_user.verification_status.clone(),
        gender: profile.and_then(|p| p.gender.clone()),
        birthday: profile.and_then(|p| p.birthday.clone()),
        height_mm: profile.and_then(|p| p.height_mm),
        weight_kg: profile.and_then(|p| p.weight_kg),
        body_type: profile.and_then(|p| p.body_type.clone()),
        hair_color: profile.and_then(|p| p.hair_color.clone()),
        eye_color: profile.and_then(|p| p.eye_color.clone()),
        ethnicity: profile.map(|p| p.ethnicity.clone()).unwrap_or_default(),
        acting_age_range_min: profile.and_then(|p| p.acting_age_range.as_ref().map(|r| r.min)),
        acting_age_range_max: profile.and_then(|p| p.acting_age_range.as_ref().map(|r| r.max)),
        acting_ethnicities: profile.map(|p| p.acting_ethnicities.clone()).unwrap_or_default(),
        nationality: profile.and_then(|p| p.nationality.clone()),
        messaging_preference: profile_user.messaging_preference.clone(),
        phone: profile.and_then(|p| p.phone.clone()),
    };

    // Create and render template using the same ProfileTemplate
    let template = ProfileTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        profile: profile_data,
        is_liked,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render profile template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Deserialize)]
struct PeopleQuery {
    filter: Option<String>,
}

async fn people(
    Query(params): Query<PeopleQuery>,
    request: Request,
) -> Result<Html<String>, Error> {
    let filter = params.filter.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());
    debug!("Rendering people page, filter: {:?}", filter);

    let mut base = BaseContext::new().with_page("people");

    // Add user to context if authenticated
    let current_user_id = if let Some(user) = request.get_user() {
        let uid = user.id.clone();
        base = base.with_user(User::from_session_user(&user).await);
        Some(uid)
    } else {
        None
    };

    let mut template = PeopleTemplate::new(base);
    template.current_user_id = current_user_id.clone().unwrap_or_default();
    template.filter = filter.map(|s| s.to_string());

    // Add specialties list (in production, fetch from database)
    template.specialties = vec![
        "Director".to_string(),
        "Producer".to_string(),
        "Cinematographer".to_string(),
        "Editor".to_string(),
        "Sound Designer".to_string(),
        "Actor".to_string(),
        "Writer".to_string(),
        "Composer".to_string(),
    ];

    // Fetch profiles from the database, optionally filtered
    let persons = if let Some(filter_text) = filter {
        // Extract location from "actors in berlin" → filter: "actors", location: "berlin"
        let loc_re = regex::Regex::new(r"(?i)\bin\s+(.+)$").unwrap();
        let (location_filter, cleaned_filter) = if let Some(caps) = loc_re.captures(filter_text) {
            let loc = caps.get(1).map(|m| m.as_str().trim().to_string());
            let cleaned = loc_re.replace(filter_text, "").to_string();
            let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
            (loc, cleaned)
        } else {
            (None, filter_text.to_string())
        };

        let filter_lower = normalize_query(&cleaned_filter);
        let has_location = location_filter.is_some();
        let query_embedding = generate_embedding_async(&cleaned_filter).await.ok();
        let has_embedding = query_embedding.is_some();
        let empty_emb: Vec<f32> = vec![];

        // When location filter is extracted, text/vector gate is optional
        let text_gate = if has_location && filter_lower.trim().is_empty() {
            // Location only, no role query (e.g., "in berlin") — return everyone in that location
            "true".to_string()
        } else {
            // Always require text/vector match when there's a query term.
            // embedding_text check catches role synonyms (e.g., "dop" finds cinematographers).
            "(\
                string::lowercase(name ?? '') CONTAINS $filter \
                OR string::lowercase(username ?? '') CONTAINS $filter \
                OR string::lowercase(profile.name ?? '') CONTAINS $filter \
                OR string::lowercase(profile.headline ?? '') CONTAINS $filter \
                OR string::lowercase(profile.bio ?? '') CONTAINS $filter \
                OR string::lowercase(profile.location ?? '') CONTAINS $filter \
                OR string::lowercase(embedding_text ?? '') CONTAINS $filter \
                OR $filter IN profile.skills.map(|$v| string::lowercase($v)) \
                OR $filter IN profile.languages.map(|$v| string::lowercase($v)) \
                OR (embedding IS NOT NONE AND $has_embedding = true \
                    AND vector::similarity::cosine(embedding, $query_embedding) > 0.75)\
            )".to_string()
        };

        let location_clause = if has_location {
            "AND (string::lowercase(profile.location ?? '') CONTAINS string::lowercase($location_filter) OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase($location_filter))"
        } else {
            ""
        };

        let query = format!(r#"
            SELECT *, verification_status = 'identity' AS _vord,
                <float> (
                    (IF string::lowercase(name ?? '') CONTAINS $filter THEN 50 ELSE 0 END)
                    + (IF string::lowercase(profile.headline ?? '') CONTAINS $filter THEN 20 ELSE 0 END)
                    + (IF string::lowercase(profile.bio ?? '') CONTAINS $filter THEN 10 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 50
                        ELSE 0
                    END)
                ) AS _score
            FROM person
            WHERE (profile.name IS NOT NULL
               OR profile.headline IS NOT NULL
               OR profile.bio IS NOT NULL)
              AND {text_gate}
              {location_clause}
            ORDER BY _score DESC, _vord DESC, created_at DESC
            LIMIT $limit
            START $offset
        "#);
        let result = match DB.query(&query)
            .bind(("filter", filter_lower))
            .bind(("location_filter", location_filter.unwrap_or_default()))
            .bind(("has_embedding", has_embedding))
            .bind(("query_embedding", query_embedding.unwrap_or(empty_emb)))
            .bind(("limit", PAGE_SIZE as i64 + 1))
            .bind(("offset", 0i64))
            .await
        {
            Ok(mut result) => match result.take::<Vec<Person>>(0) {
                Ok(persons) => persons,
                Err(e) => {
                    error!("Failed to fetch filtered persons: {}", e);
                    vec![]
                }
            },
            Err(e) => {
                error!("Failed to query filtered persons: {}", e);
                vec![]
            }
        };

        log_search(filter_text, "web", "people", Some(result.len()));
        result
    } else {
        let query = r#"
            SELECT *, verification_status = 'identity' AS _vord FROM person
            WHERE profile.name IS NOT NULL
               OR profile.headline IS NOT NULL
               OR profile.bio IS NOT NULL
            ORDER BY _vord DESC, created_at DESC
            LIMIT $limit
            START $offset
        "#;
        match DB.query(query)
            .bind(("limit", PAGE_SIZE as i64 + 1))
            .bind(("offset", 0i64))
            .await
        {
            Ok(mut result) => match result.take::<Vec<Person>>(0) {
                Ok(persons) => persons,
                Err(e) => {
                    error!("Failed to fetch persons from database: {}", e);
                    vec![]
                }
            },
            Err(e) => {
                error!("Failed to query persons from database: {}", e);
                vec![]
            }
        }
    };

    let has_more = persons.len() > PAGE_SIZE;
    let persons: Vec<Person> = persons.into_iter().take(PAGE_SIZE).collect();

    // Convert Person objects to PersonCard for the template
    template.has_more = has_more;
    template.people = persons
        .into_iter()
        .filter_map(|person| {
            // Only include profiles that have at least a name
            if let Some(profile) = person.profile {
                if profile.name.is_some() || profile.headline.is_some() || profile.bio.is_some() {
                    Some(PersonCard {
                        id: person.id.to_raw_string(),
                        name: profile
                            .name
                            .clone()
                            .unwrap_or_else(|| person.username.clone()),
                        username: person.username.clone(),
                        headline: profile.headline.clone(),
                        bio: profile.bio.clone(),
                        location: profile.location.clone(),
                        skills: profile.skills,
                        avatar: profile
                            .avatar
                            .clone()
                            .unwrap_or_else(|| format!("/static/images/default-avatar.svg")),
                        is_identity_verified: person.verification_status == "identity",
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Fetch liked IDs if user is logged in
    if let Some(ref uid) = current_user_id {
        let person_rid = if uid.starts_with("person:") {
            RecordId::parse_simple(uid).ok()
        } else {
            Some(RecordId::new("person", uid.as_str()))
        };
        if let Some(rid) = person_rid {
            let target_ids: Vec<RecordId> = template
                .people
                .iter()
                .filter_map(|p| RecordId::parse_simple(&p.id).ok())
                .collect();
            template.liked_ids = LikesModel::get_liked_ids(&rid, &target_ids)
                .await
                .unwrap_or_default();
        }
    }

    let html = template.render().map_err(|e| {
        error!("Failed to render people template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
struct PeopleMoreQuery {
    offset: usize,
    filter: Option<String>,
}

fn sse_patch_elements(selector: &str, mode: &str, elements: &str) -> String {
    let mut s = format!(
        "event: datastar-patch-elements\ndata: selector {}\ndata: mode {}\n",
        selector, mode
    );
    if !elements.is_empty() {
        s += &format!("data: elements {}\n", elements.replace('\n', " "));
    }
    s += "\n";
    s
}

fn sse_response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

fn escape_html(s: &str) -> String {
    ammonia::clean_text(s)
}

const VERIFIED_BADGE_PATH: &str = "M22.5 12.5c0-1.58-.875-2.95-2.148-3.6.154-.435.238-.905.238-1.4 0-2.21-1.71-3.998-3.818-3.998-.47 0-.92.084-1.336.25C14.818 2.415 13.51 1.5 12 1.5s-2.816.917-3.437 2.25c-.415-.165-.866-.25-1.336-.25-2.11 0-3.818 1.79-3.818 4 0 .494.083.964.237 1.4-1.272.65-2.147 2.018-2.147 3.6 0 1.495.782 2.798 1.942 3.486-.02.17-.032.34-.032.514 0 2.21 1.708 4 3.818 4 .47 0 .92-.086 1.335-.25.62 1.334 1.926 2.25 3.437 2.25 1.512 0 2.818-.916 3.437-2.25.415.163.865.248 1.336.248 2.11 0 3.818-1.79 3.818-4 0-.174-.012-.344-.033-.513 1.158-.687 1.943-1.99 1.943-3.484zm-6.616-3.334l-4.334 6.5c-.145.217-.382.334-.625.334-.143 0-.288-.04-.416-.126l-.115-.094-2.415-2.415c-.293-.293-.293-.768 0-1.06s.768-.294 1.06 0l1.77 1.767 3.825-5.74c.23-.345.696-.436 1.04-.207.346.23.44.696.21 1.04z";

fn render_person_card(person: &PersonCard) -> String {
    let mut html = String::new();
    html.push_str(r#"<article data-component="card" data-type="person">"#);
    html.push_str(&format!(
        r#"<a href="/{}" data-role="card-visual">"#,
        escape_html(&person.username)
    ));
    html.push_str(&format!(
        r#"<img src="{}" alt="{}" loading="lazy" onerror="this.style.display='none'" />"#,
        escape_html(&person.avatar),
        escape_html(&person.name)
    ));
    html.push_str(r#"<div data-role="overlay">"#);
    html.push_str(&format!("<h3>{}", escape_html(&person.name)));
    if person.is_identity_verified {
        html.push_str(&format!(
            " <svg data-role=\"verified-badge\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"#1d9bf0\" aria-label=\"Verified\"><path d=\"{}\"/></svg>",
            VERIFIED_BADGE_PATH
        ));
    }
    html.push_str("</h3>");
    html.push_str(r#"<div data-role="meta">"#);
    if let Some(ref headline) = person.headline {
        html.push_str(&format!(
            r#"<span data-role="role">{}</span>"#,
            escape_html(headline)
        ));
    }
    if let Some(ref loc) = person.location {
        html.push_str(&format!(
            r#"<span data-role="loc">{}</span>"#,
            escape_html(loc)
        ));
    }
    html.push_str("</div></div></a>");

    html.push_str(r#"<div data-role="content">"#);
    if let Some(ref bio) = person.bio {
        html.push_str(&format!(
            r#"<p data-role="bio">{}</p>"#,
            escape_html(bio)
        ));
    }
    if !person.skills.is_empty() {
        html.push_str(r#"<p data-role="skills">"#);
        for skill in &person.skills {
            html.push_str(&format!("<span>{}</span>", escape_html(skill)));
        }
        html.push_str("</p>");
    }
    html.push_str("</div></article>");

    html
}

async fn people_more_sse(Query(params): Query<PeopleMoreQuery>) -> Response {
    let filter = params.filter.as_deref().filter(|s| !s.is_empty());
    let offset = params.offset;

    let persons = if let Some(filter_text) = filter {
        let loc_re = regex::Regex::new(r"(?i)\bin\s+(.+)$").unwrap();
        let (location_filter, cleaned_filter) = if let Some(caps) = loc_re.captures(filter_text) {
            let loc = caps.get(1).map(|m| m.as_str().trim().to_string());
            let cleaned = loc_re.replace(filter_text, "").to_string();
            let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
            (loc, cleaned)
        } else {
            (None, filter_text.to_string())
        };

        let filter_lower = normalize_query(&cleaned_filter);
        let has_location = location_filter.is_some();
        let query_embedding = generate_embedding_async(&cleaned_filter).await.ok();
        let has_embedding = query_embedding.is_some();
        let empty_emb: Vec<f32> = vec![];

        let text_gate = if has_location && filter_lower.trim().is_empty() {
            // Location only, no role query (e.g., "in berlin") — return everyone in that location
            "true".to_string()
        } else {
            // Always require text/vector match when there's a query term.
            // embedding_text check catches role synonyms (e.g., "dop" finds cinematographers).
            "(\
                string::lowercase(name ?? '') CONTAINS $filter \
                OR string::lowercase(username ?? '') CONTAINS $filter \
                OR string::lowercase(profile.name ?? '') CONTAINS $filter \
                OR string::lowercase(profile.headline ?? '') CONTAINS $filter \
                OR string::lowercase(profile.bio ?? '') CONTAINS $filter \
                OR string::lowercase(profile.location ?? '') CONTAINS $filter \
                OR string::lowercase(embedding_text ?? '') CONTAINS $filter \
                OR $filter IN profile.skills.map(|$v| string::lowercase($v)) \
                OR $filter IN profile.languages.map(|$v| string::lowercase($v)) \
                OR (embedding IS NOT NONE AND $has_embedding = true \
                    AND vector::similarity::cosine(embedding, $query_embedding) > 0.75)\
            )".to_string()
        };

        let location_clause = if has_location {
            "AND (string::lowercase(profile.location ?? '') CONTAINS string::lowercase($location_filter) OR string::lowercase(embedding_text ?? '') CONTAINS string::lowercase($location_filter))"
        } else {
            ""
        };

        let query = format!(r#"
            SELECT *, verification_status = 'identity' AS _vord,
                <float> (
                    (IF string::lowercase(name ?? '') CONTAINS $filter THEN 50 ELSE 0 END)
                    + (IF string::lowercase(profile.headline ?? '') CONTAINS $filter THEN 20 ELSE 0 END)
                    + (IF string::lowercase(profile.bio ?? '') CONTAINS $filter THEN 10 ELSE 0 END)
                    + (IF embedding IS NOT NONE AND $has_embedding = true
                        THEN vector::similarity::cosine(embedding, $query_embedding) * 50
                        ELSE 0
                    END)
                ) AS _score
            FROM person
            WHERE (profile.name IS NOT NULL
               OR profile.headline IS NOT NULL
               OR profile.bio IS NOT NULL)
              AND {text_gate}
              {location_clause}
            ORDER BY _score DESC, _vord DESC, created_at DESC
            LIMIT $limit
            START $offset
        "#);
        match DB
            .query(&query)
            .bind(("filter", filter_lower))
            .bind(("location_filter", location_filter.unwrap_or_default()))
            .bind(("has_embedding", has_embedding))
            .bind(("query_embedding", query_embedding.unwrap_or(empty_emb)))
            .bind(("limit", PAGE_SIZE as i64 + 1))
            .bind(("offset", offset as i64))
            .await
        {
            Ok(mut result) => result.take::<Vec<Person>>(0).unwrap_or_default(),
            Err(_) => vec![],
        }
    } else {
        let query = r#"
            SELECT *, verification_status = 'identity' AS _vord FROM person
            WHERE profile.name IS NOT NULL
               OR profile.headline IS NOT NULL
               OR profile.bio IS NOT NULL
            ORDER BY _vord DESC, created_at DESC
            LIMIT $limit
            START $offset
        "#;
        match DB
            .query(query)
            .bind(("limit", PAGE_SIZE as i64 + 1))
            .bind(("offset", offset as i64))
            .await
        {
            Ok(mut result) => result.take::<Vec<Person>>(0).unwrap_or_default(),
            Err(_) => vec![],
        }
    };

    let has_more = persons.len() > PAGE_SIZE;

    let cards: Vec<PersonCard> = persons
        .into_iter()
        .take(PAGE_SIZE)
        .filter_map(|person| {
            if let Some(profile) = person.profile {
                if profile.name.is_some() || profile.headline.is_some() || profile.bio.is_some() {
                    Some(PersonCard {
                        id: person.id.to_raw_string(),
                        name: profile
                            .name
                            .clone()
                            .unwrap_or_else(|| person.username.clone()),
                        username: person.username.clone(),
                        headline: profile.headline.clone(),
                        bio: profile.bio.clone(),
                        location: profile.location.clone(),
                        skills: profile.skills,
                        avatar: profile
                            .avatar
                            .clone()
                            .unwrap_or_else(|| "/static/images/default-avatar.svg".to_string()),
                        is_identity_verified: person.verification_status == "identity",
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    if cards.is_empty() {
        return sse_response(sse_patch_elements("#people-sentinel", "remove", ""));
    }

    let mut replacement = String::new();
    for card in &cards {
        replacement.push_str(&render_person_card(card));
    }

    if has_more {
        let new_offset = offset + PAGE_SIZE;
        let q_param = match filter {
            Some(f) => format!("&filter={}", urlencoding::encode(f)),
            None => String::new(),
        };
        replacement.push_str(&format!(
            r#"<div id="people-sentinel" data-on-intersect="@get('/api/people/more-sse?offset={}{}')"><div class="people-loading">Loading more...</div></div>"#,
            new_offset, q_param
        ));
    }

    sse_response(sse_patch_elements("#people-sentinel", "outer", &replacement))
}
