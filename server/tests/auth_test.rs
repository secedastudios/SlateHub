use slatehub::auth::{create_jwt, decode_jwt, hash_password_sync, verify_password_sync};

#[test]
fn test_password_hashing() {
    let password = "test_password_123";
    let hash = hash_password_sync(password).expect("Should hash password");

    assert!(hash.starts_with("$argon2id$"));
    assert!(hash.contains("$m=19456,t=2,p=1$"));

    assert!(verify_password_sync(password, &hash).expect("Should verify password"));
    assert!(!verify_password_sync("wrong_password", &hash).expect("Should verify password"));
}

#[test]
fn test_jwt_creation_and_validation() {
    // create_jwt reads JWT_SECRET from the environment; tests must not
    // depend on the shell having it set.
    // SAFETY: tests run with --test-threads=1, so env mutation is safe.
    unsafe {
        std::env::set_var("JWT_SECRET", "test_secret_for_jwt_unit_test_only");
    }

    let user_id = "person:test123";
    let username = "testuser";
    let email = "test@example.com";

    let token = create_jwt(user_id, username, email).expect("Should create JWT");
    assert!(!token.is_empty());

    let claims = decode_jwt(&token).expect("Should decode JWT");
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.username, username);
    assert_eq!(claims.email, email);
}
