//! End-to-end integration tests for the deterministic breakdown
//! pipeline against checked-in fixtures.
//!
//! These tests run **everything except the DB and Ollama**:
//! parse → tier 0 → tier 1 → tier 2, plus cross-scene context. They
//! assert specific properties of the breakdown output so future
//! refactors can't silently change the cast list, drop a prop chip, or
//! reintroduce phantom scenes.
//!
//! Two fixtures:
//! - `examples/test_script.fountain` — short (4 scenes). Tests assert
//!   exact field-by-field values from `examples/test_script_expected.md`.
//! - `examples/test_script_full.fountain` — feature-length (145 scenes).
//!   Tests assert volume + invariants (no empty headings, every scene
//!   has cast, all 12 characters from the generator are detected).
//!
//! Run with `cargo test --test aristotle_breakdown_integration_test`.

use slatehub::aristotle::breakdown::{self, Context, Policy};
use slatehub::aristotle::models::{ElementKind, ParsedScene, SceneBreakdown};
use slatehub::aristotle::parser;

const SHORT_FIXTURE: &str = include_str!("fixtures/aristotle/test_script.fountain");
const FULL_FIXTURE: &str = include_str!("fixtures/aristotle/test_script_full.fountain");

/// Parse a fixture and run the full deterministic breakdown pipeline
/// (tiers 0-2 with cross-scene context). Returns the parsed scenes
/// (now with per-element tags attached) plus the per-scene
/// `SceneBreakdown` rows.
fn run_pipeline(filename: &str, source: &str) -> (Vec<ParsedScene>, Vec<SceneBreakdown>) {
    let mut parsed = parser::parse_screenplay(filename, source.as_bytes()).expect("parse fixture");
    let context = Context::from_scenes(&parsed.scenes);
    let breakdowns: Vec<SceneBreakdown> = parsed
        .scenes
        .iter_mut()
        .map(|scene| breakdown::run("test", scene, &context, Policy::DeterministicOnly))
        .collect();
    (parsed.scenes, breakdowns)
}

fn metadata_for(source: &str) -> slatehub::aristotle::models::ScriptMetadata {
    let parsed = parser::parse_screenplay("x.fountain", source.as_bytes()).unwrap();
    parsed.metadata
}

// ── Short fixture: exact-value assertions ──

#[test]
fn short_fixture_title_page_parses() {
    let meta = metadata_for(SHORT_FIXTURE);
    assert_eq!(meta.title.as_deref(), Some("THE LAST DEPOSIT"));
    assert_eq!(meta.writers, vec!["Test Author".to_string()]);
}

#[test]
fn short_fixture_has_four_scenes() {
    let (scenes, _) = run_pipeline("test.fountain", SHORT_FIXTURE);
    assert_eq!(
        scenes.len(),
        4,
        "expected exactly 4 scenes (no phantoms from FADE IN: / FADE OUT.)"
    );
}

#[test]
fn short_fixture_scene_1_apartment_night() {
    let (_, breakdowns) = run_pipeline("test.fountain", SHORT_FIXTURE);
    let s = &breakdowns[0];
    assert_eq!(s.int_ext.as_deref(), Some("INT"));
    assert_eq!(s.location, "DETECTIVE'S APARTMENT");
    assert_eq!(s.time_of_day, "NIGHT");
    assert!(s.cast.contains(&"MAYA".to_string()));
    // Tier 2 dictionary hits expected on the action lines.
    for prop in ["Laptop", "Revolver", "Briefcase", "Phone", "Keys"] {
        assert!(
            s.props.iter().any(|p| p == prop),
            "scene 1 should detect prop {prop:?}, got {:?}",
            s.props
        );
    }
    assert!(s.wardrobe.iter().any(|w| w == "Coat"));
}

#[test]
fn short_fixture_scene_2_warehouse_district_continuous() {
    let (_, breakdowns) = run_pipeline("test.fountain", SHORT_FIXTURE);
    let s = &breakdowns[1];
    assert_eq!(s.int_ext.as_deref(), Some("EXT"));
    assert_eq!(s.location, "WAREHOUSE DISTRICT");
    assert_eq!(s.time_of_day, "CONTINUOUS");
    assert!(s.cast.contains(&"MAYA".to_string()));
    assert!(
        s.extras_background.iter().any(|e| e == "Crowd"),
        "expected 'Crowd' in extras, got {:?}",
        s.extras_background
    );
    assert!(
        s.animals.iter().any(|a| a == "Dog"),
        "expected 'Dog' in animals"
    );
    assert!(
        s.special_effects.iter().any(|sx| sx == "Fog"),
        "expected 'Fog' SFX"
    );
}

#[test]
fn short_fixture_scene_3_warehouse_introduces_viktor_and_guards() {
    let (scenes, breakdowns) = run_pipeline("test.fountain", SHORT_FIXTURE);
    let s = &breakdowns[2];
    assert_eq!(s.location, "WAREHOUSE");
    assert_eq!(s.time_of_day, "MOMENTS LATER");

    // Both Maya (mentioned in action) and Viktor (speaking) are cast.
    assert!(s.cast.contains(&"MAYA".to_string()));
    assert!(s.cast.contains(&"VIKTOR".to_string()));
    assert!(s.wardrobe.iter().any(|w| w == "Suit"));
    assert!(s.props.iter().any(|p| p == "Cigar"));

    // SFX dictionary hits on the explosion paragraph.
    for sfx in ["Explosion", "Smoke", "Debris"] {
        assert!(
            s.special_effects.iter().any(|x| x == sfx),
            "expected '{sfx}' in SFX, got {:?}",
            s.special_effects
        );
    }

    // GUARDS appears all-caps in action — should generate an
    // Introduction + Description chip on the first action element.
    let scene_3 = &scenes[2];
    let intro_action = scene_3
        .elements
        .iter()
        .find(|e| {
            e.kind == ElementKind::Action && e.text.contains("VIKTOR") && e.text.contains("GUARDS")
        })
        .expect("expected the first action paragraph in scene 3");

    let intro_values: Vec<String> = intro_action
        .tags
        .iter()
        .filter(|t| t.category == "Introduction")
        .filter_map(|t| t.value.clone())
        .collect();
    assert!(
        intro_values.iter().any(|v| v == "Viktor"),
        "expected Introduction: Viktor on the action paragraph, got {intro_values:?}"
    );
    assert!(
        intro_values.iter().any(|v| v == "Guards"),
        "expected Introduction: Guards on the action paragraph, got {intro_values:?}"
    );

    let description_values: Vec<String> = intro_action
        .tags
        .iter()
        .filter(|t| t.category == "Description")
        .filter_map(|t| t.value.clone())
        .collect();
    assert!(
        description_values
            .iter()
            .any(|d| d.starts_with("Viktor:") && d.contains("scar")),
        "expected a Description tag for Viktor that includes the scar detail, got {description_values:?}"
    );
}

#[test]
fn short_fixture_scene_4_rooftop_maya_mentioned_in_action() {
    let (_, breakdowns) = run_pipeline("test.fountain", SHORT_FIXTURE);
    let s = &breakdowns[3];
    assert_eq!(s.location, "ROOFTOP");
    assert_eq!(s.time_of_day, "DAY");

    // Scene 4 has no Maya speaking cue but mentions her by name in
    // action. The mentioned-character scan should still tag her as
    // cast.
    assert!(
        s.cast.contains(&"MAYA".to_string()),
        "expected MAYA in cast via mentioned-character scan, got {:?}",
        s.cast
    );

    assert!(s.props.iter().any(|p| p == "Photograph" || p == "Photo"));
    assert!(s.props.iter().any(|p| p == "Lighter"));
    assert!(s.wardrobe.iter().any(|w| w == "Dress"));
    // Vehicle tag is its own chip category but folds into props in the
    // breakdown row's `props` field.
    assert!(s.props.iter().any(|p| p == "Helicopter"));
}

#[test]
fn short_fixture_fade_in_and_fade_out_are_not_character_cues() {
    // Regression test for the Fountain classifier bug where
    // FADE IN:, FADE OUT., and THE END were being treated as Character
    // cues, polluting the canonical character set.
    let (scenes, _) = run_pipeline("test.fountain", SHORT_FIXTURE);
    let context = Context::from_scenes(&scenes);
    let canonical: Vec<&String> = context.characters.values().collect();
    assert!(
        canonical.iter().all(|c| !c.contains("FADE")),
        "Found FADE in character set: {canonical:?}"
    );
    assert!(
        canonical.iter().all(|c| !c.contains("END")),
        "Found END in character set: {canonical:?}"
    );
}

// ── Full fixture: volume + invariants ──

#[test]
fn full_fixture_has_expected_volume() {
    let (scenes, _) = run_pipeline("full.fountain", FULL_FIXTURE);
    // Generator emits 145 scenes; the parser should preserve all of
    // them. Use a tight range so the assertion is meaningful but
    // tolerant of small generator tweaks.
    assert!(
        (140..=150).contains(&scenes.len()),
        "expected ~145 scenes, got {}",
        scenes.len()
    );
}

#[test]
fn full_fixture_detects_all_twelve_characters() {
    let (scenes, _) = run_pipeline("full.fountain", FULL_FIXTURE);
    let context = Context::from_scenes(&scenes);
    let names: Vec<&String> = context.characters.values().collect();

    // Every character defined in examples/generate_test_script.rs
    // should have a Character cue picked up somewhere in the script.
    for expected in [
        "MAYA",
        "VIKTOR",
        "RAY",
        "DR. CHEN",
        "JANE",
        "MARCUS",
        "LENA",
        "CARLOS",
        "SOPHIE",
        "FRANK",
        "ANA",
        "DETECTIVE PARK",
    ] {
        assert!(
            names.iter().any(|n| n.as_str() == expected),
            "missing canonical character {expected:?}, got {names:?}"
        );
    }
}

#[test]
fn full_fixture_no_phantom_scenes_with_empty_heading() {
    let (scenes, _) = run_pipeline("full.fountain", FULL_FIXTURE);
    for (i, scene) in scenes.iter().enumerate() {
        assert!(
            !scene.heading.is_empty(),
            "scene index {i} has an empty heading"
        );
        assert!(
            scene
                .elements
                .iter()
                .any(|e| e.kind == ElementKind::SceneHeading),
            "scene index {i} ({:?}) has no SceneHeading element",
            scene.heading
        );
    }
}

#[test]
fn full_fixture_every_scene_has_int_ext_location_time() {
    let (_, breakdowns) = run_pipeline("full.fountain", FULL_FIXTURE);
    for (i, bd) in breakdowns.iter().enumerate() {
        assert!(
            bd.int_ext.is_some(),
            "scene {} ({}) missing int_ext",
            i + 1,
            bd.scene_heading
        );
        assert!(!bd.location.is_empty(), "scene {} missing location", i + 1);
        assert!(
            !bd.time_of_day.is_empty(),
            "scene {} missing time_of_day",
            i + 1
        );
    }
}

#[test]
fn full_fixture_every_scene_has_speaking_cast() {
    let (_, breakdowns) = run_pipeline("full.fountain", FULL_FIXTURE);
    // The generator emits 2-4 dialogue beats per scene; every scene
    // should produce at least one speaking-cast entry.
    for (i, bd) in breakdowns.iter().enumerate() {
        assert!(
            !bd.speaking_cast.is_empty(),
            "scene {} ({}) has no speaking cast",
            i + 1,
            bd.scene_heading
        );
    }
}

#[test]
fn full_fixture_element_ids_are_unique_per_scene() {
    let (scenes, _) = run_pipeline("full.fountain", FULL_FIXTURE);
    for scene in &scenes {
        let mut seen = std::collections::HashSet::new();
        for el in &scene.elements {
            assert!(
                seen.insert(el.id.clone()),
                "duplicate element ID {:?} in scene {}",
                el.id,
                scene.scene_number
            );
            assert!(
                el.id.starts_with(&format!("s{}.e", scene.scene_number)),
                "element ID {:?} doesn't match s{}.e* convention",
                el.id,
                scene.scene_number
            );
        }
    }
}

#[test]
fn full_fixture_page_length_is_always_set() {
    let (_, breakdowns) = run_pipeline("full.fountain", FULL_FIXTURE);
    for bd in &breakdowns {
        assert!(
            !bd.page_length.is_empty(),
            "scene {} has empty page_length",
            bd.scene_number
        );
        // Page length string should match patterns like "1/8", "3 4/8",
        // or whole numbers.
        let pl = &bd.page_length;
        let plausible = pl
            .chars()
            .all(|c| c.is_ascii_digit() || c == '/' || c == ' ');
        assert!(
            plausible,
            "scene {} has malformed page_length {:?}",
            bd.scene_number, pl
        );
    }
}

#[test]
fn full_fixture_breakdown_rows_one_per_scene() {
    let (scenes, breakdowns) = run_pipeline("full.fountain", FULL_FIXTURE);
    assert_eq!(scenes.len(), breakdowns.len());
}

// ── Prop leakage / phantom-prop regression tests ──
//
// These guard against the user-reported bug where the dedupe pass
// merged a prop into "Binoculars" and rewrote unrelated scenes to
// show it. Tier 2 + tier 1 only — no dedupe — so any phantom prop
// here would point at the dictionary or classifier rather than the
// embedding merge.

/// True when `bd.props` contains the given word (case-insensitive).
fn has_prop(bd: &SceneBreakdown, prop: &str) -> bool {
    bd.props.iter().any(|p| p.eq_ignore_ascii_case(prop))
}

#[test]
fn fixtures_have_no_phantom_binoculars() {
    // Neither fixture mentions "binoculars". After the deterministic
    // pipeline, no scene's prop list may contain Binoculars. This is
    // the regression test for the user-reported bug.
    let (_, short_breakdowns) = run_pipeline("test.fountain", SHORT_FIXTURE);
    let (_, full_breakdowns) = run_pipeline("full.fountain", FULL_FIXTURE);

    for (i, bd) in short_breakdowns.iter().enumerate() {
        assert!(
            !has_prop(bd, "binoculars"),
            "short fixture scene {} has phantom Binoculars in props: {:?}",
            i + 1,
            bd.props
        );
    }
    for (i, bd) in full_breakdowns.iter().enumerate() {
        assert!(
            !has_prop(bd, "binoculars"),
            "full fixture scene {} has phantom Binoculars in props: {:?}",
            i + 1,
            bd.props
        );
    }
}

#[test]
fn props_only_appear_in_scenes_whose_text_mentions_them() {
    // For every prop the pipeline detected in any scene, the scene's
    // own action text must contain that word (case-insensitive whole-
    // word match). If this fails, props are leaking across scenes.
    let (scenes, breakdowns) = run_pipeline("full.fountain", FULL_FIXTURE);

    for (idx, bd) in breakdowns.iter().enumerate() {
        let scene = &scenes[idx];
        let scene_text: String = scene
            .elements
            .iter()
            .filter(|e| e.kind == slatehub::aristotle::models::ElementKind::Action)
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();

        for prop in &bd.props {
            let needle = prop.to_ascii_lowercase();
            assert!(
                contains_whole_word(&scene_text, &needle),
                "scene {} props contains {prop:?} but action text doesn't mention it",
                idx + 1
            );
        }
    }
}

fn contains_whole_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let n = needle.as_bytes();
    let nlen = n.len();
    if nlen == 0 || nlen > bytes.len() {
        return false;
    }
    let mut i = 0;
    while i + nlen <= bytes.len() {
        if &bytes[i..i + nlen] == n {
            let before = i == 0 || !is_word_byte(bytes[i - 1]);
            let after = i + nlen == bytes.len() || !is_word_byte(bytes[i + nlen]);
            if before && after {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[test]
fn short_fixture_props_match_action_text_exactly() {
    // For each scene in the short fixture, assert the exact set of
    // dictionary-detectable props. Anything beyond this list would be
    // a leak or a false positive from a new dictionary entry.
    let (_, breakdowns) = run_pipeline("test.fountain", SHORT_FIXTURE);

    // Scene 1: DETECTIVE'S APARTMENT.
    // Action mentions: bourbon (not in dict), laptop, revolver,
    // briefcase, phone, coat, keys.
    let s1 = &breakdowns[0];
    for expected in ["Laptop", "Revolver", "Briefcase", "Phone", "Keys"] {
        assert!(
            has_prop(s1, expected),
            "scene 1 missing expected prop {expected}, got {:?}",
            s1.props
        );
    }

    // Scene 2: WAREHOUSE DISTRICT.
    // Action mentions: sedan (Vehicle → Props), pistol.
    let s2 = &breakdowns[1];
    assert!(has_prop(s2, "Pistol"), "scene 2 missing Pistol");
    assert!(has_prop(s2, "Sedan"), "scene 2 missing Sedan");

    // Scene 3: WAREHOUSE.
    // Action mentions: cigar, pistol, crate (not in dict).
    let s3 = &breakdowns[2];
    assert!(has_prop(s3, "Cigar"), "scene 3 missing Cigar");
    assert!(has_prop(s3, "Pistol"), "scene 3 missing Pistol");

    // Scene 4: ROOFTOP.
    // Action mentions: photograph, lighter, helicopter (Vehicle).
    let s4 = &breakdowns[3];
    assert!(has_prop(s4, "Photograph") || has_prop(s4, "Photo"));
    assert!(has_prop(s4, "Lighter"));
    assert!(has_prop(s4, "Helicopter"));

    // Cross-scene leakage check: scene 1 props should NOT appear in
    // scene 4 (different action paragraphs entirely).
    assert!(
        !has_prop(s4, "Revolver"),
        "scene 4 has Revolver from scene 1"
    );
    assert!(!has_prop(s4, "Laptop"), "scene 4 has Laptop from scene 1");
    assert!(!has_prop(s4, "Cigar"), "scene 4 has Cigar from scene 3");
    assert!(
        !has_prop(s1, "Helicopter"),
        "scene 1 has Helicopter from scene 4"
    );
}

#[test]
fn full_fixture_no_unexpected_props_from_outside_action() {
    // Stronger version of the leakage test for the full fixture:
    // every prop in every scene's breakdown must trace back to that
    // scene's action text.
    let (scenes, breakdowns) = run_pipeline("full.fountain", FULL_FIXTURE);

    for (i, bd) in breakdowns.iter().enumerate() {
        let scene = &scenes[i];
        // Concatenate ALL element text for the scene (action +
        // dialogue + heading) — props rule should only hit action,
        // but if a word happens to appear in dialogue and the
        // breakdown shows it, that's a (minor) heuristic miss, not a
        // contamination. We allow it.
        let scene_text: String = scene
            .elements
            .iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();

        for prop in &bd.props {
            let needle = prop.to_ascii_lowercase();
            assert!(
                contains_whole_word(&scene_text, &needle),
                "scene {} has prop {prop:?} but no element text contains the word — \
                 likely cross-scene contamination",
                i + 1
            );
        }
    }
}

// ── PDF round-trip ──
//
// Render the Fountain fixture into a screenplay-formatted PDF with
// `printpdf` (dev-dep only), then run the same parser + breakdown
// pipeline against the PDF bytes. This is the *only* path that
// exercises `pdf_extract` end-to-end in our test suite — the rest of
// the integration tests work on Fountain or PDF-style text constants.

/// Render the fountain bytes into a screenplay-formatted PDF with
/// standard column positions (action at 1.5", dialogue at 2.5", parens
/// at 3.1", character at 3.7", transitions right-aligned). Uses the
/// built-in Courier face so no font asset is required.
fn fountain_to_pdf_bytes(filename: &str, fountain: &str) -> Vec<u8> {
    use printpdf::*;
    use std::io::BufWriter;

    let parsed = parser::parse_screenplay(filename, fountain.as_bytes()).expect("fountain parses");

    let (doc, page1, layer1) = PdfDocument::new(
        parsed.metadata.title.as_deref().unwrap_or("Test Script"),
        Mm(216.0),
        Mm(279.4),
        "Layer 1",
    );
    let font = doc
        .add_builtin_font(BuiltinFont::Courier)
        .expect("add Courier font");

    // Title page on the first physical page. Centered title and a
    // "Written by" / author block — enough for `extract_metadata` to
    // pick it up after the PDF round-trip.
    let title_layer = doc.get_page(page1).get_layer(layer1);
    if let Some(title) = parsed.metadata.title.as_deref() {
        title_layer.use_text(title, 12.0, Mm(90.0), Mm(180.0), &font);
    }
    if !parsed.metadata.writers.is_empty() {
        title_layer.use_text("Written by", 12.0, Mm(95.0), Mm(160.0), &font);
        title_layer.use_text(
            parsed.metadata.writers.join(", "),
            12.0,
            Mm(95.0),
            Mm(150.0),
            &font,
        );
    }

    // Body pages.
    let (mut current_page, mut current_layer) = doc.add_page(Mm(216.0), Mm(279.4), "Layer 1");
    let line_height = 4.94_f32; // 12pt Courier
    let top_y = 254.0_f32;
    let bottom_margin = 25.4_f32;
    let mut y = top_y;
    let mut prev_kind: Option<ElementKind> = None;

    for scene in &parsed.scenes {
        for element in &scene.elements {
            // Blank line before each element EXCEPT inside a dialogue
            // block (Character → Parenthetical/Dialogue, Parenthetical
            // → Dialogue, multi-line Dialogue). Standard screenplay
            // layout stacks dialogue elements tight.
            if needs_blank_before(prev_kind.as_ref(), &element.kind) {
                y -= line_height;
            }

            let (x, max_chars) = position_for_kind(&element.kind);
            for line in wrap_text(&element.text, max_chars) {
                if y < bottom_margin {
                    let (p, l) = doc.add_page(Mm(216.0), Mm(279.4), "Layer 1");
                    current_page = p;
                    current_layer = l;
                    y = top_y;
                }
                doc.get_page(current_page)
                    .get_layer(current_layer)
                    .use_text(line, 12.0, Mm(x), Mm(y), &font);
                y -= line_height;
            }
            prev_kind = Some(element.kind.clone());
        }
    }

    let mut buf = BufWriter::new(Vec::<u8>::new());
    doc.save(&mut buf).expect("PDF serializes");
    buf.into_inner().expect("PDF bytes")
}

/// Standard screenplay vertical spacing: blank line between blocks,
/// no blank within a Character → Parenthetical/Dialogue block.
fn needs_blank_before(prev: Option<&ElementKind>, current: &ElementKind) -> bool {
    let Some(prev) = prev else {
        return false;
    };
    use ElementKind::*;
    !matches!(
        (prev, current),
        (Character, Parenthetical)
            | (Character, Dialogue)
            | (Parenthetical, Dialogue)
            | (Parenthetical, Parenthetical)
            | (Dialogue, Parenthetical)
            | (Dialogue, Dialogue)
    )
}

fn position_for_kind(kind: &ElementKind) -> (f32, usize) {
    match kind {
        ElementKind::SceneHeading | ElementKind::Action => (38.0, 60),
        ElementKind::Character => (95.0, 35),
        ElementKind::Parenthetical => (78.0, 25),
        ElementKind::Dialogue => (63.0, 35),
        ElementKind::Transition => (165.0, 15),
        _ => (38.0, 60),
    }
}

/// Wrap a string at word boundaries. PDFs preserve whatever line
/// breaks we emit, so long action paragraphs need wrapping or
/// `pdf_extract` produces a single endless line.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.len() + 1 + word.len() > max_chars {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Dump the extracted text to stderr so we can see what
/// `pdf_extract` is actually producing for our generated PDF. Run
/// with `cargo test --test breakdown_integration debug_pdf -- --nocapture`.
#[test]
#[ignore = "diagnostic — enable with --ignored to inspect pdf_extract output"]
fn debug_pdf_extracted_text() {
    let pdf = fountain_to_pdf_bytes("test.fountain", SHORT_FIXTURE);
    let text = pdf_extract::extract_text_from_mem(&pdf).expect("extract text");
    eprintln!("=== first 1000 chars ===");
    eprintln!("{}", &text.chars().take(1000).collect::<String>());
    eprintln!("=== first 40 lines (each shown as >line<) ===");
    for (i, line) in text.lines().enumerate().take(40) {
        eprintln!("{i:>3}: >{line}<");
    }
}

#[test]
fn pdf_round_trip_recovers_title_and_writer() {
    let pdf_bytes = fountain_to_pdf_bytes("test.fountain", SHORT_FIXTURE);
    let parsed = parser::parse_screenplay("test.pdf", &pdf_bytes).expect("parse PDF");

    assert_eq!(
        parsed.metadata.title.as_deref(),
        Some("THE LAST DEPOSIT"),
        "title should survive the Fountain → PDF → pdf_extract round-trip"
    );
    assert!(
        parsed.metadata.writers.iter().any(|w| w == "Test Author"),
        "writer should survive the round-trip, got {:?}",
        parsed.metadata.writers
    );
}

#[test]
fn pdf_round_trip_finds_all_four_scenes() {
    let pdf_bytes = fountain_to_pdf_bytes("test.fountain", SHORT_FIXTURE);
    let parsed = parser::parse_screenplay("test.pdf", &pdf_bytes).expect("parse PDF");

    assert_eq!(
        parsed.scenes.len(),
        4,
        "expected 4 scenes after PDF round-trip, got {}",
        parsed.scenes.len()
    );

    let locations: Vec<&str> = parsed.scenes.iter().map(|s| s.heading.as_str()).collect();
    assert!(
        locations
            .iter()
            .any(|h| h.contains("DETECTIVE'S APARTMENT"))
    );
    assert!(locations.iter().any(|h| h.contains("WAREHOUSE DISTRICT")));
    assert!(locations.iter().any(|h| h.contains("WAREHOUSE")));
    assert!(locations.iter().any(|h| h.contains("ROOFTOP")));
}

#[test]
fn pdf_round_trip_detects_character_cues() {
    let pdf_bytes = fountain_to_pdf_bytes("test.fountain", SHORT_FIXTURE);
    let mut parsed = parser::parse_screenplay("test.pdf", &pdf_bytes).expect("parse PDF");

    // After indent-aware classification, MAYA and VIKTOR should be
    // recognized as character cues even though the PDF stripped the
    // original Fountain syntax.
    let context = Context::from_scenes(&parsed.scenes);
    let names: Vec<&String> = context.characters.values().collect();
    assert!(
        names.iter().any(|n| n.as_str() == "MAYA"),
        "expected MAYA in canonical characters after PDF round-trip, got {names:?}"
    );
    assert!(
        names.iter().any(|n| n.as_str() == "VIKTOR"),
        "expected VIKTOR in canonical characters after PDF round-trip, got {names:?}"
    );

    // Run the breakdown to make sure cast lands in the breakdown row,
    // not just on the Character cue elements.
    let breakdowns: Vec<SceneBreakdown> = parsed
        .scenes
        .iter_mut()
        .map(|s| breakdown::run("pdf-test", s, &context, Policy::DeterministicOnly))
        .collect();
    let total_cast: usize = breakdowns.iter().map(|b| b.cast.len()).sum();
    assert!(
        total_cast > 0,
        "no cast detected anywhere in the PDF round-trip"
    );
}

#[test]
fn pdf_round_trip_keeps_int_ext_and_time() {
    let pdf_bytes = fountain_to_pdf_bytes("test.fountain", SHORT_FIXTURE);
    let mut parsed = parser::parse_screenplay("test.pdf", &pdf_bytes).expect("parse PDF");
    let context = Context::from_scenes(&parsed.scenes);

    let breakdowns: Vec<SceneBreakdown> = parsed
        .scenes
        .iter_mut()
        .map(|s| breakdown::run("pdf-test", s, &context, Policy::DeterministicOnly))
        .collect();

    for (i, bd) in breakdowns.iter().enumerate() {
        assert!(
            bd.int_ext.is_some(),
            "scene {} missing int_ext after PDF round-trip",
            i + 1
        );
        assert!(!bd.location.is_empty(), "scene {} missing location", i + 1);
        assert!(
            !bd.time_of_day.is_empty(),
            "scene {} missing time_of_day",
            i + 1
        );
    }
}
