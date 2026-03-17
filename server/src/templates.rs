use askama::Template;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::db::DB;
use crate::models::likes::{LikedLocation, LikedPerson};
use crate::models::notification::NotificationModel;
use crate::models::person::SessionUser;

mod filters {
    /// Convert a relative path to an absolute URL using APP_URL
    pub fn abs_url(path: &str) -> askama::Result<String> {
        let base = crate::config::app_url();
        Ok(format!("{}{}", base, path))
    }

    /// Check if a Vec<String> contains a given value
    pub fn contains(list: &[String], value: &String) -> askama::Result<bool> {
        Ok(list.contains(value))
    }

    /// Abbreviate a signed number: +1500 → "+1.5k", -200000 → "-200k"
    pub fn abbr_i64(value: i64) -> askama::Result<String> {
        let abs = value.unsigned_abs();
        let formatted = abbr(&abs)?;
        if value >= 0 {
            Ok(format!("+{}", formatted))
        } else {
            Ok(format!("-{}", formatted))
        }
    }

    /// Abbreviate large numbers: 1500 → "1.5k", 200000 → "200k", 1500000 → "1.5M"
    pub fn abbr(value: &u64) -> askama::Result<String> {
        abbr_num(*value as usize)
    }

    /// Abbreviate large usize numbers
    pub fn abbr_usize(value: &usize) -> askama::Result<String> {
        abbr_num(*value)
    }

    fn abbr_num(n: usize) -> askama::Result<String> {
        let (divisor, suffix) = if n >= 1_000_000 {
            (1_000_000.0, "M")
        } else if n >= 1_000 {
            (1_000.0, "k")
        } else {
            return Ok(n.to_string());
        };
        let v = n as f64 / divisor;
        let s = format!("{:.1}", v);
        let s = s.strip_suffix(".0").unwrap_or(&s);
        Ok(format!("{}{}", s, suffix))
    }
}

/// Represents a user for template rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar: String,             // Compatibility field - either URL or empty
    pub avatar_url: Option<String>, // Actual profile image URL if exists
    pub initials: String,           // Fallback initials from username/name
    pub notification_count: u32,    // Unread notification count
    pub is_identity_verified: bool, // Whether user has identity verification
    pub is_admin: bool,             // Whether user is a system administrator
}

impl User {
    /// Create a User from a SessionUser, fetching the actual avatar URL from the database
    pub async fn from_session_user(session_user: &SessionUser) -> Self {
        // Generate initials from name or username
        let initials = Self::generate_initials(&session_user.name);

        // Try to fetch the avatar URL, verification status, and admin flag from the database
        let (avatar_url, is_identity_verified, is_admin) =
            Self::fetch_avatar_and_verification(&session_user.id).await;

        // Fetch unread notification count
        let notification_count = NotificationModel::new()
            .get_unread_count(&session_user.id)
            .await
            .unwrap_or(0);

        // For compatibility, set avatar to the URL if it exists, otherwise use /api/avatar endpoint
        let avatar = avatar_url
            .clone()
            .unwrap_or_else(|| format!("/api/avatar?id={}", session_user.id));

        User {
            id: session_user.id.clone(),
            name: session_user.name.clone(),
            email: session_user.email.clone(),
            avatar,
            avatar_url,
            initials,
            notification_count,
            is_identity_verified,
            is_admin,
        }
    }

    /// Generate initials from a name or username
    fn generate_initials(name: &str) -> String {
        let parts: Vec<&str> = name.split_whitespace().collect();

        if parts.len() >= 2 {
            // Use first letter of first and last name
            let first = parts[0].chars().next().unwrap_or('?');
            let last = parts[parts.len() - 1].chars().next().unwrap_or('?');
            format!("{}{}", first, last).to_uppercase()
        } else if !parts.is_empty() {
            // Use first two letters of single name
            let chars: Vec<char> = parts[0].chars().take(2).collect();
            if chars.len() == 2 {
                format!("{}{}", chars[0], chars[1]).to_uppercase()
            } else if chars.len() == 1 {
                format!("{}", chars[0]).to_uppercase()
            } else {
                "??".to_string()
            }
        } else {
            "??".to_string()
        }
    }

    /// Fetch avatar URL, verification status, and admin flag from the database
    async fn fetch_avatar_and_verification(person_id: &str) -> (Option<String>, bool, bool) {
        // Ensure we have full record ID
        let person_rid = if person_id.starts_with("person:") {
            surrealdb::types::RecordId::parse_simple(person_id).ok()
        } else {
            Some(surrealdb::types::RecordId::new("person", person_id))
        };

        let Some(rid) = person_rid else {
            return (None, false, false);
        };

        // Query for the person's avatar URL, verification status, and admin flag
        if let Ok(mut response) = DB
            .query("SELECT profile.avatar, verification_status, is_admin FROM ONLY $pid LIMIT 1")
            .bind(("pid", rid))
            .await
        {
            if let Ok(result) = response.take::<Option<serde_json::Value>>(0) {
                if let Some(data) = result {
                    let avatar_url = data
                        .get("profile")
                        .and_then(|p| p.get("avatar"))
                        .and_then(|a| a.as_str())
                        .map(|s| s.to_string());
                    let is_verified = data
                        .get("verification_status")
                        .and_then(|v| v.as_str())
                        .map(|s| s == "identity")
                        .unwrap_or(false);
                    let is_admin = data
                        .get("is_admin")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    return (avatar_url, is_verified, is_admin);
                }
            }
        }

        (None, false, false)
    }
}

/// Common template data
#[derive(Debug, Clone)]
pub struct CommonData {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
}

impl Default for CommonData {
    fn default() -> Self {
        Self {
            app_name: "SlateHub".to_string(),
            year: chrono::Utc::now().year(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            active_page: String::new(),
            user: None,
        }
    }
}

/// Index/Home page template
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub production_count: u32,
    pub user_count: u32,
    pub connection_count: u32,
    pub activities: Vec<Activity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub user: String,
    pub action: String,
    pub time: String,
}

/// Login page template
#[derive(Template)]
#[template(path = "login/index.html")]
pub struct LoginTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub error: Option<String>,
    pub redirect_to: Option<String>,
}

/// Signup page template
#[derive(Template)]
#[template(path = "signup/index.html")]
pub struct SignupTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub error: Option<String>,
    pub prefill_email: Option<String>,
}

/// Email verification page template
#[derive(Template)]
#[template(path = "auth/verify_email.html")]
pub struct EmailVerificationTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub error: Option<String>,
    pub success: Option<String>,
    pub email: Option<String>,
}

/// Forgot password page template
#[derive(Template)]
#[template(path = "auth/forgot_password.html")]
pub struct ForgotPasswordTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub error: Option<String>,
    pub success: Option<String>,
    pub email: Option<String>,
}

/// Reset password page template
#[derive(Template)]
#[template(path = "auth/reset_password.html")]
pub struct ResetPasswordTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub error: Option<String>,
    pub success: Option<String>,
    pub email: Option<String>,
    pub code: Option<String>,
}

/// Profile page template
#[derive(Template)]
#[template(path = "persons/profile.html")]
pub struct ProfileTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub profile: ProfileData,
    pub is_liked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileData {
    pub id: String,
    pub name: String,
    pub username: String,
    pub email: String,
    pub avatar: Option<String>,
    pub initials: String,
    pub headline: Option<String>,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub skills: Vec<String>,
    pub languages: Vec<String>,
    pub availability: Option<String>,
    pub involvements: Vec<InvolvementDisplay>,
    pub education: Vec<Education>,
    pub social_links: Vec<SocialLinkDisplay>,
    pub reels: Vec<ReelDisplay>,
    pub photos: Vec<PhotoDisplay>,
    pub is_own_profile: bool,
    pub is_public: bool,
    pub verification_status: String,
    // Physical attributes
    pub gender: Option<String>,
    pub birthday: Option<String>,
    pub height_mm: Option<i32>,
    pub weight_kg: Option<i32>,
    pub body_type: Option<String>,
    pub hair_color: Option<String>,
    pub eye_color: Option<String>,
    pub ethnicity: Vec<String>,
    pub acting_age_range_min: Option<i32>,
    pub acting_age_range_max: Option<i32>,
    pub acting_ethnicities: Vec<String>,
    pub nationality: Option<String>,
    pub messaging_preference: String,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLinkDisplay {
    pub platform: String,
    pub url: String,
    pub name: String,
    pub icon_svg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReelDisplay {
    pub url: String,
    pub title: String,
    pub platform: String,
    pub video_id: String,
    pub thumbnail_url: String,
    pub embed_url: String,
    pub platform_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoDisplay {
    pub url: String,
    pub thumbnail_url: String,
    pub caption: String,
}

/// Display struct for involvement-based credits (graph traversal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvolvementDisplay {
    pub involvement_id: String,
    pub role: Option<String>,
    pub relation_type: String,
    pub department: Option<String>,
    pub verification_status: String,
    pub production_title: String,
    pub production_slug: String,
    pub production_type: String,
    pub poster_url: Option<String>,
    pub tmdb_url: Option<String>,
    pub release_date: Option<String>,
    pub media_type: Option<String>,
    pub is_claimed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Education {
    pub institution: String,
    pub degree: Option<String>,
    pub field: Option<String>,
    pub dates: Option<DateRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: Option<String>,
    pub end: Option<String>,
}

/// Profile edit page template
#[derive(Template)]
#[template(path = "persons/profile_edit.html")]
pub struct ProfileEditTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub profile: ProfileData,
    pub platforms: Vec<SocialPlatformOption>,
    pub error: Option<String>,
    pub success: Option<String>,
    pub photo_count: usize,
    pub photo_limit: Option<usize>,
    pub reel_count: usize,
    pub reel_limit: Option<usize>,
    pub is_identity_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPlatformOption {
    pub id: String,
    pub name: String,
    pub placeholder: String,
    pub base_url: Option<String>,
}

/// Productions page template
#[derive(Template)]
#[template(path = "productions/productions.html")]
pub struct ProductionsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub productions: Vec<Production>,
    pub filter: Option<String>,
    pub sort_by: String,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Production {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub production_type: String,
    pub created_at: String,
    pub owner: String,
    pub tags: Vec<String>,
    pub poster_url: Option<String>,
    pub poster_photo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionPhotoView {
    pub url: String,
    pub thumbnail_url: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionScriptView {
    pub id: String,
    pub title: String,
    pub version: i64,
    pub visibility: String,
    pub file_url: String,
    pub notes: Option<String>,
    pub created_at: String,
}

/// Single production view template
#[derive(Template)]
#[template(path = "productions/production.html")]
pub struct ProductionTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub production: ProductionDetail,
    pub production_roles: Vec<String>,
    pub org_production_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionDetail {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub production_type: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub location: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub members: Vec<ProductionMemberView>,
    pub person_members: Vec<ProductionMemberView>,
    pub org_members: Vec<ProductionMemberView>,
    pub can_edit: bool,
    pub poster_url: Option<String>,
    pub poster_photo: Option<String>,
    pub header_photo: Option<String>,
    pub photos: Vec<ProductionPhotoView>,
    pub scripts: Vec<ProductionScriptView>,
    pub tmdb_url: Option<String>,
    pub release_date: Option<String>,
    pub source: String,
    pub is_claimed: bool,
    pub cast: Vec<CastCrewMember>,
    pub crew: Vec<CastCrewMember>,
    pub pending_credits: Vec<CastCrewMember>,
    pub budget_level: Option<String>,
    pub production_tier: Option<String>,
}

/// A cast or crew member on a production (from involvement graph traversal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastCrewMember {
    pub involvement_id: String,
    pub person_name: Option<String>,
    pub person_username: String,
    pub person_avatar: Option<String>,
    pub role: Option<String>,
    pub department: Option<String>,
    pub verification_status: String,
    pub person_is_identity_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionMemberView {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub slug: Option<String>,
    pub role: String,
    pub production_roles: Option<Vec<String>>,
    pub member_type: String,
    pub invitation_status: String,
    pub is_verified: bool,
}

/// Organization option for ownership dropdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgOption {
    pub id: String,
    pub name: String,
    pub role: String,
}

/// Production create form template
#[derive(Template)]
#[template(path = "productions/production_create.html")]
pub struct ProductionCreateTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub production_types: Vec<String>,
    pub production_statuses: Vec<String>,
    pub budget_levels: Vec<String>,
    pub production_tiers: Vec<String>,
    pub user_organizations: Vec<OrgOption>,
    pub production_roles: Vec<String>,
    pub org_production_roles: Vec<String>,
    pub errors: Option<Vec<String>>,
}

/// Production edit form template
#[derive(Template)]
#[template(path = "productions/production_edit.html")]
pub struct ProductionEditTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub production: ProductionEditData,
    pub production_types: Vec<String>,
    pub production_statuses: Vec<String>,
    pub budget_levels: Vec<String>,
    pub production_tiers: Vec<String>,
    pub production_roles: Vec<String>,
    pub org_production_roles: Vec<String>,
    pub members: Vec<ProductionMemberView>,
    pub person_members: Vec<ProductionMemberView>,
    pub org_members: Vec<ProductionMemberView>,
    pub errors: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionEditData {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub production_type: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub location: Option<String>,
    pub header_photo: Option<String>,
    pub poster_photo: Option<String>,
    pub photos: Vec<ProductionPhotoView>,
    pub budget_level: Option<String>,
    pub production_tier: Option<String>,
}

/// Locations page template
#[derive(Template)]
#[template(path = "locations/locations.html")]
pub struct LocationsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub locations: Vec<LocationView>,
    pub filter: Option<String>,
    pub city: Option<String>,
    pub show_private: bool,
    pub sort_by: String,
    pub liked_ids: Vec<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationView {
    pub id: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub country: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub profile_photo: Option<String>,
    pub created_at: String,
}

/// Single location view template
#[derive(Template)]
#[template(path = "locations/location.html")]
pub struct LocationTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub location: LocationDetail,
    pub is_liked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationPhoto {
    pub url: String,
    pub thumbnail_url: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationDetail {
    pub id: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub country: String,
    pub postal_code: Option<String>,
    pub description: Option<String>,
    pub contact_name: String,
    pub contact_email: String,
    pub contact_phone: Option<String>,
    pub is_public: bool,
    pub amenities: Option<Vec<String>>,
    pub restrictions: Option<Vec<String>>,
    pub parking_info: Option<String>,
    pub max_capacity: Option<i32>,
    pub profile_photo: Option<String>,
    pub photos: Vec<LocationPhoto>,
    pub created_at: String,
    pub updated_at: String,
    pub rates: Vec<RateView>,
    pub can_edit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateView {
    pub id: String,
    pub rate_type: String,
    pub amount: f64,
    pub currency: String,
    pub minimum_duration: Option<i32>,
    pub description: Option<String>,
}

/// Location create form template
#[derive(Template)]
#[template(path = "locations/location_create.html")]
pub struct LocationCreateTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub errors: Option<Vec<String>>,
}

/// Location edit form template
#[derive(Template)]
#[template(path = "locations/location_edit.html")]
pub struct LocationEditTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub location: LocationEditData,
    pub errors: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationEditData {
    pub id: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub country: String,
    pub postal_code: Option<String>,
    pub description: Option<String>,
    pub contact_name: String,
    pub contact_email: String,
    pub contact_phone: Option<String>,
    pub is_public: bool,
    pub amenities: Option<String>,
    pub restrictions: Option<String>,
    pub parking_info: Option<String>,
    pub max_capacity: Option<i32>,
    pub profile_photo: Option<String>,
    pub photos: Vec<LocationPhoto>,
}

/// People page template
#[derive(Template)]
#[template(path = "persons/people.html")]
pub struct PeopleTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub people: Vec<PersonCard>,
    pub filter: Option<String>,
    pub specialties: Vec<String>,
    pub liked_ids: Vec<String>,
    pub current_user_id: String,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonCard {
    pub id: String,
    pub name: String,
    pub username: String,
    pub headline: Option<String>,
    pub bio: Option<String>,
    pub location: Option<String>,
    pub skills: Vec<String>,
    pub avatar: String,
    pub is_identity_verified: bool,
}

/// About page template
#[derive(Template)]
#[template(path = "about/index.html")]
pub struct AboutTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub stat_creatives: usize,
    pub stat_organizations: usize,
    pub stat_locations: usize,
    pub stat_jobs: usize,
    pub stat_connections: usize,
}

#[derive(Template)]
#[template(path = "terms/index.html")]
pub struct TermsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
}

#[derive(Template)]
#[template(path = "privacy/index.html")]
pub struct PrivacyTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
}

/// Get Verified page template
#[derive(Template)]
#[template(path = "verification/get_verified.html")]
pub struct GetVerifiedTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub has_pending_request: bool,
}

/// Account settings page template
#[derive(Template)]
#[template(path = "account/settings.html")]
pub struct AccountSettingsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub username: String,
    pub email: String,
    pub messaging_preference: String,
    pub show_contact_info: bool,
    pub error: Option<String>,
    pub success: Option<String>,
}

/// Likes page template
#[derive(Template)]
#[template(path = "likes/index.html")]
pub struct LikesTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub liked_people: Vec<LikedPerson>,
    pub liked_locations: Vec<LikedLocation>,
}

/// Profile analytics page template
#[derive(Template)]
#[template(path = "persons/analytics.html")]
pub struct ProfileAnalyticsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub profile_name: String,
    pub profile_username: String,
    pub total_views: u64,
    pub unique_views: u64,
    pub likes_received: u64,
    pub views_30d: crate::models::analytics::PeriodStat,
    pub views_90d: crate::models::analytics::PeriodStat,
    pub views_1y: crate::models::analytics::PeriodStat,
    pub referrer_breakdown: Vec<crate::models::analytics::ReferrerCount>,
}

// ============================
// Job Templates
// ============================

/// View struct for job list items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListView {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub poster_name: String,
    pub poster_slug: String,
    pub poster_type: String,
    pub is_poster_verified: bool,
    pub role_count: i64,
    pub status: String,
    pub expires_at: String,
    pub created_at: String,
    pub production_title: Option<String>,
    pub production_poster: Option<String>,
    pub applications_enabled: bool,
}

pub use crate::models::job::JobRoleView;

/// View struct for job detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetailView {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub poster_name: String,
    pub poster_slug: String,
    pub poster_type: String,
    pub is_poster_verified: bool,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_website: Option<String>,
    pub applications_enabled: bool,
    pub status: String,
    pub expires_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub roles: Vec<JobRoleView>,
    pub production_title: Option<String>,
    pub production_slug: Option<String>,
    pub production_poster: Option<String>,
    pub can_edit: bool,
    pub is_expired: bool,
    pub application_count: i64,
    pub applications: Vec<ApplicationView>,
}

pub use crate::models::job::ApplicationView;

/// User's own application view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserApplicationView {
    pub id: String,
    pub job_id: String,
    pub job_title: String,
    pub role_title: String,
    pub poster_name: String,
    pub cover_letter: Option<String>,
    pub status: String,
    pub applied_at: String,
}

/// Organization option for job posting "post as" dropdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobOrgOption {
    pub id: String,
    pub name: String,
}

/// Job role data for edit form
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRoleEditData {
    pub title: String,
    pub description: Option<String>,
    pub rate_type: String,
    pub rate_amount: Option<String>,
    pub location_override: Option<String>,
}

/// Jobs list page
#[derive(Template)]
#[template(path = "jobs/jobs.html")]
pub struct JobsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub jobs: Vec<JobListView>,
    pub search_query: Option<String>,
    pub has_more: bool,
}

/// Job detail page
#[derive(Template)]
#[template(path = "jobs/job.html")]
pub struct JobTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub job: JobDetailView,
}

/// Job create form
#[derive(Template)]
#[template(path = "jobs/job_create.html")]
pub struct JobCreateTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub pay_rate_types: Vec<String>,
    pub user_organizations: Vec<JobOrgOption>,
    pub errors: Option<Vec<String>>,
}

/// Job edit form
#[derive(Template)]
#[template(path = "jobs/job_edit.html")]
pub struct JobEditTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub job_id: String,
    pub title: String,
    pub description: String,
    pub location: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_website: Option<String>,
    pub applications_enabled: bool,
    pub roles: Vec<JobRoleEditData>,
    pub pay_rate_types: Vec<String>,
    pub errors: Option<Vec<String>>,
}

/// My jobs page (postings + applications)
#[derive(Template)]
#[template(path = "jobs/my_jobs.html")]
pub struct MyJobsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub postings: Vec<JobListView>,
    pub applications: Vec<UserApplicationView>,
}

// ============================
// Equipment Templates
// ============================

pub mod equipment {
    use crate::models::equipment::{
        Equipment, EquipmentCategory, EquipmentCondition, EquipmentKit, EquipmentRental,
    };
    use crate::models::person::SessionUser;
    use askama::Template;

    /// Custom Askama filters for equipment templates
    mod filters {
        use crate::record_id_ext::RecordIdExt;
        use surrealdb::types::RecordId;

        /// Convert a relative path to an absolute URL using APP_URL
        pub fn abs_url(path: &str) -> askama::Result<String> {
            let base = crate::config::app_url();
            Ok(format!("{}{}", base, path))
        }

        /// Render a RecordId as "table:key" string for use in templates
        pub fn rid(id: &RecordId) -> askama::Result<String> {
            Ok(id.to_raw_string())
        }
    }

    /// Equipment list page template
    #[derive(Template)]
    #[template(path = "equipment/list.html")]
    pub struct EquipmentListTemplate {
        pub app_name: String,
        pub year: i32,
        pub version: String,
        pub active_page: String,
        pub user: Option<super::User>,
        pub current_user: Option<SessionUser>,
        pub equipment: Vec<Equipment>,
        pub kits: Vec<EquipmentKit>,
        pub owner_type: String,
        pub owner_id: String,
        pub page_title: String,
        pub error_message: Option<String>,
    }

    /// Equipment form template (for create/edit)
    #[derive(Template)]
    #[template(path = "equipment/form.html")]
    pub struct EquipmentFormTemplate {
        pub app_name: String,
        pub year: i32,
        pub version: String,
        pub active_page: String,
        pub user: Option<super::User>,
        pub current_user: Option<SessionUser>,
        pub equipment: Option<Equipment>,
        pub categories: Vec<EquipmentCategory>,
        pub conditions: Vec<EquipmentCondition>,
        pub owner_type: String,
        pub owner_id: String,
        pub page_title: String,
        pub error_message: Option<String>,
    }

    /// Equipment detail page template
    #[derive(Template)]
    #[template(path = "equipment/detail.html")]
    pub struct EquipmentDetailTemplate {
        pub app_name: String,
        pub year: i32,
        pub version: String,
        pub active_page: String,
        pub user: Option<super::User>,
        pub current_user: Option<SessionUser>,
        pub equipment: Equipment,
        pub rentals: Vec<EquipmentRental>,
        pub can_edit: bool,
        pub page_title: String,
        pub error_message: Option<String>,
    }

    /// Kit form template (for create/edit)
    #[derive(Template)]
    #[template(path = "equipment/kit_form.html")]
    pub struct KitFormTemplate {
        pub app_name: String,
        pub year: i32,
        pub version: String,
        pub active_page: String,
        pub user: Option<super::User>,
        pub current_user: Option<SessionUser>,
        pub kit: Option<EquipmentKit>,
        pub available_equipment: Vec<Equipment>,
        pub selected_equipment: Vec<Equipment>,
        pub categories: Vec<EquipmentCategory>,
        pub owner_type: String,
        pub owner_id: String,
        pub page_title: String,
        pub error_message: Option<String>,
    }

    /// Kit detail page template
    #[derive(Template)]
    #[template(path = "equipment/kit_detail.html")]
    pub struct KitDetailTemplate {
        pub app_name: String,
        pub year: i32,
        pub version: String,
        pub active_page: String,
        pub user: Option<super::User>,
        pub current_user: Option<SessionUser>,
        pub kit: EquipmentKit,
        pub kit_items: Vec<Equipment>,
        pub rentals: Vec<EquipmentRental>,
        pub can_edit: bool,
        pub page_title: String,
        pub error_message: Option<String>,
    }

    /// Equipment checkout form template
    #[derive(Template)]
    #[template(path = "equipment/checkout.html")]
    pub struct EquipmentCheckoutTemplate {
        pub app_name: String,
        pub year: i32,
        pub version: String,
        pub active_page: String,
        pub user: Option<super::User>,
        pub current_user: Option<SessionUser>,
        pub equipment: Option<Equipment>,
        pub kit: Option<EquipmentKit>,
        pub conditions: Vec<EquipmentCondition>,
        pub page_title: String,
        pub error_message: Option<String>,
    }

    /// Equipment check-in form template
    #[derive(Template)]
    #[template(path = "equipment/checkin.html")]
    pub struct EquipmentCheckInTemplate {
        pub app_name: String,
        pub year: i32,
        pub version: String,
        pub active_page: String,
        pub user: Option<super::User>,
        pub current_user: Option<SessionUser>,
        pub rental: EquipmentRental,
        pub conditions: Vec<EquipmentCondition>,
        pub page_title: String,
        pub error_message: Option<String>,
    }

    /// Rental history template
    #[derive(Template)]
    #[template(path = "equipment/rental_history.html")]
    pub struct RentalHistoryTemplate {
        pub app_name: String,
        pub year: i32,
        pub version: String,
        pub active_page: String,
        pub user: Option<super::User>,
        pub current_user: Option<SessionUser>,
        pub rentals: Vec<EquipmentRental>,
        pub page_title: String,
        pub error_message: Option<String>,
    }
}

// Helper struct for backwards compatibility
pub struct BaseContext {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
}

impl Default for BaseContext {
    fn default() -> Self {
        Self {
            app_name: "SlateHub".to_string(),
            year: chrono::Utc::now().year(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            active_page: String::new(),
            user: None,
        }
    }
}

impl BaseContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_page(mut self, page: &str) -> Self {
        self.active_page = page.to_string();
        self
    }

    pub fn with_user(mut self, user: User) -> Self {
        self.user = Some(user);
        self
    }
}

// Template constructors for easier creation
impl IndexTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            production_count: 0,
            user_count: 0,
            connection_count: 0,
            activities: vec![],
        }
    }
}

impl LoginTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            error: None,
            redirect_to: None,
        }
    }
}

impl SignupTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            error: None,
            prefill_email: None,
        }
    }
}

impl EmailVerificationTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            error: None,
            success: None,
            email: None,
        }
    }
}

impl ForgotPasswordTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            error: None,
            success: None,
            email: None,
        }
    }
}

impl ResetPasswordTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            error: None,
            success: None,
            email: None,
            code: None,
        }
    }
}

impl ProductionsTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            productions: vec![],
            filter: None,
            sort_by: "recent".to_string(),
            has_more: false,
        }
    }
}

impl PeopleTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            people: vec![],
            filter: None,
            specialties: vec![],
            liked_ids: vec![],
            current_user_id: String::new(),
            has_more: false,
        }
    }
}

impl AboutTemplate {
    pub fn new(
        base: BaseContext,
        stat_creatives: usize,
        stat_organizations: usize,
        stat_locations: usize,
        stat_jobs: usize,
        stat_connections: usize,
    ) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            stat_creatives,
            stat_organizations,
            stat_locations,
            stat_jobs,
            stat_connections,
        }
    }
}

impl TermsTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
        }
    }
}

impl PrivacyTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
        }
    }
}

#[derive(Template)]
#[template(path = "impressum/index.html")]
pub struct ImpressumTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
}

impl ImpressumTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
        }
    }
}

impl AccountSettingsTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            username: String::new(),
            email: String::new(),
            messaging_preference: "anyone".to_string(),
            show_contact_info: false,
            error: None,
            success: None,
        }
    }
}

pub fn base_context() -> BaseContext {
    BaseContext::new()
}

// Initialize function for compatibility (Askama compiles templates at build time)
pub fn init() -> Result<(), String> {
    // Askama compiles templates at build time, so there's no runtime initialization needed
    Ok(())
}
