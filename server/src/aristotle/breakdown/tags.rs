//! Element-tag write-back helpers shared across breakdown tiers.

use crate::aristotle::models::{ScreenplayElement, Tag, TagSource};

/// Append a tag to an element, but only if no existing tag has the same
/// (category, value) pair — comparison is case-insensitive on both.
/// Returns true if a new tag was added.
pub fn add_tag_if_new(
    element: &mut ScreenplayElement,
    category: &str,
    value: Option<String>,
    source: TagSource,
    confidence: f32,
) -> bool {
    let cat_key = category.to_ascii_lowercase();
    let val_key = value.as_deref().map(str::to_ascii_lowercase);

    let exists = element.tags.iter().any(|t| {
        t.category.to_ascii_lowercase() == cat_key
            && t.value.as_deref().map(str::to_ascii_lowercase) == val_key
    });

    if exists {
        return false;
    }

    element.tags.push(Tag {
        category: category.to_string(),
        value,
        source,
        confidence,
    });
    true
}
