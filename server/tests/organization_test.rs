mod common;

use slatehub::db::DB;
use slatehub::models::organization::{CreateOrganizationData, OrganizationModel};
use surrealdb::types::SurrealValue;

async fn seed_org_type() -> String {
    #[derive(serde::Deserialize, SurrealValue)]
    struct OrgType {
        id: String,
    }

    let mut response = DB
        .query("SELECT string::concat('organization_type:', meta::id(id)) AS id FROM organization_type LIMIT 1")
        .await
        .expect("Failed to query org types");

    let result: Vec<OrgType> = response.take(0).expect("Failed to take org type result");

    assert!(
        !result.is_empty(),
        "No organization types found — did you run make test-db-init?"
    );

    result[0].id.clone()
}

async fn seed_test_person() -> String {
    #[derive(serde::Deserialize, SurrealValue)]
    struct PersonId {
        id: String,
    }

    let mut response = DB
        .query(
            "CREATE person CONTENT {
                email: 'test@example.com',
                password: 'hashed_password',
                username: 'testuser',
                profile: { name: 'Test User', skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
            } RETURN string::concat('person:', meta::id(id)) AS id",
        )
        .await
        .expect("Failed to create test person");

    let result: Vec<PersonId> = response.take(0).expect("Failed to take person result");
    assert!(!result.is_empty(), "No person record returned from CREATE");
    result[0].id.clone()
}

fn make_org_data(slug: &str, org_type: &str) -> CreateOrganizationData {
    CreateOrganizationData {
        name: format!("Test Org {slug}"),
        slug: slug.to_string(),
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
    }
}

fn clean_all() {
    common::clean_table("member_of");
    common::clean_table("organization");
    common::clean_table("person");
}

#[test]
fn test_create_organization_success() {
    common::setup_test_db();
    clean_all();

    common::run(async {
        let org_type = seed_org_type().await;
        let person_id = seed_test_person().await;

        let model = OrganizationModel::new();
        let data = make_org_data("test-org", &org_type);

        let org = model.create(data, &person_id).await;
        assert!(
            org.is_ok(),
            "Expected org creation to succeed: {:?}",
            org.err()
        );

        let org = org.unwrap();
        assert_eq!(org.slug, "test-org");
        assert_eq!(org.name, "Test Org test-org");
        assert!(org.public);
    });
}

#[test]
fn test_create_organization_duplicate_slug() {
    common::setup_test_db();
    clean_all();

    common::run(async {
        let org_type = seed_org_type().await;
        let person_id = seed_test_person().await;

        let model = OrganizationModel::new();

        // First creation should succeed
        let data = make_org_data("dupe-slug", &org_type);
        let result = model.create(data, &person_id).await;
        assert!(result.is_ok(), "First creation failed: {:?}", result.err());

        // Second creation with same slug should fail
        let data2 = make_org_data("dupe-slug", &org_type);
        let result2 = model.create(data2, &person_id).await;
        assert!(result2.is_err(), "Expected duplicate slug to fail");

        let err = result2.unwrap_err();
        let err_str = format!("{err}");
        assert!(
            err_str.contains("already taken"),
            "Expected 'already taken' error, got: {err_str}"
        );
    });
}

#[test]
fn test_create_organization_invalid_type() {
    common::setup_test_db();
    clean_all();

    common::run(async {
        let person_id = seed_test_person().await;

        let model = OrganizationModel::new();
        let data = make_org_data("valid-slug", "organization_type:nonexistent");

        let result = model.create(data, &person_id).await;
        assert!(result.is_err(), "Expected invalid org type to fail");

        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("Invalid organization type"),
            "Expected validation error about org type, got: {err}"
        );
    });
}
