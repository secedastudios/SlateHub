use serde::Deserialize;
use serde_json::json;
use slatehub::serde_utils::{
    deserialize_optional_i32, deserialize_optional_string, deserialize_string_list,
};

#[derive(Deserialize, Debug, PartialEq)]
struct TestForm {
    #[serde(deserialize_with = "deserialize_optional_i32")]
    max_capacity: Option<i32>,

    #[serde(deserialize_with = "deserialize_optional_string")]
    description: Option<String>,

    #[serde(deserialize_with = "deserialize_string_list")]
    tags: Vec<String>,
}

#[test]
fn test_deserialize_optional_i32() {
    // Test with empty string
    let json = json!({ "max_capacity": "", "description": "", "tags": "" });
    let form: TestForm = serde_json::from_value(json).unwrap();
    assert_eq!(form.max_capacity, None);

    // Test with valid number
    let json = json!({ "max_capacity": "42", "description": "", "tags": "" });
    let form: TestForm = serde_json::from_value(json).unwrap();
    assert_eq!(form.max_capacity, Some(42));

    // Test with null
    let json = json!({ "max_capacity": null, "description": "", "tags": "" });
    let form: TestForm = serde_json::from_value(json).unwrap();
    assert_eq!(form.max_capacity, None);
}

#[test]
fn test_deserialize_optional_string() {
    let json = json!({ "max_capacity": "", "description": "", "tags": "" });
    let form: TestForm = serde_json::from_value(json).unwrap();
    assert_eq!(form.description, None);

    let json = json!({ "max_capacity": "", "description": "Test", "tags": "" });
    let form: TestForm = serde_json::from_value(json).unwrap();
    assert_eq!(form.description, Some("Test".to_string()));
}

#[test]
fn test_deserialize_string_list() {
    let json = json!({ "max_capacity": "", "description": "", "tags": "" });
    let form: TestForm = serde_json::from_value(json).unwrap();
    assert_eq!(form.tags, Vec::<String>::new());

    let json = json!({ "max_capacity": "", "description": "", "tags": "tag1, tag2, tag3" });
    let form: TestForm = serde_json::from_value(json).unwrap();
    assert_eq!(form.tags, vec!["tag1", "tag2", "tag3"]);
}
