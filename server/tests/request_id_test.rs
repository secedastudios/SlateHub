use slatehub::middleware::request_id::{is_valid_request_id, RequestId};

#[test]
fn test_valid_request_ids() {
    // Valid ULIDs
    assert!(is_valid_request_id("01AN4Z07BY79KA1307SR9X4MV3"));
    assert!(is_valid_request_id("01ARYZ6S41TSV4RRFFQ69G5FAV"));

    // Valid UUIDs
    assert!(is_valid_request_id("550e8400-e29b-41d4-a716-446655440000"));
    assert!(is_valid_request_id("550e8400e29b41d4a716446655440000"));

    // Valid alphanumeric IDs
    assert!(is_valid_request_id("abc123def456"));
    assert!(is_valid_request_id("trace_123_456"));
    assert!(is_valid_request_id("correlation.id.789"));
    assert!(is_valid_request_id("req-2024-01-15-001"));
}

#[test]
fn test_invalid_request_ids() {
    // Too short
    assert!(!is_valid_request_id("abc"));

    // Too long
    let long_id = "a".repeat(129);
    assert!(!is_valid_request_id(&long_id));

    // Invalid characters
    assert!(!is_valid_request_id("abc@123"));
    assert!(!is_valid_request_id("abc#def"));
    assert!(!is_valid_request_id("id with spaces"));
    assert!(!is_valid_request_id("id/with/slashes"));
}

#[test]
fn test_request_id_new() {
    let id1 = RequestId::new();
    let id2 = RequestId::new();

    assert_ne!(id1.as_str(), id2.as_str());
    assert!(is_valid_request_id(id1.as_str()));
    assert!(is_valid_request_id(id2.as_str()));
    assert_eq!(id1.as_str().len(), 26);
    assert_eq!(id2.as_str().len(), 26);
}

#[test]
fn test_request_id_from_string() {
    let original = "test-request-id-123";
    let id = RequestId::from_string(original.to_string());
    assert_eq!(id.as_str(), original);
}

#[test]
fn test_request_id_display() {
    let id = RequestId::from_string("display-test".to_string());
    assert_eq!(format!("{}", id), "display-test");
}
