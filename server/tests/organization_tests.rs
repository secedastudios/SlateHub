use slatehub::models::organization::Organization;

mod common;
use common::*;

#[tokio::test]
async fn test_create_organization() {
    with_test_db(|db| async move {
        // First create an owner user
        let owner_email = "owner@example.com";
        let owner_username = "orgowner";
        let password_hash = "hashed_password";
        let owner_id = create_test_user(&db, owner_email, owner_username, password_hash).await?;
        let owner_id_clean = owner_id.replace("users:", "");

        // Create organization
        let org_name = "Test Organization";
        let org_slug = "test-org";
        let org_id = create_test_org(&db, org_name, org_slug, &owner_id_clean).await?;

        // Verify organization was created
        assert!(!org_id.is_empty());
        assert!(org_id.starts_with("organizations:"));

        // Query the organization back
        let query = "SELECT * FROM organizations WHERE slug = $slug";
        let mut response = db.query(query).bind(("slug", org_slug)).await?;
        let orgs: Vec<serde_json::Value> = response.take(0)?;

        assert_eq!(orgs.len(), 1);
        let org = &orgs[0];
        assert_eq!(org.get("name").and_then(|n| n.as_str()), Some(org_name));
        assert_eq!(org.get("slug").and_then(|s| s.as_str()), Some(org_slug));

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_duplicate_slug_fails() {
    with_test_db(|db| async move {
        // Create owner
        let owner_id = create_test_user(&db, "owner1@example.com", "owner1", "password").await?;
        let owner_id_clean = owner_id.replace("users:", "");

        // Create first organization
        let org_slug = "duplicate-slug";
        create_test_org(&db, "Org 1", org_slug, &owner_id_clean).await?;

        // Try to create second organization with same slug
        let result = create_test_org(&db, "Org 2", org_slug, &owner_id_clean).await;

        // Should fail due to unique constraint
        assert!(result.is_err());

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_add_organization_member() {
    with_test_db(|db| async move {
        // Create owner and member users
        let owner_id = create_test_user(&db, "owner@example.com", "owner", "password").await?;
        let owner_id_clean = owner_id.replace("users:", "");

        let member_id = create_test_user(&db, "member@example.com", "member", "password").await?;
        let member_id_clean = member_id.replace("users:", "");

        // Create organization
        let org_id = create_test_org(&db, "Test Org", "test-org", &owner_id_clean).await?;
        let org_id_clean = org_id.replace("organizations:", "");

        // Add member to organization
        let add_member_query = r#"
            CREATE organization_members SET
                organization_id = type::thing('organizations', $org_id),
                user_id = type::thing('users', $user_id),
                role = $role,
                joined_at = time::now()
        "#;

        let mut response = db
            .query(add_member_query)
            .bind(("org_id", &org_id_clean))
            .bind(("user_id", &member_id_clean))
            .bind(("role", "member"))
            .await?;

        let membership: Option<serde_json::Value> = response.take(0)?;
        assert!(membership.is_some());

        // Verify membership
        let query = "SELECT * FROM organization_members WHERE organization_id = type::thing('organizations', $org_id)";
        let mut response = db.query(query).bind(("org_id", &org_id_clean)).await?;
        let members: Vec<serde_json::Value> = response.take(0)?;

        // Should have at least one member (we added)
        assert!(members.len() >= 1);

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_update_organization() {
    with_test_db(|db| async move {
        // Create owner
        let owner_id = create_test_user(&db, "owner@example.com", "owner", "password").await?;
        let owner_id_clean = owner_id.replace("users:", "");

        // Create organization
        let org_id =
            create_test_org(&db, "Original Name", "original-slug", &owner_id_clean).await?;

        // Update organization
        let update_query = r#"
            UPDATE $org_id SET
                name = $new_name,
                description = $description,
                updated_at = time::now()
        "#;

        db.query(update_query)
            .bind(("org_id", &org_id))
            .bind(("new_name", "Updated Organization"))
            .bind(("description", "This is an updated description"))
            .await?;

        // Verify update
        let query = format!("SELECT * FROM {}", org_id);
        let mut response = db.query(&query).await?;
        let orgs: Vec<serde_json::Value> = response.take(0)?;

        assert_eq!(orgs.len(), 1);
        assert_eq!(
            orgs[0].get("name").and_then(|n| n.as_str()),
            Some("Updated Organization")
        );
        assert_eq!(
            orgs[0].get("description").and_then(|d| d.as_str()),
            Some("This is an updated description")
        );

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_delete_organization() {
    with_test_db(|db| async move {
        // Create owner
        let owner_id = create_test_user(&db, "owner@example.com", "owner", "password").await?;
        let owner_id_clean = owner_id.replace("users:", "");

        // Create organization
        let org_id = create_test_org(&db, "To Delete", "to-delete", &owner_id_clean).await?;

        // Delete organization
        let delete_query = format!("DELETE {}", org_id);
        db.query(&delete_query).await?;

        // Verify deletion
        let query = format!("SELECT * FROM {}", org_id);
        let mut response = db.query(&query).await?;
        let orgs: Vec<serde_json::Value> = response.take(0)?;

        assert_eq!(orgs.len(), 0);

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_list_user_organizations() {
    with_test_db(|db| async move {
        // Create user
        let user_id = create_test_user(&db, "user@example.com", "user", "password").await?;
        let user_id_clean = user_id.replace("users:", "");

        // Create multiple organizations
        let org1_id = create_test_org(&db, "Org 1", "org-1", &user_id_clean).await?;
        let org2_id = create_test_org(&db, "Org 2", "org-2", &user_id_clean).await?;
        let org3_id = create_test_org(&db, "Org 3", "org-3", &user_id_clean).await?;

        // Query user's organizations
        let query = "SELECT * FROM organizations WHERE owner_id = type::thing('users', $user_id) ORDER BY created_at";
        let mut response = db.query(query).bind(("user_id", &user_id_clean)).await?;
        let orgs: Vec<serde_json::Value> = response.take(0)?;

        assert_eq!(orgs.len(), 3);
        assert_eq!(orgs[0].get("name").and_then(|n| n.as_str()), Some("Org 1"));
        assert_eq!(orgs[1].get("name").and_then(|n| n.as_str()), Some("Org 2"));
        assert_eq!(orgs[2].get("name").and_then(|n| n.as_str()), Some("Org 3"));

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_organization_with_projects() {
    with_test_db(|db| async move {
        // Create owner
        let owner_id = create_test_user(&db, "owner@example.com", "owner", "password").await?;
        let owner_id_clean = owner_id.replace("users:", "");

        // Create organization
        let org_id = create_test_org(&db, "Project Org", "project-org", &owner_id_clean).await?;
        let org_id_clean = org_id.replace("organizations:", "");

        // Create projects in organization
        let project1_id =
            create_test_project(&db, "Project 1", &owner_id_clean, Some(&org_id_clean)).await?;
        let project2_id =
            create_test_project(&db, "Project 2", &owner_id_clean, Some(&org_id_clean)).await?;

        // Query organization's projects
        let query =
            "SELECT * FROM projects WHERE organization_id = type::thing('organizations', $org_id)";
        let mut response = db.query(query).bind(("org_id", &org_id_clean)).await?;
        let projects: Vec<serde_json::Value> = response.take(0)?;

        assert_eq!(projects.len(), 2);

        let titles: Vec<&str> = projects
            .iter()
            .filter_map(|p| p.get("title").and_then(|t| t.as_str()))
            .collect();

        assert!(titles.contains(&"Project 1"));
        assert!(titles.contains(&"Project 2"));

        Ok(())
    })
    .await
    .expect("Test failed");
}
