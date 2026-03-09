use slatehub::auth::{create_jwt, decode_jwt, hash_password, verify_password};

#[test]
fn test_password_hashing() {
    let password = "test_password_123";
    let hash = hash_password(password).expect("Should hash password");

    assert!(hash.starts_with("$argon2id$"));
    assert!(hash.contains("$m=19456,t=2,p=1$"));

    assert!(verify_password(password, &hash).expect("Should verify password"));
    assert!(!verify_password("wrong_password", &hash).expect("Should verify password"));
}

#[test]
fn test_jwt_creation_and_validation() {
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
