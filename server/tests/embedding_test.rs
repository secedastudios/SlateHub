use slatehub::services::embedding::{build_location_embedding_text, build_person_embedding_text};

#[test]
fn test_person_embedding_text() {
    let text = build_person_embedding_text(
        "John Doe",
        Some("Actor"),
        Some("Experienced theater performer"),
        &vec!["acting".to_string(), "singing".to_string()],
        Some("Los Angeles, CA"),
        Some((25, 35)),
        Some("male"),
        &vec!["caucasian".to_string()],
        Some(180),
        Some("athletic"),
        Some("brown"),
        Some("blue"),
        &vec!["English".to_string(), "Spanish".to_string()],
        &vec!["SAG-AFTRA".to_string()],
        &vec!["Broadway musical theater".to_string()],
    );

    assert!(text.contains("John Doe"));
    assert!(text.contains("Actor"));
    assert!(text.contains("male"));
    assert!(text.contains("25-35 years old"));
    assert!(text.contains("Los Angeles"));
    assert!(text.contains("acting, singing"));
}

#[test]
fn test_location_embedding_text() {
    let text = build_location_embedding_text(
        "Modern Office Space",
        Some("Bright, modern office with floor-to-ceiling windows and natural light"),
        "Los Angeles",
        "CA",
        "USA",
        &vec!["natural light".to_string(), "modern furniture".to_string()],
        &vec!["no smoking".to_string()],
        Some(50),
        Some("Street parking available"),
    );

    assert!(text.contains("Modern Office Space"));
    assert!(text.contains("Los Angeles, CA"));
    assert!(text.contains("natural light"));
    assert!(text.contains("50 people"));
}
