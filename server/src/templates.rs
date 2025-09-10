use askama::Template;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

/// User information for templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar: String,
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
    pub project_count: u32,
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
#[template(path = "login.html")]
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
#[template(path = "signup.html")]
pub struct SignupTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub error: Option<String>,
}

/// Profile page template
#[derive(Template)]
#[template(path = "profile.html")]
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
#[template(path = "profile_edit.html")]
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

/// Projects page template
#[derive(Template)]
#[template(path = "projects.html")]
pub struct ProjectsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub projects: Vec<Project>,
    pub filter: Option<String>,
    pub sort_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
    pub owner: String,
    pub tags: Vec<String>,
}

/// People page template
#[derive(Template)]
#[template(path = "people.html")]
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
#[template(path = "about.html")]
pub struct AboutTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
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
            project_count: 0,
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

impl ProjectsTemplate {
    pub fn new(base: BaseContext) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            projects: vec![],
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

// Helper function for backwards compatibility
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
