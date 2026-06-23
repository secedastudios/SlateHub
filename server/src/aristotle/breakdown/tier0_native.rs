//! Tier 0 — ingest native breakdown markup carried by the source format.
//!
//! Final Draft Tagger emits `<TagCategories>` / `<TagDefinitions>` and
//! inline `TagDef` references; the FDX parser turns those into
//! [`Tag`](crate::aristotle::models::Tag) entries on each element with
//! `source = NativeFdx`. Fountain `[[CAT: value]]` notes produce the same
//! shape with `source = NativeFountain`.
//!
//! Here we walk every element's tags and map the category name into one
//! of the [`SceneBreakdown`](crate::aristotle::models::SceneBreakdown) list fields.
//! Unknown categories are dropped silently — the original tags remain on
//! the element for the UI to display.

use crate::aristotle::breakdown::builder::{Builder, Field};
use crate::aristotle::models::ParsedScene;

pub fn apply(builder: &mut Builder, scene: &mut ParsedScene) {
    for element in &scene.elements {
        for tag in &element.tags {
            let value = tag.value.clone().unwrap_or_else(|| element.text.clone());
            let Some(field) = field_for_category(&tag.category) else {
                continue;
            };
            builder.add(field, value);
        }
    }
}

/// Map a tag category name onto a breakdown field. Casing and punctuation
/// vary across tools (Final Draft uses `"Cast Members"`; Movie Magic uses
/// `"Cast"`), so we normalize aggressively.
fn field_for_category(name: &str) -> Option<Field> {
    // Normalize: lowercase, replace any non-alphanumeric with a space
    // (so `/`, `-`, `&` all split words), collapse whitespace.
    let key: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    Some(match key.as_str() {
        "cast" | "cast members" | "cast member" | "speaking cast" => Field::Cast,
        "silent" | "silent cast" | "non speaking" | "non speaking cast" => Field::Cast,
        "extras" | "background" | "background actors" | "atmosphere" => Field::ExtrasBackground,
        "props" | "prop" | "set dressing" | "vehicles" | "vehicle" | "weapons" | "weapon" => {
            Field::Props
        }
        "wardrobe" | "costume" | "costumes" => Field::Wardrobe,
        "makeup" | "hair" | "makeup hair" | "makeup and hair" => Field::MakeupHair,
        "special effects" | "sfx" | "special effect" | "practical effects" => Field::SpecialEffects,
        "visual effects" | "vfx" | "visual effect" => Field::VisualEffects,
        "stunts" | "stunt" => Field::Stunts,
        "animals" | "animal" => Field::Animals,
        "sound effects" | "sound" | "sfx audio" => Field::SoundEffects,
        "music" | "songs" | "score" => Field::Music,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str) -> Option<Field> {
        field_for_category(name)
    }

    #[test]
    fn final_draft_vocabulary_maps_correctly() {
        assert!(matches!(field("Cast Members"), Some(Field::Cast)));
        assert!(matches!(
            field("Background Actors"),
            Some(Field::ExtrasBackground)
        ));
        assert!(matches!(
            field("Special Effects"),
            Some(Field::SpecialEffects)
        ));
        assert!(matches!(
            field("Visual Effects"),
            Some(Field::VisualEffects)
        ));
    }

    #[test]
    fn movie_magic_vocabulary_maps_correctly() {
        assert!(matches!(field("Cast"), Some(Field::Cast)));
        assert!(matches!(field("Extras"), Some(Field::ExtrasBackground)));
        assert!(matches!(field("Props"), Some(Field::Props)));
        assert!(matches!(field("Wardrobe"), Some(Field::Wardrobe)));
    }

    #[test]
    fn silent_and_non_speaking_map_to_cast() {
        assert!(matches!(field("Silent"), Some(Field::Cast)));
        assert!(matches!(field("Non-Speaking"), Some(Field::Cast)));
        assert!(matches!(field("Silent Cast"), Some(Field::Cast)));
    }

    #[test]
    fn vehicles_and_set_dressing_fold_into_props() {
        assert!(matches!(field("Vehicles"), Some(Field::Props)));
        assert!(matches!(field("Set Dressing"), Some(Field::Props)));
        assert!(matches!(field("Weapons"), Some(Field::Props)));
    }

    #[test]
    fn casing_and_punctuation_normalized() {
        assert!(matches!(field("CAST MEMBERS"), Some(Field::Cast)));
        assert!(matches!(field("makeup/hair"), Some(Field::MakeupHair)));
        assert!(matches!(field("Makeup and Hair"), Some(Field::MakeupHair)));
    }

    #[test]
    fn unknown_category_returns_none() {
        assert!(field("Continuity").is_none());
        assert!(field("Random Tag").is_none());
        assert!(field("").is_none());
    }
}
