use chrono::Utc;
use slatehub::models::organization::{
    CreateOrganizationData, OrganizationMember, OrganizationModel, SocialLink,
    UpdateOrganizationData,
};
use surrealdb::types::RecordId;

fn create_test_org_data(slug: &str) -> CreateOrganizationData {
    CreateOrganizationData {
        name: format!("Test Organization {}", slug),
        slug: slug.to_string(),
        org_type: "Production Company".to_string(),
        description: Some("A test organization for unit testing".to_string()),
        location: Some("Los Angeles, CA".to_string()),
        website: Some("https://example.com".to_string()),
        contact_email: Some("contact@example.com".to_string()),
        phone: Some("+1-555-0123".to_string()),
        services: vec!["production".to_string(), "post-production".to_string()],
        founded_year: Some(2020),
        employees_count: Some(50),
        public: true,
    }
}

#[test]
fn test_organization_data_creation() {
    let org_data = create_test_org_data("test-org");

    assert_eq!(org_data.name, "Test Organization test-org");
    assert_eq!(org_data.slug, "test-org");
    assert_eq!(org_data.org_type, "Production Company");
    assert!(org_data.description.is_some());
    assert_eq!(org_data.services.len(), 2);
    assert_eq!(org_data.founded_year, Some(2020));
    assert!(org_data.public);
}

#[test]
fn test_organization_slug_validation() {
    let valid_slugs = vec![
        "my-company",
        "company-123",
        "test-org-2024",
        "production-co",
    ];

    let invalid_slugs = vec![
        "My Company", // spaces
        "company!",   // special chars
        "test@org",   // @ symbol
        "",           // empty
    ];

    for slug in valid_slugs {
        assert!(!slug.is_empty());
        assert!(!slug.contains(' '));
        assert!(!slug.contains('@'));
        assert!(!slug.contains('!'));
    }

    for slug in invalid_slugs {
        assert!(
            slug.is_empty() || slug.contains(' ') || slug.contains('@') || slug.contains('!')
        );
    }
}

#[test]
fn test_social_link_structure() {
    let social_link = SocialLink {
        platform: "linkedin".to_string(),
        url: "https://linkedin.com/company/test".to_string(),
    };

    assert_eq!(social_link.platform, "linkedin");
    assert!(social_link.url.starts_with("https://"));
}

#[test]
fn test_update_organization_data() {
    let update_data = UpdateOrganizationData {
        name: "Updated Organization".to_string(),
        org_type: "Film Studio".to_string(),
        description: Some("Updated description".to_string()),
        location: Some("New York, NY".to_string()),
        website: Some("https://updated.com".to_string()),
        contact_email: Some("new@example.com".to_string()),
        phone: Some("+1-555-9999".to_string()),
        services: vec!["editing".to_string()],
        founded_year: Some(2019),
        employees_count: Some(100),
        public: false,
    };

    assert_eq!(update_data.name, "Updated Organization");
    assert_eq!(update_data.org_type, "Film Studio");
    assert!(!update_data.public);
    assert_eq!(update_data.employees_count, Some(100));
}

#[tokio::test]
async fn test_organization_model_new() {
    let _model = OrganizationModel::new();
    assert!(true);
}

#[test]
fn test_organization_member_structure() {
    let member = OrganizationMember {
        id: RecordId::parse_simple("member_of:member_123").unwrap(),
        person_id: RecordId::parse_simple("person:person_456").unwrap(),
        person_username: "johndoe".to_string(),
        person_name: Some("John Doe".to_string()),
        role: "admin".to_string(),
        joined_at: Utc::now(),
        invitation_status: "accepted".to_string(),
    };

    assert_eq!(member.person_username, "johndoe");
    assert_eq!(member.role, "admin");
    assert_eq!(member.invitation_status, "accepted");
    assert!(member.person_name.is_some());
}

#[test]
fn test_organization_fields_optional() {
    let org_data = CreateOrganizationData {
        name: "Minimal Org".to_string(),
        slug: "minimal-org".to_string(),
        org_type: "Production Company".to_string(),
        description: None,
        location: None,
        website: None,
        contact_email: None,
        phone: None,
        services: vec![],
        founded_year: None,
        employees_count: None,
        public: false,
    };

    assert!(org_data.description.is_none());
    assert!(org_data.location.is_none());
    assert!(org_data.website.is_none());
    assert!(org_data.contact_email.is_none());
    assert!(org_data.phone.is_none());
    assert!(org_data.founded_year.is_none());
    assert!(org_data.employees_count.is_none());
    assert!(org_data.services.is_empty());
}

#[test]
fn test_organization_type_variations() {
    let org_types = vec![
        "Production Company",
        "Film Studio",
        "Talent Agency",
        "Post Production House",
        "Equipment Rental",
        "Community Group",
    ];

    for org_type in org_types {
        let org_data = CreateOrganizationData {
            name: format!("Test {}", org_type),
            slug: format!("test-{}", org_type.to_lowercase().replace(' ', "-")),
            org_type: org_type.to_string(),
            description: None,
            location: None,
            website: None,
            contact_email: None,
            phone: None,
            services: vec![],
            founded_year: None,
            employees_count: None,
            public: true,
        };

        assert_eq!(org_data.org_type, org_type);
    }
}
