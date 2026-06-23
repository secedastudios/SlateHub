//! Tier 1 — structural extraction from already-typed screenplay elements.
//!
//! Pure rules, no dictionaries, no ML. What we extract:
//!
//! - **`int_ext`, `location`, `time_of_day`** from the scene heading slug
//!   line. Format: `INT./EXT. LOCATION - TIME OF DAY`. We use a tolerant
//!   regex that handles `INT/EXT.` and `I/E.` variants and accepts any
//!   dash character (ASCII `-`, en dash, em dash).
//! - **`speaking_cast`** from `ElementKind::Character` cues — these are
//!   100% reliable when the source format carried paragraph types.
//! - **`page_length`** approximated from total element text length using
//!   the industry rule of ~55 lines per page, rounded to eighths.

use crate::aristotle::breakdown::builder::{Builder, Field};
use crate::aristotle::breakdown::tags::add_tag_if_new;
use crate::aristotle::models::{ElementKind, ParsedScene, TagSource};
use regex::Regex;
use std::sync::OnceLock;

pub fn apply(builder: &mut Builder, scene: &mut ParsedScene) {
    let heading_info = parse_heading(builder, &scene.heading);
    annotate_scene_heading(scene, &heading_info);
    collect_speaking_cast(builder, scene);
    estimate_page_length(builder, scene);
}

/// Structural breakdown of the slug line, captured for both the
/// builder and the per-element tag write-back.
struct HeadingInfo {
    int_ext: Option<String>,
    location: Option<String>,
    time_of_day: Option<String>,
}

fn parse_heading(builder: &mut Builder, heading: &str) -> HeadingInfo {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(INT\.?/EXT\.?|EXT\.?/INT\.?|I/E\.?|INT\.?|EXT\.?|EST\.?)\s*(.*)$")
            .unwrap()
    });

    let mut info = HeadingInfo {
        int_ext: None,
        location: None,
        time_of_day: None,
    };

    let Some(caps) = re.captures(heading) else {
        return info;
    };

    let int_ext = normalize_int_ext(&caps[1]);
    builder.set_int_ext(int_ext.clone());
    info.int_ext = Some(int_ext);

    let rest = caps[2].trim();
    let (location, time) = split_location_time(rest);
    if !location.is_empty() {
        builder.set_location(location.clone());
        info.location = Some(location);
    }
    if !time.is_empty() {
        builder.set_time_of_day(time.clone());
        info.time_of_day = Some(time);
    }
    info
}

/// Write the parsed slug-line components back onto the scene-heading
/// element as `Structural` tags. The UI renders these as chips.
fn annotate_scene_heading(scene: &mut ParsedScene, info: &HeadingInfo) {
    let Some(heading_el) = scene
        .elements
        .iter_mut()
        .find(|e| e.kind == ElementKind::SceneHeading)
    else {
        return;
    };

    if let Some(v) = &info.int_ext {
        add_tag_if_new(
            heading_el,
            "Int/Ext",
            Some(v.clone()),
            TagSource::Structural,
            1.0,
        );
    }
    if let Some(v) = &info.location {
        add_tag_if_new(
            heading_el,
            "Location",
            Some(v.clone()),
            TagSource::Structural,
            1.0,
        );
    }
    if let Some(v) = &info.time_of_day {
        add_tag_if_new(
            heading_el,
            "Time of Day",
            Some(v.clone()),
            TagSource::Structural,
            1.0,
        );
    }
}

fn normalize_int_ext(raw: &str) -> String {
    let u = raw.to_ascii_uppercase();
    match u.replace('.', "").trim() {
        "INT" => "INT".into(),
        "EXT" => "EXT".into(),
        "INT/EXT" | "EXT/INT" | "I/E" => "INT/EXT".into(),
        "EST" => "EST".into(),
        other => other.to_string(),
    }
}

/// Split `"LOBBY - NIGHT"` into `("LOBBY", "NIGHT")`. Falls back to
/// returning the whole thing as the location if no dash is present.
/// Accepts ASCII hyphen, en dash, em dash, or " — ".
fn split_location_time(rest: &str) -> (String, String) {
    let separators = [" - ", " – ", " — ", " -- "];
    for sep in &separators {
        if let Some((loc, time)) = rest.rsplit_once(sep) {
            return (loc.trim().to_string(), time.trim().to_string());
        }
    }
    // Single-char dash fallback.
    if let Some(idx) = rest.rfind(['-', '–', '—']) {
        let (loc, rest_after) = rest.split_at(idx);
        let time = rest_after.trim_start_matches(['-', '–', '—', ' ']);
        return (loc.trim().to_string(), time.trim().to_string());
    }
    (rest.trim().to_string(), String::new())
}

fn collect_speaking_cast(builder: &mut Builder, scene: &mut ParsedScene) {
    for element in scene.elements.iter_mut() {
        if element.kind != ElementKind::Character {
            continue;
        }
        let name = strip_character_modifiers(&element.text);
        if name.is_empty() {
            continue;
        }
        builder.add(Field::SpeakingCast, name.clone());
        builder.add(Field::Cast, name.clone());
        add_tag_if_new(element, "Cast", Some(name), TagSource::Structural, 1.0);
    }
}

/// Strip `(V.O.)`, `(CONT'D)`, `(O.S.)`, etc. from a character cue and
/// normalize whitespace. Returns the bare character name.
pub(crate) fn strip_character_modifiers(cue: &str) -> String {
    let stripped = cue
        .split('(')
        .next()
        .unwrap_or(cue)
        .trim()
        .trim_end_matches('*');
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Approximate page length from total word count. Industry rule of
/// thumb: a screenplay page holds ~250 words of mixed action and
/// dialogue. We sum every element's whitespace-separated word count
/// (action and dialogue contribute equally), divide, and round to the
/// nearest eighth, clamped to `1/8` minimum.
///
/// Counting words is more robust than counting source `\n`s because
/// FDX/Fountain paragraphs are typically one source line each but wrap
/// across several visual lines when rendered.
fn estimate_page_length(builder: &mut Builder, scene: &mut ParsedScene) {
    if let Some(hint) = &scene.page_hint {
        builder.set_page_length(hint.clone());
        return;
    }

    let total_words: usize = scene
        .elements
        .iter()
        .filter(|e| e.kind != ElementKind::SceneHeading)
        .map(|e| e.text.split_whitespace().count())
        .sum();

    let pages = (total_words as f32) / 250.0;
    let eighths = (pages * 8.0).round().max(1.0) as i32;
    let whole = eighths / 8;
    let remainder = eighths % 8;

    let label = match (whole, remainder) {
        (0, r) => format!("{r}/8"),
        (w, 0) => w.to_string(),
        (w, r) => format!("{w} {r}/8"),
    };
    builder.set_page_length(label);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aristotle::models::ScreenplayElement;

    #[test]
    fn normalize_int_ext_canonicalizes_common_prefixes() {
        assert_eq!(normalize_int_ext("INT."), "INT");
        assert_eq!(normalize_int_ext("ext"), "EXT");
        assert_eq!(normalize_int_ext("INT./EXT."), "INT/EXT");
        assert_eq!(normalize_int_ext("I/E."), "INT/EXT");
        assert_eq!(normalize_int_ext("EST."), "EST");
    }

    #[test]
    fn split_location_time_handles_ascii_dash() {
        assert_eq!(
            split_location_time("LOBBY - NIGHT"),
            ("LOBBY".into(), "NIGHT".into())
        );
    }

    #[test]
    fn split_location_time_handles_unicode_dashes() {
        assert_eq!(
            split_location_time("LOBBY \u{2013} NIGHT"),
            ("LOBBY".into(), "NIGHT".into())
        );
        assert_eq!(
            split_location_time("LOBBY \u{2014} NIGHT"),
            ("LOBBY".into(), "NIGHT".into())
        );
    }

    #[test]
    fn split_location_time_picks_rightmost_separator() {
        // "WAREHOUSE - LOADING DOCK - NIGHT" should keep the full
        // location on the left of the final dash.
        assert_eq!(
            split_location_time("WAREHOUSE - LOADING DOCK - NIGHT"),
            ("WAREHOUSE - LOADING DOCK".into(), "NIGHT".into())
        );
    }

    #[test]
    fn split_location_time_falls_back_with_no_dash() {
        assert_eq!(
            split_location_time("ROOFTOP"),
            ("ROOFTOP".into(), String::new())
        );
    }

    #[test]
    fn strip_character_modifiers_removes_voice_over_and_cont_d() {
        assert_eq!(strip_character_modifiers("MAYA (V.O.)"), "MAYA");
        assert_eq!(strip_character_modifiers("MAYA (CONT'D)"), "MAYA");
        assert_eq!(strip_character_modifiers("DR. CHEN (O.S.)"), "DR. CHEN");
    }

    #[test]
    fn strip_character_modifiers_collapses_whitespace_and_trailing_star() {
        // Production scripts sometimes mark dialogue revisions with `*`.
        assert_eq!(strip_character_modifiers("MAYA*"), "MAYA");
        assert_eq!(strip_character_modifiers("  MAYA  "), "MAYA");
    }

    fn scene_with_action(words: usize) -> ParsedScene {
        let action_text = "lorem ".repeat(words);
        ParsedScene {
            scene_number: 1,
            heading: "INT. ROOM - DAY".into(),
            body: action_text.clone(),
            page_hint: None,
            elements: vec![
                ScreenplayElement {
                    id: "s1.e0".into(),
                    kind: ElementKind::SceneHeading,
                    text: "INT. ROOM - DAY".into(),
                    tags: vec![],
                },
                ScreenplayElement {
                    id: "s1.e1".into(),
                    kind: ElementKind::Action,
                    text: action_text.trim_end().into(),
                    tags: vec![],
                },
            ],
        }
    }

    fn page_length_for(words: usize) -> String {
        let mut scene = scene_with_action(words);
        let mut builder = Builder::new("test", &scene);
        estimate_page_length(&mut builder, &mut scene);
        builder.into_breakdown().page_length
    }

    #[test]
    fn estimate_page_length_floors_at_one_eighth() {
        // A short scene with only a handful of words still claims 1/8.
        assert_eq!(page_length_for(5), "1/8");
    }

    #[test]
    fn estimate_page_length_quarter_page() {
        // 250 words/page → 2/8 ≈ 62 words.
        assert_eq!(page_length_for(62), "2/8");
    }

    #[test]
    fn estimate_page_length_full_page() {
        assert_eq!(page_length_for(250), "1");
    }

    #[test]
    fn estimate_page_length_one_and_change() {
        // 250 words = 1 page, +half-page (~125 words) → 1 4/8.
        assert_eq!(page_length_for(375), "1 4/8");
    }

    #[test]
    fn estimate_page_length_uses_explicit_hint() {
        let mut scene = scene_with_action(10);
        scene.page_hint = Some("3 2/8".into());
        let mut builder = Builder::new("test", &scene);
        estimate_page_length(&mut builder, &mut scene);
        assert_eq!(builder.into_breakdown().page_length, "3 2/8");
    }
}
