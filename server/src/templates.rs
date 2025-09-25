use askama::Template;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::db::DB;
// use crate::models::equipment::{
//     Equipment, EquipmentCategory, EquipmentCondition, EquipmentKit, EquipmentRental,
// };
use crate::models::person::SessionUser;

/// Represents a user for template rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar: String,             // Compatibility field - either URL or empty
    pub avatar_url: Option<String>, // Actual profile image URL if exists
    pub initials: String,           // Fallback initials from username/name
}

impl User {
    /// Create a User from a SessionUser, fetching the actual avatar URL from the database
    pub async fn from_session_user(session_user: &SessionUser) -> Self {
        // Generate initials from name or username
        let initials = Self::generate_initials(&session_user.name);

        // Try to fetch the avatar URL from the database
        let avatar_url = Self::fetch_avatar_url(&session_user.id).await;

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

    /// Fetch avatar URL from the database
    async fn fetch_avatar_url(person_id: &str) -> Option<String> {
        // Ensure we have full record ID
        let person_record = if person_id.starts_with("person:") {
            person_id.to_string()
        } else {
            format!("person:{}", person_id)
        };

        // Query for the person's avatar URL
        let sql = format!("SELECT profile.avatar FROM {} LIMIT 1", person_record);

        if let Ok(mut response) = DB.query(&sql).await {
            if let Ok(result) = response.take::<Option<serde_json::Value>>(0) {
                if let Some(data) = result {
                    if let Some(avatar_url) = data
                        .get("profile")
                        .and_then(|p| p.get("avatar"))
                        .and_then(|a| a.as_str())
                    {
                        return Some(avatar_url.to_string());
                    }
                }
            }
        }

        None
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
    pub experience: Vec<Experience>,
    pub education: Vec<Education>,
    pub is_own_profile: bool,
    pub is_public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub role: String,
    pub production: Option<String>,
    pub description: Option<String>,
    pub dates: Option<DateRange>,
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
    pub error: Option<String>,
    pub success: Option<String>,
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
    pub can_edit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionMemberView {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub slug: Option<String>,
    pub role: String,
    pub member_type: String,
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
    pub show_private: bool,
    pub sort_by: String,
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
    pub amenities: Option<String>, // Comma-separated string for editing
    pub restrictions: Option<String>, // Comma-separated string for editing
    pub parking_info: Option<String>,
    pub max_capacity: Option<i32>,
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

// ============================
// Equipment Templates
// ============================

pub mod equipment {
    use crate::models::equipment::{
        Equipment, EquipmentCategory, EquipmentCondition, EquipmentKit, EquipmentRental,
    };
    use crate::models::person::SessionUser;
    use askama::Template;

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
        }
    }
}

impl AboutTemplate {
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

pub fn base_context() -> BaseContext {
    BaseContext::new()
}

// Initialize function for compatibility (Askama compiles templates at build time)
pub fn init() -> Result<(), String> {
    // Askama compiles templates at build time, so there's no runtime initialization needed
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_context() {
        let context = BaseContext::new();
        assert_eq!(context.app_name, "SlateHub");
        assert!(context.user.is_none());
    }

    #[test]
    fn test_base_context_with_page() {
        let context = BaseContext::new().with_page("home");
        assert_eq!(context.active_page, "home");
    }

    #[test]
    fn test_template_creation() {
        let base = BaseContext::new().with_page("index");
        let template = IndexTemplate::new(base);
        assert_eq!(template.active_page, "index");
        assert_eq!(template.app_name, "SlateHub");
    }
}
