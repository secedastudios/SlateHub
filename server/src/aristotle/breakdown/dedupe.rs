//! Cross-element synonym dedupe via embedding similarity.
//!
//! Tier 2 happily emits both `Photo` and `Photograph` as Props because
//! they match the dictionary independently. This pass collects every
//! `(category, value)` pair across all scenes, embeds the values, and
//! merges clusters above a cosine-similarity threshold within the same
//! category. The canonical form (longest variant in the cluster) wins;
//! all other variants are rewritten to it on both the element tags and
//! the in-flight breakdown rows.
//!
//! Gracefully no-ops when embeddings are unavailable (e.g., the
//! `chip_preview` example runs without Ollama).

use crate::aristotle::llm::LlmClient;
use crate::aristotle::models::{ParsedScene, SceneBreakdown};
use std::collections::{BTreeSet, HashMap};
use tracing::{info, warn};

/// Cosine-similarity threshold for merging values into a cluster.
///
/// With `nomic-embed-text` (768-d), semantically related but distinct
/// items like `binoculars` ↔ `telescope` cluster around 0.85-0.92, so
/// a 0.90 threshold falsely merged them and the longer string won as
/// canonical — that's how scenes that didn't have binoculars ended up
/// tagged with them.
///
/// 0.95 keeps `photo`/`photograph` and `phone`/`telephone` together
/// (both embed ≥ 0.95) while letting `binoculars`/`telescope` and
/// `gun`/`pistol` stay separate.
const MERGE_THRESHOLD: f32 = 0.95;

/// Categories whose values are scanned for synonyms. Cast/Silent share
/// a namespace because the same character may show up under both.
const DEDUPE_CATEGORIES: &[&str] = &[
    "props",
    "wardrobe",
    "vehicle",
    "animals",
    "special effects",
    "visual effects",
    "sound effects",
    "music",
    "stunts",
    "makeup",
    "makeup/hair",
    "extras",
];

/// Walk every element + breakdown, cluster equivalent values via
/// embedding similarity, and rewrite occurrences to a canonical form.
/// Returns `Ok(())` even if embeddings can't be reached — the caller
/// gets the original tags untouched.
pub async fn canonicalize(
    scenes: &mut [ParsedScene],
    breakdowns: &mut [SceneBreakdown],
    llm: &LlmClient,
) -> Result<(), String> {
    // Probe the embedding endpoint before doing any work. If Ollama is
    // unreachable (e.g., we're running the chip_preview example) we
    // simply skip the dedupe pass.
    if llm.embed("test").await.is_err() {
        info!("embedding endpoint unavailable; skipping cross-element synonym dedupe");
        return Ok(());
    }

    let mut by_category: HashMap<String, BTreeSet<String>> = HashMap::new();
    collect_values(scenes, &mut by_category);
    collect_breakdown_values(breakdowns, &mut by_category);

    if by_category.is_empty() {
        return Ok(());
    }

    // value (lowercased) → canonical form (original casing).
    let mut canonical: HashMap<String, String> = HashMap::new();

    for (category, values) in by_category {
        if !DEDUPE_CATEGORIES.contains(&category.as_str()) {
            continue;
        }
        if values.len() < 2 {
            continue;
        }

        let items: Vec<String> = values.into_iter().collect();
        let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(items.len());
        for v in &items {
            match llm.embed(v).await {
                Ok(e) => embeddings.push(e),
                Err(err) => {
                    warn!(error = %err, "embed failed during dedupe; aborting pass");
                    return Ok(());
                }
            }
        }

        let clusters = cluster(&items, &embeddings, MERGE_THRESHOLD);
        for cluster_indices in clusters {
            if cluster_indices.len() < 2 {
                continue;
            }
            // Canonical = longest variant; tiebreak by first occurrence.
            let canonical_form = cluster_indices
                .iter()
                .map(|&i| items[i].as_str())
                .max_by_key(|s| (s.len(), std::cmp::Reverse(*s)))
                .unwrap_or("")
                .to_string();

            // Log every merge so we can diagnose contamination
            // (e.g., "binoculars" eating other prop words).
            let merged_variants: Vec<&str> = cluster_indices
                .iter()
                .map(|&i| items[i].as_str())
                .filter(|s| *s != canonical_form)
                .collect();
            if !merged_variants.is_empty() {
                info!(
                    category = %category,
                    canonical = %canonical_form,
                    merged = ?merged_variants,
                    "dedupe merged variants"
                );
            }

            for &i in &cluster_indices {
                let variant = items[i].clone();
                canonical.insert(variant.to_ascii_lowercase(), canonical_form.clone());
            }
        }
    }

    if canonical.is_empty() {
        return Ok(());
    }

    apply_canonical_to_scenes(scenes, &canonical);
    apply_canonical_to_breakdowns(breakdowns, &canonical);

    Ok(())
}

fn collect_values(scenes: &[ParsedScene], out: &mut HashMap<String, BTreeSet<String>>) {
    for scene in scenes {
        for element in &scene.elements {
            for tag in &element.tags {
                let Some(value) = &tag.value else { continue };
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    continue;
                }
                out.entry(tag.category.to_ascii_lowercase())
                    .or_default()
                    .insert(trimmed.to_string());
            }
        }
    }
}

fn collect_breakdown_values(
    breakdowns: &[SceneBreakdown],
    out: &mut HashMap<String, BTreeSet<String>>,
) {
    for bd in breakdowns {
        push_list(out, "props", &bd.props);
        push_list(out, "wardrobe", &bd.wardrobe);
        push_list(out, "animals", &bd.animals);
        push_list(out, "special effects", &bd.special_effects);
        push_list(out, "visual effects", &bd.visual_effects);
        push_list(out, "sound effects", &bd.sound_effects);
        push_list(out, "music", &bd.music);
        push_list(out, "stunts", &bd.stunts);
        push_list(out, "makeup/hair", &bd.makeup_hair);
        push_list(out, "extras", &bd.extras_background);
    }
}

fn push_list(out: &mut HashMap<String, BTreeSet<String>>, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let set = out.entry(key.to_string()).or_default();
    for v in values {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            set.insert(trimmed.to_string());
        }
    }
}

/// Greedy single-link clustering: each item joins the first existing
/// cluster whose representative is above `threshold` cosine similarity.
/// Cluster representatives are the first item added; this is O(N*K)
/// where K is the eventual cluster count.
fn cluster(items: &[String], embeddings: &[Vec<f32>], threshold: f32) -> Vec<Vec<usize>> {
    let mut clusters: Vec<Vec<usize>> = Vec::new();

    for (i, _) in items.iter().enumerate() {
        let mut joined = false;
        for c in clusters.iter_mut() {
            let rep = c[0];
            if cosine(&embeddings[i], &embeddings[rep]) >= threshold {
                c.push(i);
                joined = true;
                break;
            }
        }
        if !joined {
            clusters.push(vec![i]);
        }
    }

    clusters
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = (na.sqrt()) * (nb.sqrt());
    if denom == 0.0 { 0.0 } else { dot / denom }
}

fn apply_canonical_to_scenes(scenes: &mut [ParsedScene], canonical: &HashMap<String, String>) {
    for scene in scenes.iter_mut() {
        for element in scene.elements.iter_mut() {
            for tag in element.tags.iter_mut() {
                if let Some(value) = &tag.value
                    && let Some(canon) = canonical.get(&value.to_ascii_lowercase())
                    && canon != value
                {
                    tag.value = Some(canon.clone());
                }
            }
            // De-dupe tags within the element after rewriting.
            dedupe_tags(&mut element.tags);
        }
    }
}

fn apply_canonical_to_breakdowns(
    breakdowns: &mut [SceneBreakdown],
    canonical: &HashMap<String, String>,
) {
    for bd in breakdowns.iter_mut() {
        rewrite_list(&mut bd.props, canonical);
        rewrite_list(&mut bd.wardrobe, canonical);
        rewrite_list(&mut bd.animals, canonical);
        rewrite_list(&mut bd.special_effects, canonical);
        rewrite_list(&mut bd.visual_effects, canonical);
        rewrite_list(&mut bd.sound_effects, canonical);
        rewrite_list(&mut bd.music, canonical);
        rewrite_list(&mut bd.stunts, canonical);
        rewrite_list(&mut bd.makeup_hair, canonical);
        rewrite_list(&mut bd.extras_background, canonical);
    }
}

fn rewrite_list(list: &mut Vec<String>, canonical: &HashMap<String, String>) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<String> = Vec::with_capacity(list.len());
    for v in list.drain(..) {
        let mapped = canonical.get(&v.to_ascii_lowercase()).cloned().unwrap_or(v);
        if seen.insert(mapped.to_ascii_lowercase()) {
            out.push(mapped);
        }
    }
    *list = out;
}

fn dedupe_tags(tags: &mut Vec<crate::aristotle::models::Tag>) {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut out: Vec<crate::aristotle::models::Tag> = Vec::with_capacity(tags.len());
    for tag in tags.drain(..) {
        let key = (
            tag.category.to_ascii_lowercase(),
            tag.value
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default(),
        );
        if seen.insert(key) {
            out.push(tag);
        }
    }
    *tags = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_vectors_is_one() {
        let v = vec![0.1, 0.2, 0.3];
        let sim = cosine(&v, &v);
        assert!((sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_orthogonal_vectors_is_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn cosine_opposite_vectors_is_negative_one() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine(&a, &b) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_handles_zero_vector() {
        let zero = vec![0.0, 0.0];
        let v = vec![1.0, 1.0];
        assert_eq!(cosine(&zero, &v), 0.0);
    }

    #[test]
    fn cluster_groups_similar_above_threshold() {
        let items = vec!["a".into(), "b".into(), "c".into()];
        // a and b are nearly identical; c is orthogonal.
        let embeddings = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.99, 0.01, 0.0],
            vec![0.0, 1.0, 0.0],
        ];
        let clusters = cluster(&items, &embeddings, 0.90);
        // a+b cluster, c alone.
        assert_eq!(clusters.len(), 2);
        let sizes: Vec<usize> = clusters.iter().map(|c| c.len()).collect();
        assert!(sizes.contains(&2));
        assert!(sizes.contains(&1));
    }

    #[test]
    fn cluster_keeps_distinct_items_separate() {
        let items = vec!["a".into(), "b".into(), "c".into()];
        let embeddings = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let clusters = cluster(&items, &embeddings, 0.90);
        assert_eq!(clusters.len(), 3);
    }

    #[test]
    fn rewrite_list_canonicalizes_and_dedupes() {
        let mut list = vec![
            "Photo".to_string(),
            "photograph".to_string(),
            "photo".to_string(),
        ];
        let mut canonical = HashMap::new();
        canonical.insert("photo".into(), "Photograph".into());
        canonical.insert("photograph".into(), "Photograph".into());

        rewrite_list(&mut list, &canonical);

        // All three collapse to one canonical entry.
        assert_eq!(list, vec!["Photograph".to_string()]);
    }

    #[test]
    fn cluster_does_not_merge_semantically_close_but_distinct() {
        // Regression test for the "binoculars eats telescope" bug.
        // With nomic-embed-text, semantically related items like
        // `binoculars` and `telescope` tend to embed around cosine
        // similarity 0.88-0.92. The 0.90 threshold falsely merged
        // them; 0.95 keeps them separate.
        //
        // Simulate two vectors at cosine ≈ 0.92 — close enough to
        // have merged at the old threshold, but separate at 0.95.
        let items = vec!["binoculars".into(), "telescope".into()];
        let embeddings = vec![vec![1.0, 0.0], vec![0.92, 0.392]];
        let cos = cosine(&embeddings[0], &embeddings[1]);
        assert!(
            (0.90..0.95).contains(&cos),
            "test vectors should simulate ~0.92 cosine sim, got {cos}"
        );

        let merged_at_old = cluster(&items, &embeddings, 0.90);
        assert_eq!(
            merged_at_old.len(),
            1,
            "old threshold (0.90) would have merged them"
        );

        let separate_at_new = cluster(&items, &embeddings, MERGE_THRESHOLD);
        assert_eq!(
            separate_at_new.len(),
            2,
            "new threshold ({MERGE_THRESHOLD}) keeps them separate"
        );
    }

    #[test]
    fn cluster_still_merges_near_identical_synonyms() {
        // Photo / photograph case — these embed ≥ 0.95 with nomic.
        // Simulate with two nearly-identical vectors and verify our
        // tightened threshold still catches them.
        let items = vec!["photo".into(), "photograph".into()];
        let embeddings = vec![vec![1.0, 0.0, 0.0], vec![0.98, 0.198, 0.0]];
        let cos = cosine(&embeddings[0], &embeddings[1]);
        assert!(
            cos > MERGE_THRESHOLD,
            "test sim should exceed threshold, got {cos}"
        );

        let clusters = cluster(&items, &embeddings, MERGE_THRESHOLD);
        assert_eq!(
            clusters.len(),
            1,
            "near-identical synonyms should still merge"
        );
    }

    #[test]
    fn rewrite_list_preserves_unrelated_entries() {
        let mut list = vec!["Photograph".to_string(), "Gun".to_string()];
        let canonical = HashMap::new();
        rewrite_list(&mut list, &canonical);
        assert_eq!(list, vec!["Photograph".to_string(), "Gun".to_string()]);
    }
}
