//! Tier 4 — LLM-driven classifier for ambiguous candidates.
//!
//! Tier 2 marks every all-caps phrase in action as a generic
//! `Introduction` chip without committing to whether it's a character,
//! prop, sound, or effect. This pass asks the configured LLM to
//! classify each introduction with a single letter (A–E). The LLM
//! NEVER generates JSON or structured output — it returns one token,
//! and Rust code slots the answer into the right breakdown field.
//!
//! Cost-aware:
//! - Runs only when `BREAKDOWN_POLICY=hybrid` is set.
//! - One small chat call per `Introduction` tag (~50 prompt tokens, 1
//!   output token).
//! - Pre-flight probe so we no-op cleanly when the endpoint is down.
//! - Original `Introduction` tag is left in place for provenance.
//!
//! This is the only place the LLM touches element tags — descriptions,
//! categories, and structure are still owned by deterministic code.

use crate::aristotle::breakdown::tags::add_tag_if_new;
use crate::aristotle::llm::LlmClient;
use crate::aristotle::models::{ParsedScene, Tag, TagSource};
use std::collections::HashSet;
use tracing::{info, warn};

/// Lower than tier-1 structural (1.0) but higher than tier-2 heuristic
/// (0.6) — the LLM's classification is informed but not authoritative.
const TIER4_CONFIDENCE: f32 = 0.8;

const SYSTEM_PROMPT: &str = "You are a screenplay breakdown assistant. You answer with EXACTLY one letter (A, B, C, D, or E). Do not write anything else.";

/// Classify every `Introduction` tag across the script using the LLM.
/// Adds a classified tag (`Cast`, `Props`, …) with `source = Llm`
/// alongside the existing `Introduction` chip. No-ops if the LLM
/// endpoint is unreachable.
pub async fn apply(scenes: &mut [ParsedScene], llm: &LlmClient) -> Result<(), String> {
    // Quick probe — if the embedding endpoint is alive, the chat
    // endpoint typically is too (they share Ollama for the default
    // setup). For Anthropic-backed chat with Ollama-backed embeds, a
    // failed classify call falls through to the `warn!` path below.
    if llm.embed("ping").await.is_err() {
        info!("LLM endpoint unreachable; skipping tier 4 classification");
        return Ok(());
    }

    let mut classified = 0usize;

    for scene in scenes.iter_mut() {
        for element in scene.elements.iter_mut() {
            let intros: Vec<String> = element
                .tags
                .iter()
                .filter(|t| t.category.eq_ignore_ascii_case("Introduction"))
                .filter_map(|t| t.value.clone())
                .collect();

            for value in intros {
                // Skip if we've already classified this value on this
                // element (e.g., a rebuild reran tier 4).
                if element.tags.iter().any(|t| {
                    t.source == TagSource::Llm
                        && t.value
                            .as_ref()
                            .map(|v| v.eq_ignore_ascii_case(&value))
                            .unwrap_or(false)
                }) {
                    continue;
                }

                match classify(llm, &element.text, &value).await {
                    Ok(category) => {
                        add_tag_if_new(
                            element,
                            &category,
                            Some(value.clone()),
                            TagSource::Llm,
                            TIER4_CONFIDENCE,
                        );
                        classified += 1;
                    }
                    Err(e) => {
                        warn!(introduction = %value, error = %e, "tier 4 classify failed");
                    }
                }
            }
        }
    }

    info!(classified, "tier 4 classification complete");
    Ok(())
}

/// Ask the LLM for a 1-2 sentence character description for each
/// `Introduction` that this pass already classified as `Cast`. Adds a
/// `Description` tag with `source = Llm` carrying the LLM's reply.
///
/// The tier-2 deterministic pass already produced a `Description` tag
/// from the trailing comma clause; this richer version coexists with
/// it (dedup on identical text). The LLM only returns prose, not JSON
/// — it never builds the breakdown structure.
pub async fn extract_descriptions(
    scenes: &mut [ParsedScene],
    llm: &LlmClient,
) -> Result<(), String> {
    if llm.embed("ping").await.is_err() {
        info!("LLM endpoint unreachable; skipping tier 4 descriptions");
        return Ok(());
    }

    let mut extracted = 0usize;

    for scene in scenes.iter_mut() {
        for element in scene.elements.iter_mut() {
            let names = cast_classified_introductions(&element.tags);
            for name in names {
                if has_llm_description(&element.tags, &name) {
                    continue;
                }
                match describe(llm, &element.text, &name).await {
                    Ok(desc) => {
                        add_tag_if_new(
                            element,
                            "Description",
                            Some(format!("{name}: {desc}")),
                            TagSource::Llm,
                            TIER4_CONFIDENCE,
                        );
                        extracted += 1;
                    }
                    Err(e) => {
                        warn!(name = %name, error = %e, "tier 4 description failed");
                    }
                }
            }
        }
    }

    info!(extracted, "tier 4 description extraction complete");
    Ok(())
}

/// Find introduction values that this same element has classified as
/// `Cast` via the LLM tier — i.e., the introduction the user/LLM has
/// confirmed is a character, not a prop or sound.
fn cast_classified_introductions(tags: &[Tag]) -> Vec<String> {
    let cast_llm: HashSet<String> = tags
        .iter()
        .filter(|t| t.category.eq_ignore_ascii_case("Cast") && t.source == TagSource::Llm)
        .filter_map(|t| t.value.as_ref().map(|v| v.to_ascii_lowercase()))
        .collect();

    let mut names: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for tag in tags {
        if !tag.category.eq_ignore_ascii_case("Introduction") {
            continue;
        }
        let Some(value) = &tag.value else { continue };
        let key = value.to_ascii_lowercase();
        if cast_llm.contains(&key) && seen.insert(key) {
            names.push(value.clone());
        }
    }
    names
}

/// True if this element already has an LLM-sourced Description tag for
/// the given character (idempotent guard for re-runs).
fn has_llm_description(tags: &[Tag], name: &str) -> bool {
    let prefix = format!("{}:", name.to_ascii_lowercase());
    tags.iter().any(|t| {
        t.source == TagSource::Llm
            && t.category.eq_ignore_ascii_case("Description")
            && t.value
                .as_ref()
                .map(|v| v.to_ascii_lowercase().starts_with(&prefix))
                .unwrap_or(false)
    })
}

const DESCRIPTION_SYSTEM_PROMPT: &str = "You are a screenplay breakdown assistant. Reply with only a 1-2 sentence character description. No quotes, no preamble, no character name.";

async fn describe(llm: &LlmClient, action: &str, name: &str) -> Result<String, String> {
    let user = format!(
        "Action paragraph:\n\"{action}\"\n\n\
        Describe the character {name} based on this paragraph. \
        Cover appearance, age, demeanor, or notable traits when present. \
        Reply with 1-2 sentences only. Do not start with the character's name.\n\
        Description:"
    );

    let response = llm
        .chat(DESCRIPTION_SYSTEM_PROMPT, &user)
        .await
        .map_err(|e| format!("LLM error: {e}"))?;

    let cleaned = response
        .trim()
        .trim_matches('"')
        .trim_start_matches(&[':', ' '][..])
        .trim()
        .to_string();

    if cleaned.len() < 5 {
        return Err(format!("response too short: {cleaned:?}"));
    }
    Ok(cleaned)
}

async fn classify(llm: &LlmClient, action: &str, intro: &str) -> Result<String, String> {
    let user = format!(
        "In this action paragraph: \"{action}\"\n\
        A new element is introduced as \"{intro}\". Classify it:\n\
        A = Cast (a named character or person)\n\
        B = Props (an object that can be carried, held, or used)\n\
        C = Sound Effects (an audio cue, noise, scream, gunshot)\n\
        D = Special Effects (a visual or practical effect: fire, explosion, smoke, blood)\n\
        E = Other (scenery, set piece, abstract concept)\n\
        Answer:"
    );

    let response = llm
        .chat(SYSTEM_PROMPT, &user)
        .await
        .map_err(|e| format!("LLM error: {e}"))?;

    let letter = response
        .chars()
        .find(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase());

    Ok(match letter {
        Some('A') => "Cast",
        Some('B') => "Props",
        Some('C') => "Sound Effects",
        Some('D') => "Special Effects",
        Some('E') => "Note",
        _ => return Err(format!("unrecognized response: {response:?}")),
    }
    .to_string())
}
