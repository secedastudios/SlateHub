//! Deterministic screenplay breakdown.
//!
//! Builds a [`SceneBreakdown`] from a [`ParsedScene`] without calling the
//! LLM. Three tiers run in order, each writing both into a [`Builder`]
//! (scene-level fields) and back onto the scene's elements (per-element
//! tags, so the UI can render breakdown markup inline):
//!
//! 1. **Tier 0 — native tags.** If the source format carried breakdown
//!    markup (FDX Tagger categories, Fountain `[[CAT: value]]` notes),
//!    those tags are already on the elements; we map their categories
//!    into the corresponding breakdown fields.
//! 2. **Tier 1 — structural.** Slug-line regex emits Location / Time /
//!    Int-Ext tags on the scene-heading element. Character cues emit a
//!    Cast tag. Page length is estimated from line counts.
//! 3. **Tier 2 — heuristic.** Action lines scanned against curated
//!    dictionaries (props, wardrobe, vehicles, animals) and trigger
//!    keywords (SFX, extras, mentioned characters). Each match becomes a
//!    low-confidence tag on the action element, dashed-bordered in the
//!    UI so users can confirm or reject quickly.
//!
//! Tier 4 (LLM yes/no classifier) is intentionally not wired in yet —
//! tiers 0-2 give a solid no-LLM baseline that the eventual LLM tier
//! can refine without owning JSON structure.

pub mod builder;
pub mod dedupe;
pub mod tags;
pub mod tier0_native;
pub mod tier1_structural;
pub mod tier2_heuristic;
pub mod tier4_classifier;

use crate::aristotle::models::{ElementKind, ParsedScene, SceneBreakdown};
use builder::Builder;
use std::collections::HashMap;

/// Cross-scene knowledge available to per-scene tiers.
///
/// Today this carries the canonical character set built from every
/// scene's `Character` cue elements — tier 2 uses it to detect
/// characters who are *mentioned* in action but don't speak in the
/// current scene. Keyed by lowercased name for case-insensitive lookup;
/// the value preserves the original casing for display.
#[derive(Debug, Default, Clone)]
pub struct Context {
    pub characters: HashMap<String, String>,
}

impl Context {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Walk every scene's Character cue elements, strip the standard
    /// `(V.O.)` / `(CONT'D)` modifiers, and collect the unique
    /// canonical names. Order-preserving HashMap entry: the first
    /// occurrence wins for display casing.
    pub fn from_scenes(scenes: &[ParsedScene]) -> Self {
        let mut characters: HashMap<String, String> = HashMap::new();
        for scene in scenes {
            for el in &scene.elements {
                if el.kind != ElementKind::Character {
                    continue;
                }
                let name = tier1_structural::strip_character_modifiers(&el.text);
                if name.is_empty() {
                    continue;
                }
                characters.entry(name.to_ascii_lowercase()).or_insert(name);
            }
        }
        Self { characters }
    }
}

/// How aggressively to invoke the LLM during breakdown.
///
/// - `DeterministicOnly`: tiers 0-2 only. Zero LLM calls per scene.
/// - `HybridLlm`: also runs tier 4 (a 1-letter-answer classifier) for
///   every `Introduction` tag the heuristic tier emits. The LLM never
///   generates JSON; it only picks A/B/C/D/E for the category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    DeterministicOnly,
    HybridLlm,
}

impl Policy {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "hybrid" | "llm" | "tier4" => Self::HybridLlm,
            _ => Self::DeterministicOnly,
        }
    }
}

/// Run the deterministic tiers, mutating `scene.elements` with per-element
/// tags and producing a [`SceneBreakdown`] ready to store. Callers persist
/// both the breakdown and the (now-annotated) elements.
///
/// `context` carries cross-scene knowledge (currently the character
/// list). Pass [`Context::from_scenes`] before the loop, then
/// [`Context::empty`] in code paths that intentionally skip
/// cross-scene heuristics.
pub fn run(
    job_id: &str,
    scene: &mut ParsedScene,
    context: &Context,
    _policy: Policy,
) -> SceneBreakdown {
    let mut builder = Builder::new(job_id, scene);
    tier0_native::apply(&mut builder, scene);
    tier1_structural::apply(&mut builder, scene);
    tier2_heuristic::apply(&mut builder, scene, context);
    builder.into_breakdown()
}

/// Rebuild a [`SceneBreakdown`] from a scene whose elements already carry
/// their current tags (typically loaded from the DB after user edits).
/// Runs tier 0 (read element tags) and tier 1 (structural rules) only —
/// tier 2's heuristic extractor is intentionally skipped so any tags the
/// user deleted aren't re-added from the same action text.
pub fn rebuild_from_edits(job_id: &str, scene: &mut ParsedScene) -> SceneBreakdown {
    let mut builder = Builder::new(job_id, scene);
    tier0_native::apply(&mut builder, scene);
    tier1_structural::apply(&mut builder, scene);
    builder.into_breakdown()
}
