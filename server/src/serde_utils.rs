//! Custom serde deserializers for handling form data
//!
//! This module provides deserializers that handle HTML form quirks,
//! such as empty strings being sent for empty numeric fields.

use serde::{Deserialize, Deserializer};

/// Deserialize an optional i32 from a string that might be empty
///
/// HTML forms send empty strings for empty number inputs instead of omitting them.
/// This deserializer treats empty strings as None.
///
/// # Example
/// ```ignore
/// #[derive(Deserialize)]
/// struct MyForm {
///     #[serde(deserialize_with = "deserialize_optional_i32")]
///     max_capacity: Option<i32>,
/// }
/// ```
pub fn deserialize_optional_i32<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => s
            .trim()
            .parse::<i32>()
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format!("Invalid integer: {}", e))),
    }
}

/// Deserialize an optional u32 from a string that might be empty
pub fn deserialize_optional_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => s
            .trim()
            .parse::<u32>()
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format!("Invalid positive integer: {}", e))),
    }
}

/// Deserialize an optional i64 from a string that might be empty
pub fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => s
            .trim()
            .parse::<i64>()
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format!("Invalid number: {}", e))),
    }
}

/// Deserialize an optional f64 from a string that might be empty
pub fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => s
            .trim()
            .parse::<f64>()
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format!("Invalid decimal number: {}", e))),
    }
}

/// Deserialize an optional string, treating empty strings as None
///
/// This is useful for optional text fields where an empty string should be None.
pub fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    Ok(s.filter(|s| !s.trim().is_empty()))
}

/// Deserialize a vector of strings from a comma-separated string
///
/// Useful for form fields that accept comma-separated values.
/// Empty strings result in an empty vector.
///
/// # Example
/// ```ignore
/// #[derive(Deserialize)]
/// struct MyForm {
///     #[serde(deserialize_with = "deserialize_string_list")]
///     tags: Vec<String>,
/// }
/// ```
pub fn deserialize_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(Vec::new()),
        Some(s) if s.trim().is_empty() => Ok(Vec::new()),
        Some(s) => Ok(s
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()),
    }
}

/// Deserialize an optional vector of strings from a comma-separated string
pub fn deserialize_optional_string_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => {
            let items: Vec<String> = s
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect();

            if items.is_empty() {
                Ok(None)
            } else {
                Ok(Some(items))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

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
}
