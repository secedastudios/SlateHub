use slatehub::auth;
use slatehub::models::user::User;

mod common;
use common::*;

#[tokio::test]
async fn test_create_user() {
    with_test_db(|db| async move {
        // Create a test user
        let email = "test@example.com";
        let username = "testuser";
        let password = "TestPassword123!";
        let password_hash = auth::hash_password(password).await?;

        let user_id = create_test_user(&db, email, username, &password_hash).await?;

        // Verify user was created
        assert!(!user_id.is_empty());
        assert!(user_id.starts_with("users:"));

        // Query the user back
        let query = "SELECT * FROM users WHERE email = $email";
        let mut response = db.query(query).bind(("email", email)).await?;
        let users: Vec<serde_json::Value> = response.take(0)?;

        assert_eq!(users.len(), 1);
        let user = &users[0];
        assert_eq!(user.get("email").and_then(|e| e.as_str()), Some(email));
        assert_eq!(
            user.get("username").and_then(|u| u.as_str()),
            Some(username)
        );
        assert_eq!(user.get("is_active").and_then(|a| a.as_bool()), Some(true));

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_duplicate_email_fails() {
    with_test_db(|db| async move {
        let email = "duplicate@example.com";
        let password_hash = auth::hash_password("password").await?;

        // Create first user
        create_test_user(&db, email, "user1", &password_hash).await?;

        // Try to create second user with same email
        let result = create_test_user(&db, email, "user2", &password_hash).await;

        // Should fail due to unique constraint
        assert!(result.is_err());

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_duplicate_username_fails() {
    with_test_db(|db| async move {
        let username = "duplicateuser";
        let password_hash = auth::hash_password("password").await?;

        // Create first user
        create_test_user(&db, "user1@example.com", username, &password_hash).await?;

        // Try to create second user with same username
        let result = create_test_user(&db, "user2@example.com", username, &password_hash).await;

        // Should fail due to unique constraint
        assert!(result.is_err());

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_user_authentication() {
    with_test_db(|db| async move {
        let email = "auth@example.com";
        let username = "authuser";
        let password = "SecurePassword123!";
        let password_hash = auth::hash_password(password).await?;

        // Create user
        let user_id = create_test_user(&db, email, username, &password_hash).await?;

        // Verify password
        let is_valid = auth::verify_password(password, &password_hash).await?;
        assert!(is_valid);

        // Verify wrong password fails
        let is_invalid = auth::verify_password("WrongPassword", &password_hash).await?;
        assert!(!is_invalid);

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_update_user() {
    with_test_db(|db| async move {
        let email = "update@example.com";
        let username = "updateuser";
        let password_hash = auth::hash_password("password").await?;

        // Create user
        let user_id = create_test_user(&db, email, username, &password_hash).await?;

        // Update user
        let update_query = r#"
            UPDATE $user_id SET
                username = $new_username,
                updated_at = time::now()
        "#;

        db.query(update_query)
            .bind(("user_id", &user_id))
            .bind(("new_username", "newusername"))
            .await?;

        // Verify update
        let query = format!("SELECT * FROM {}", user_id);
        let mut response = db.query(&query).await?;
        let users: Vec<serde_json::Value> = response.take(0)?;

        assert_eq!(users.len(), 1);
        assert_eq!(
            users[0].get("username").and_then(|u| u.as_str()),
            Some("newusername")
        );

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_delete_user() {
    with_test_db(|db| async move {
        let email = "delete@example.com";
        let username = "deleteuser";
        let password_hash = auth::hash_password("password").await?;

        // Create user
        let user_id = create_test_user(&db, email, username, &password_hash).await?;

        // Delete user
        let delete_query = format!("DELETE {}", user_id);
        db.query(&delete_query).await?;

        // Verify deletion
        let query = format!("SELECT * FROM {}", user_id);
        let mut response = db.query(&query).await?;
        let users: Vec<serde_json::Value> = response.take(0)?;

        assert_eq!(users.len(), 0);

        Ok(())
    })
    .await
    .expect("Test failed");
}

#[tokio::test]
async fn test_user_profile_creation() {
    with_test_db(|db| async move {
        let email = "profile@example.com";
        let username = "profileuser";
        let password_hash = auth::hash_password("password").await?;

        // Create user
        let user_id = create_test_user(&db, email, username, &password_hash).await?;

        // Create profile for user
        let profile_query = r#"
            CREATE profiles SET
                user_id = type::thing('users', $user_id),
                display_name = $display_name,
                bio = $bio
        "#;

        let mut response = db
            .query(profile_query)
            .bind(("user_id", user_id.replace("users:", "")))
            .bind(("display_name", "Test User"))
            .bind(("bio", "This is a test bio"))
            .await?;

        let profile: Option<serde_json::Value> = response.take(0)?;
        assert!(profile.is_some());

        let profile = profile.unwrap();
        assert_eq!(
            profile.get("display_name").and_then(|d| d.as_str()),
            Some("Test User")
        );
        assert_eq!(
            profile.get("bio").and_then(|b| b.as_str()),
            Some("This is a test bio")
        );

        Ok(())
    })
    .await
    .expect("Test failed");
}
