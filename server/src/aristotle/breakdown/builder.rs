//! Builder for [`SceneBreakdown`] used by the breakdown tiers.
//!
//! Each tier writes into the builder; the builder owns deduplication and
//! field-level merging so tiers don't need to know about each other.

use crate::aristotle::models::{ParsedScene, SceneBreakdown};
use std::collections::BTreeSet;

/// Accumulates breakdown fields across tiers. Each `Vec<String>` field is
/// backed by an order-preserving deduping set: the first time a value is
/// added it joins the field; subsequent adds are no-ops.
pub struct Builder {
    job_id: String,
    scene_number: i64,
    scene_heading: String,
    int_ext: Option<String>,
    location: String,
    time_of_day: String,
    page_length: String,
    cast: OrderedSet,
    extras_background: OrderedSet,
    speaking_cast: OrderedSet,
    props: OrderedSet,
    wardrobe: OrderedSet,
    makeup_hair: OrderedSet,
    special_effects: OrderedSet,
    stunts: OrderedSet,
    animals: OrderedSet,
    sound_effects: OrderedSet,
    music: OrderedSet,
    visual_effects: OrderedSet,
}

impl Builder {
    pub fn new(job_id: &str, scene: &ParsedScene) -> Self {
        Self {
            job_id: job_id.to_string(),
            scene_number: scene.scene_number as i64,
            scene_heading: scene.heading.clone(),
            int_ext: None,
            location: String::new(),
            time_of_day: String::new(),
            page_length: "1/8".into(),
            cast: OrderedSet::default(),
            extras_background: OrderedSet::default(),
            speaking_cast: OrderedSet::default(),
            props: OrderedSet::default(),
            wardrobe: OrderedSet::default(),
            makeup_hair: OrderedSet::default(),
            special_effects: OrderedSet::default(),
            stunts: OrderedSet::default(),
            animals: OrderedSet::default(),
            sound_effects: OrderedSet::default(),
            music: OrderedSet::default(),
            visual_effects: OrderedSet::default(),
        }
    }

    pub fn set_int_ext(&mut self, v: impl Into<String>) {
        let v = v.into();
        if !v.is_empty() {
            self.int_ext = Some(v);
        }
    }
    pub fn set_location(&mut self, v: impl Into<String>) {
        let v = v.into();
        if !v.is_empty() {
            self.location = v;
        }
    }
    pub fn set_time_of_day(&mut self, v: impl Into<String>) {
        let v = v.into();
        if !v.is_empty() {
            self.time_of_day = v;
        }
    }
    pub fn set_page_length(&mut self, v: impl Into<String>) {
        let v = v.into();
        if !v.is_empty() {
            self.page_length = v;
        }
    }

    pub fn add(&mut self, field: Field, value: impl Into<String>) {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        let v = trimmed.to_string();
        match field {
            Field::Cast => self.cast.insert(v),
            Field::ExtrasBackground => self.extras_background.insert(v),
            Field::SpeakingCast => self.speaking_cast.insert(v),
            Field::Props => self.props.insert(v),
            Field::Wardrobe => self.wardrobe.insert(v),
            Field::MakeupHair => self.makeup_hair.insert(v),
            Field::SpecialEffects => self.special_effects.insert(v),
            Field::Stunts => self.stunts.insert(v),
            Field::Animals => self.animals.insert(v),
            Field::SoundEffects => self.sound_effects.insert(v),
            Field::Music => self.music.insert(v),
            Field::VisualEffects => self.visual_effects.insert(v),
        }
    }

    pub fn into_breakdown(self) -> SceneBreakdown {
        SceneBreakdown {
            job_id: self.job_id,
            scene_number: self.scene_number,
            scene_heading: self.scene_heading,
            int_ext: self.int_ext,
            location: self.location,
            time_of_day: self.time_of_day,
            page_length: self.page_length,
            cast: self.cast.into_vec(),
            extras_background: self.extras_background.into_vec(),
            speaking_cast: self.speaking_cast.into_vec(),
            props: self.props.into_vec(),
            wardrobe: self.wardrobe.into_vec(),
            makeup_hair: self.makeup_hair.into_vec(),
            special_effects: self.special_effects.into_vec(),
            stunts: self.stunts.into_vec(),
            animals: self.animals.into_vec(),
            sound_effects: self.sound_effects.into_vec(),
            music: self.music.into_vec(),
            visual_effects: self.visual_effects.into_vec(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Field {
    Cast,
    ExtrasBackground,
    SpeakingCast,
    Props,
    Wardrobe,
    MakeupHair,
    SpecialEffects,
    Stunts,
    Animals,
    SoundEffects,
    Music,
    VisualEffects,
}

#[derive(Default)]
struct OrderedSet {
    order: Vec<String>,
    seen: BTreeSet<String>,
}

impl OrderedSet {
    fn insert(&mut self, v: String) {
        let key = v.to_ascii_lowercase();
        if self.seen.insert(key) {
            self.order.push(v);
        }
    }

    fn into_vec(self) -> Vec<String> {
        self.order
    }
}
