//! Tier 2 — heuristic extraction from action lines.
//!
//! No model, no dictionary lookup beyond a curated word list, no LLM.
//! For each `Action` element we run a handful of compiled regex patterns
//! (one per breakdown category) and emit a low-confidence `Tag` for every
//! match. The CSS in the script view renders these with a dashed border
//! so users can spot automatic guesses vs. native markup.
//!
//! The dictionaries are deliberately compact — common nouns that appear
//! often enough in screenplays to be worth catching. False positives are
//! preferable to false negatives at this tier because the user can
//! quickly reject in the UI.

use crate::aristotle::breakdown::Context;
use crate::aristotle::breakdown::builder::{Builder, Field};
use crate::aristotle::breakdown::tags::add_tag_if_new;
use crate::aristotle::models::{ElementKind, ParsedScene, ScreenplayElement, TagSource};
use regex::Regex;
use std::sync::OnceLock;

const TIER2_CONFIDENCE: f32 = 0.6;
/// Confirmed-known character names get a slightly higher score than
/// a generic dictionary hit because the source (Character cue elsewhere
/// in the script) is itself ground truth.
const KNOWN_CHARACTER_CONFIDENCE: f32 = 0.7;
/// Capitalized introductions in action that don't match a known cast
/// member are guesses — make them visually distinct in the UI.
const NEW_INTRODUCTION_CONFIDENCE: f32 = 0.5;

pub fn apply(builder: &mut Builder, scene: &mut ParsedScene, context: &Context) {
    let dicts = dictionaries();

    for element in scene.elements.iter_mut() {
        if element.kind != ElementKind::Action {
            continue;
        }
        let text = element.text.clone();

        for (category, field, re) in dicts {
            for cap in re.find_iter(&text) {
                let matched = cap.as_str().to_string();
                let value = title_case(&matched);
                builder.add(*field, value.clone());
                add_tag_if_new(
                    element,
                    category,
                    Some(value),
                    TagSource::Pos,
                    TIER2_CONFIDENCE,
                );
            }
        }

        extract_extras(builder, element, &text);
        scan_mentioned_characters(builder, element, &text, context);
        scan_caps_introductions(builder, element, &text, context);
    }
}

/// Capitalize the first letter of each whitespace-separated word; lowercase
/// the rest. Keeps "FIRE" → "Fire" so chip labels look uniform.
fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Detect background extras / atmosphere with trigger phrases and numeric
/// crowd counts. `extract_extras` is broken out because the patterns
/// differ — triggers fire on phrase context, not on a dictionary hit.
fn extract_extras(
    builder: &mut Builder,
    element: &mut crate::aristotle::models::ScreenplayElement,
    text: &str,
) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(crowd|crowds|group|groups|onlookers|bystanders|guests|patrons|customers|students|pedestrians|passengers|audience|protesters|mob|fans|spectators|workers|soldiers|guards|policemen|officers|reporters|paparazzi)\b",
        )
        .unwrap()
    });

    for m in re.find_iter(text) {
        let value = title_case(m.as_str());
        builder.add(Field::ExtrasBackground, value.clone());
        add_tag_if_new(
            element,
            "Extras",
            Some(value),
            TagSource::Pos,
            TIER2_CONFIDENCE,
        );
    }
}

/// Walk the canonical character set and tag every Action mention of a
/// known character. The chip uses category `"Silent"` rather than
/// `"Cast"` so the UI can distinguish a character who is physically
/// present in this scene (mentioned in action) from one who actually
/// speaks (caught by tier 1 from a `Character` cue). Both still feed
/// the breakdown's `cast` field — `speaking_cast` is the subset with
/// dialogue cues.
///
/// Case-insensitive whole-word search — handles `Maya`, `MAYA`, and
/// `Maya's` (the apostrophe is a non-word char so `\b` after `Maya`
/// matches). Multi-word names like `DR. CHEN` are supported because
/// we do direct substring search with manual word boundary checks.
fn scan_mentioned_characters(
    builder: &mut Builder,
    element: &mut ScreenplayElement,
    text: &str,
    context: &Context,
) {
    if context.characters.is_empty() {
        return;
    }
    let lower = text.to_ascii_lowercase();

    for (needle, display) in &context.characters {
        if find_whole_word(&lower, needle).is_some() {
            builder.add(Field::Cast, display.clone());
            add_tag_if_new(
                element,
                "Silent",
                Some(display.clone()),
                TagSource::Pos,
                KNOWN_CHARACTER_CONFIDENCE,
            );
        }
    }
}

/// Detect ALL-CAPS phrases in action — screenplay convention for the
/// first introduction of a character, prop, sound, or significant
/// object. We don't try to classify (cast vs. prop vs. sfx); we just
/// emit a generic `Introduction` chip plus a `Description` chip
/// capturing the trailing comma clause. The user (or a future LLM
/// tier) recategorizes from there.
///
/// Runs for *every* all-caps phrase including ones that match a known
/// character — a character's first appearance in a screenplay is the
/// place writers put their description ("VIKTOR, 50s, sits behind a
/// desk in a tailored suit"), and we want that text captured.
fn scan_caps_introductions(
    _builder: &mut Builder,
    element: &mut ScreenplayElement,
    text: &str,
    _context: &Context,
) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // Three-or-more uppercase letters, optionally extended with
        // additional caps words (handles `JOHN SMITH`, `DR. CHEN`,
        // `MR. BROWN`). Internal apostrophes/hyphens/periods allowed.
        Regex::new(r"\b[A-Z][A-Z0-9'\-]{2,}(?:[ .]+[A-Z][A-Z0-9'\-]+)*\b").unwrap()
    });

    for m in re.find_iter(text) {
        let raw = m.as_str().trim_end_matches(['.', ' ']);
        let phrase = strip_direction_prefix(raw);
        if phrase.is_empty() || is_screenplay_direction(&phrase) {
            continue;
        }

        let display = title_case(&phrase);
        add_tag_if_new(
            element,
            "Introduction",
            Some(display.clone()),
            TagSource::Pos,
            NEW_INTRODUCTION_CONFIDENCE,
        );

        if let Some(desc) = extract_description(text, m.end()) {
            add_tag_if_new(
                element,
                "Description",
                Some(format!("{display}: {desc}")),
                TagSource::Pos,
                NEW_INTRODUCTION_CONFIDENCE,
            );
        }
    }
}

/// If `phrase` starts with one or more screenplay direction words
/// (`CUT TO`, `SUDDENLY`, `INT.`, `POV`, …), strip them and return
/// what remains. Picks the longest matching prefix so `CUT TO MAYA`
/// strips `CUT TO` (two words) rather than just `CUT`.
fn strip_direction_prefix(phrase: &str) -> String {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    let mut prefix_len = 0usize;
    for n in 1..=words.len() {
        let prefix = words[..n].join(" ");
        if is_screenplay_direction(&prefix) {
            prefix_len = n;
        }
    }
    words[prefix_len..].join(" ")
}

/// Filter out ALL-CAPS phrases that aren't introductions: transitions,
/// camera/scene directions, common screenplay slug words. Conservative
/// list — we'd rather emit a low-confidence tag and have the user
/// dismiss it than swallow a real character introduction.
fn is_screenplay_direction(s: &str) -> bool {
    const DIRECTIONS: &[&str] = &[
        "CONT",
        "CONT'D",
        "CONTINUED",
        "MORE",
        "OFF",
        "OFF SCREEN",
        "CUT TO",
        "CUT",
        "FADE IN",
        "FADE OUT",
        "FADE TO",
        "DISSOLVE",
        "DISSOLVE TO",
        "SMASH CUT",
        "MATCH CUT",
        "INTERCUT",
        "FLASHBACK",
        "END",
        "THE END",
        "TITLE",
        "SUPER",
        "SUPERIMPOSE",
        "CHYRON",
        "MONTAGE",
        "SERIES OF SHOTS",
        "BACK TO",
        "ANGLE ON",
        "CLOSE ON",
        "WIDE ON",
        "POV",
        "INSERT",
        "PRELAP",
        "V.O",
        "V.O.",
        "O.S",
        "O.S.",
        "O.C",
        "O.C.",
        "INT",
        "EXT",
        "INT.",
        "EXT.",
        "INT/EXT",
        "I/E",
        "DAY",
        "NIGHT",
        "MORNING",
        "AFTERNOON",
        "EVENING",
        "CONTINUOUS",
        "LATER",
        "MOMENTS LATER",
        "SUDDENLY",
        "BEAT",
    ];
    let upper = s.to_ascii_uppercase();
    let trimmed = upper.trim_matches('.').trim();
    DIRECTIONS.contains(&trimmed)
}

/// Capture the descriptive clause that immediately follows a
/// just-introduced ALL CAPS phrase. Stops at the next sentence boundary
/// (`.`, `!`, `?`) or 200 characters, and strips a leading `, ` if
/// present. Returns `None` if nothing meaningful follows.
fn extract_description(text: &str, start: usize) -> Option<String> {
    let rest = text.get(start..)?;
    let after = rest.trim_start_matches([',', ' ']);
    if after.is_empty() {
        return None;
    }
    let stop = after.find(['.', '!', '?']).unwrap_or(after.len());
    let end = stop.min(200);
    let desc = after[..end].trim();
    (!desc.is_empty()).then(|| desc.to_string())
}

/// Find `needle` as a whole-word match inside `haystack`. Both must
/// already be lowercase. ASCII word boundaries on both sides.
/// Apostrophes/dashes/periods bordering the match are not word chars,
/// so `Maya's`, `Maya-Lee`, and `Maya.` all match a needle of `maya`.
fn find_whole_word(haystack: &str, needle: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let n = needle.as_bytes();
    let nlen = n.len();
    if nlen == 0 || nlen > bytes.len() {
        return None;
    }
    let mut i = 0;
    while i + nlen <= bytes.len() {
        if &bytes[i..i + nlen] == n {
            let before_ok = i == 0 || !is_word_byte(bytes[i - 1]);
            let after_ok = i + nlen == bytes.len() || !is_word_byte(bytes[i + nlen]);
            if before_ok && after_ok {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// `(category_label, breakdown_field, compiled_regex)` triples used by
/// the main `apply` loop. Lazily built on first call and cached.
fn dictionaries() -> &'static [(&'static str, Field, &'static Regex)] {
    static DICTS: OnceLock<Vec<(&'static str, Field, &'static Regex)>> = OnceLock::new();
    DICTS.get_or_init(|| {
        vec![
            ("Props", Field::Props, compile(PROPS)),
            ("Wardrobe", Field::Wardrobe, compile(WARDROBE)),
            // Vehicles fold into Props in the SceneBreakdown schema, but
            // we keep the chip category distinct so the UI can color-code.
            ("Vehicle", Field::Props, compile(VEHICLES)),
            ("Animals", Field::Animals, compile(ANIMALS)),
            ("Special Effects", Field::SpecialEffects, compile(SFX_CUES)),
        ]
    })
}

fn compile(words: &[&str]) -> &'static Regex {
    let pattern = format!(r"(?i)\b({})\b", words.join("|"));
    let regex = Regex::new(&pattern).expect("dictionary regex compiles");
    // Leak so the inner reference is `'static`. The list is tiny and the
    // memory lives for the process lifetime anyway.
    Box::leak(Box::new(regex))
}

// ── Curated dictionaries ──
//
// Order doesn't matter — regex alternation is OR. Keep entries lowercase;
// the regex flag handles case-insensitivity, and `title_case` normalizes
// the displayed value.

const PROPS: &[&str] = &[
    // Weapons
    "gun",
    "pistol",
    "rifle",
    "shotgun",
    "revolver",
    "handgun",
    "weapon",
    "weapons",
    "knife",
    "sword",
    "dagger",
    "axe",
    "hammer",
    "club",
    "bat",
    "bow",
    "arrow",
    "grenade",
    // Personal
    "phone",
    "cellphone",
    "telephone",
    "smartphone",
    "mobile",
    "laptop",
    "computer",
    "tablet",
    "camera",
    "wallet",
    "keys",
    "key",
    "watch",
    "ring",
    "necklace",
    "badge",
    "glasses",
    "sunglasses",
    // Bags & containers
    "briefcase",
    "suitcase",
    "bag",
    "backpack",
    "purse",
    "satchel",
    "box",
    "package",
    "envelope",
    "folder",
    "file",
    "document",
    // Paper
    "book",
    "notebook",
    "journal",
    "diary",
    "magazine",
    "newspaper",
    "letter",
    "paper",
    "map",
    "photograph",
    "photo",
    "ticket",
    "receipt",
    "card",
    "id",
    // Tools
    "flashlight",
    "torch",
    "binoculars",
    "telescope",
    "rope",
    "ladder",
    // Smoking / drugs
    "cigarette",
    "cigar",
    "lighter",
    "matches",
    "syringe",
    "needle",
    "pill",
    "pills",
    // Furniture / drink ware (small props level)
    "glass",
    "bottle",
    "can",
    "cup",
    "mug",
    "plate",
    // Money
    "money",
    "cash",
    "coins",
];

const WARDROBE: &[&str] = &[
    "coat",
    "jacket",
    "blazer",
    "suit",
    "tuxedo",
    "vest",
    "sweater",
    "hoodie",
    "cardigan",
    "shirt",
    "t-shirt",
    "blouse",
    "polo",
    "dress",
    "gown",
    "skirt",
    "robe",
    "kimono",
    "pants",
    "jeans",
    "trousers",
    "slacks",
    "shorts",
    "leggings",
    "shoes",
    "boots",
    "sneakers",
    "heels",
    "sandals",
    "loafers",
    "hat",
    "cap",
    "helmet",
    "beanie",
    "fedora",
    "gloves",
    "mittens",
    "tie",
    "bowtie",
    "scarf",
    "bandana",
    "mask",
    "veil",
    "belt",
    "suspenders",
    "harness",
    "uniform",
    "scrubs",
    "costume",
    "armor",
    "apron",
];

const VEHICLES: &[&str] = &[
    "car",
    "truck",
    "van",
    "bus",
    "motorcycle",
    "scooter",
    "bicycle",
    "bike",
    "sedan",
    "suv",
    "jeep",
    "convertible",
    "coupe",
    "minivan",
    "rv",
    "taxi",
    "cab",
    "limo",
    "limousine",
    "helicopter",
    "plane",
    "jet",
    "airplane",
    "drone",
    "boat",
    "ship",
    "yacht",
    "submarine",
    "raft",
    "canoe",
    "kayak",
    "train",
    "subway",
    "tram",
];

const ANIMALS: &[&str] = &[
    "dog", "puppy", "cat", "kitten", "horse", "cow", "bull", "pig", "sheep", "goat", "chicken",
    "rooster", "hen", "duck", "goose", "turkey", "deer", "fox", "wolf", "bear", "rabbit", "mouse",
    "rat", "squirrel", "raccoon", "fish", "shark", "dolphin", "whale", "bird", "eagle", "hawk",
    "owl", "parrot", "crow", "raven", "pigeon", "seagull", "snake", "lizard", "frog", "turtle",
    "lion", "tiger", "elephant", "monkey", "gorilla",
];

// `smoking` and bare `fire` are too polysemous — "smoking a cigar"
// and "fire wildly" (= shoot) generate false SFX hits. We rely on
// `smoke` / `smolders` for atmospheric smoke and `flames` / `burning`
// / `ablaze` / `inferno` for actual fire.
const SFX_CUES: &[&str] = &[
    "explosion",
    "explodes",
    "blast",
    "fireball",
    "detonation",
    "detonates",
    "flames",
    "burning",
    "ablaze",
    "inferno",
    "smoke",
    "smolders",
    "blood",
    "bleeds",
    "bleeding",
    "gushing",
    "gunshot",
    "gunfire",
    "gunshots",
    "crash",
    "crashes",
    "collision",
    "wreckage",
    "splash",
    "drowns",
    "drowning",
    "fog",
    "mist",
    "haze",
    "lightning",
    "thunder",
    "storm",
    "earthquake",
    "tremor",
    "debris",
    "rubble",
    "shrapnel",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aristotle::models::ScreenplayElement;

    #[test]
    fn title_case_normalizes_caps_phrases() {
        assert_eq!(title_case("MAYA"), "Maya");
        assert_eq!(title_case("LOUD CRASH"), "Loud Crash");
        assert_eq!(title_case("Dr. Chen"), "Dr. Chen");
    }

    #[test]
    fn is_screenplay_direction_filters_camera_and_transition_slugs() {
        assert!(is_screenplay_direction("CUT TO"));
        assert!(is_screenplay_direction("FADE IN"));
        assert!(is_screenplay_direction("POV"));
        assert!(is_screenplay_direction("CONT'D"));
        assert!(is_screenplay_direction("INT"));
        assert!(is_screenplay_direction("SUDDENLY"));
        assert!(is_screenplay_direction("CONTINUOUS"));
    }

    #[test]
    fn is_screenplay_direction_lets_real_names_through() {
        assert!(!is_screenplay_direction("MAYA"));
        assert!(!is_screenplay_direction("DR. CHEN"));
        assert!(!is_screenplay_direction("GUARDS"));
        assert!(!is_screenplay_direction("VIKTOR"));
    }

    #[test]
    fn extract_description_captures_comma_clause() {
        let text = "VIKTOR, 50s, weathered, sits at the desk. Then he stands.";
        // After "VIKTOR" (index 6), description should be the clause up
        // to the first period.
        let desc = extract_description(text, 6).unwrap();
        assert_eq!(desc, "50s, weathered, sits at the desk");
    }

    #[test]
    fn extract_description_handles_no_punctuation() {
        let text = "MAYA enters the room";
        let desc = extract_description(text, 4).unwrap();
        assert_eq!(desc, "enters the room");
    }

    #[test]
    fn extract_description_returns_none_when_nothing_follows() {
        let text = "MAYA.";
        assert_eq!(extract_description(text, 5), None);
    }

    #[test]
    fn find_whole_word_matches_simple_name() {
        // Both inputs are lowercased before this function runs.
        assert_eq!(find_whole_word("maya enters the room", "maya"), Some(0));
        assert_eq!(find_whole_word("she sees maya leave", "maya"), Some(9));
    }

    #[test]
    fn find_whole_word_handles_possessive_apostrophe() {
        // "Maya's" — apostrophe is non-word so the boundary after maya
        // is valid.
        assert_eq!(find_whole_word("maya's coat", "maya"), Some(0));
        assert_eq!(find_whole_word("viktor's bodyguards", "viktor"), Some(0));
    }

    #[test]
    fn find_whole_word_rejects_substring_inside_another_word() {
        // "mayan" is a different word; we should NOT match maya in it.
        assert_eq!(find_whole_word("the mayan calendar", "maya"), None);
        assert_eq!(find_whole_word("guns blazing", "gun"), None);
    }

    #[test]
    fn find_whole_word_matches_at_string_end() {
        assert_eq!(find_whole_word("call maya", "maya"), Some(5));
    }

    #[test]
    fn scan_mentioned_characters_tags_known_names_in_action() {
        let mut element = ScreenplayElement {
            id: "s1.e1".into(),
            kind: ElementKind::Action,
            text: "Maya enters with Viktor's coat.".into(),
            tags: vec![],
        };
        let mut scene = ParsedScene {
            scene_number: 1,
            heading: String::new(),
            body: String::new(),
            page_hint: None,
            elements: vec![],
        };
        let mut builder = Builder::new("t", &scene);

        let mut ctx = Context::empty();
        ctx.characters.insert("maya".into(), "MAYA".into());
        ctx.characters.insert("viktor".into(), "VIKTOR".into());

        let text = element.text.clone();
        scan_mentioned_characters(&mut builder, &mut element, &text, &ctx);

        // Maya and Viktor both flagged as silent on this element.
        let categories: Vec<_> = element.tags.iter().map(|t| t.category.as_str()).collect();
        assert!(categories.iter().all(|c| *c == "Silent"));
        assert_eq!(element.tags.len(), 2);

        let values: Vec<_> = element
            .tags
            .iter()
            .filter_map(|t| t.value.clone())
            .collect();
        assert!(values.contains(&"MAYA".to_string()));
        assert!(values.contains(&"VIKTOR".to_string()));

        // Don't recurse into scene unused warning.
        let _ = &mut scene;
    }

    // ── Prop detection coverage ──
    //
    // These tests guard the prop dictionary against the bug where
    // scenes were tagged with props (e.g., "binoculars") that didn't
    // appear in their action text. Each test constructs a single
    // action element with controlled content and asserts the exact
    // set of props the tier-2 heuristic detects.

    fn breakdown_for_action(text: &str) -> crate::aristotle::models::SceneBreakdown {
        let mut scene = ParsedScene {
            scene_number: 1,
            heading: "INT. ROOM - DAY".into(),
            body: text.to_string(),
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
                    text: text.to_string(),
                    tags: vec![],
                },
            ],
        };
        let mut builder = Builder::new("t", &scene);
        let ctx = Context::empty();
        apply(&mut builder, &mut scene, &ctx);
        builder.into_breakdown()
    }

    fn detected_props(text: &str) -> Vec<String> {
        breakdown_for_action(text).props
    }

    #[test]
    fn props_single_match_detected() {
        assert_eq!(detected_props("She picks up the gun."), vec!["Gun"]);
        assert_eq!(detected_props("Her phone buzzes."), vec!["Phone"]);
    }

    #[test]
    fn props_multiple_matches_in_one_paragraph() {
        let props = detected_props(
            "She grabs the briefcase, slides a laptop in, and tucks a revolver inside.",
        );
        assert!(props.contains(&"Briefcase".to_string()), "got {props:?}");
        assert!(props.contains(&"Laptop".to_string()), "got {props:?}");
        assert!(props.contains(&"Revolver".to_string()), "got {props:?}");
    }

    #[test]
    fn props_empty_when_no_dictionary_hit() {
        // Plain action with no dictionary words.
        assert!(detected_props("She walks down the street and thinks about it.").is_empty());
        assert!(detected_props("The room is dim. He paces.").is_empty());
        assert!(detected_props("Wind howls through the trees.").is_empty());
    }

    #[test]
    fn props_word_boundary_rejects_substring_in_compound() {
        // "gunner" contains "gun" but the trailing 'n' is a word char,
        // so the \b boundary check rejects it.
        assert!(!detected_props("The gunner takes aim.").contains(&"Gun".to_string()));
        // "phonetic" contains "phone" — boundary should reject.
        assert!(
            !detected_props("Her phonetic accent is unmistakable.").contains(&"Phone".to_string())
        );
    }

    #[test]
    fn props_case_insensitive() {
        // ALL CAPS, mixed case, lower — all detected, all title-cased.
        assert_eq!(detected_props("She fires the GUN."), vec!["Gun"]);
        assert_eq!(detected_props("She fires the Gun."), vec!["Gun"]);
        assert_eq!(detected_props("She fires the gun."), vec!["Gun"]);
    }

    #[test]
    fn props_possessive_apostrophe_matches() {
        // "Maya's gun" — apostrophe is non-word so boundary matches.
        let props = detected_props("She finds Maya's gun on the floor.");
        assert!(props.contains(&"Gun".to_string()), "got {props:?}");
    }

    #[test]
    fn props_binoculars_explicit_match() {
        // The word IS in our dictionary — when it appears in action,
        // it should be tagged. This is the positive case.
        assert!(detected_props("He raises the binoculars.").contains(&"Binoculars".to_string()));
    }

    #[test]
    fn props_no_phantom_binoculars_when_absent() {
        // Regression test for the user-reported bug. When binoculars
        // are not in the action text, they must not appear in props
        // — neither via tier 2 detection nor by name confusion with
        // other words.
        let texts = [
            "She picks up the telescope and scans the rooftop.",
            "His glasses catch the light as he turns.",
            "The camera shutter clicks twice.",
            "He looks through the scope on the rifle.",
            "She holds the flashlight steady.",
            "An old microscope sits on the shelf.",
        ];
        for text in texts {
            let props = detected_props(text);
            assert!(
                !props.iter().any(|p| p.eq_ignore_ascii_case("binoculars")),
                "phantom Binoculars detected in {text:?} → props {props:?}"
            );
        }
    }

    #[test]
    fn props_no_leak_between_elements() {
        // Two action elements; each prop should only appear in the
        // scene's breakdown via the element that actually contains it.
        let mut scene = ParsedScene {
            scene_number: 1,
            heading: "INT. ROOM - DAY".into(),
            body: String::new(),
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
                    text: "She picks up the gun.".into(),
                    tags: vec![],
                },
                ScreenplayElement {
                    id: "s1.e2".into(),
                    kind: ElementKind::Action,
                    text: "He grabs the briefcase.".into(),
                    tags: vec![],
                },
            ],
        };
        let mut builder = Builder::new("t", &scene);
        let ctx = Context::empty();
        apply(&mut builder, &mut scene, &ctx);

        // Each action element should only carry the prop tag for its
        // own text.
        let action1 = &scene.elements[1];
        let action2 = &scene.elements[2];

        let a1_props: Vec<_> = action1
            .tags
            .iter()
            .filter(|t| t.category == "Props")
            .filter_map(|t| t.value.as_deref())
            .collect();
        let a2_props: Vec<_> = action2
            .tags
            .iter()
            .filter(|t| t.category == "Props")
            .filter_map(|t| t.value.as_deref())
            .collect();

        assert_eq!(a1_props, vec!["Gun"]);
        assert_eq!(a2_props, vec!["Briefcase"]);

        // Scene-level breakdown aggregates both.
        let bd = builder.into_breakdown();
        assert!(bd.props.contains(&"Gun".to_string()));
        assert!(bd.props.contains(&"Briefcase".to_string()));
    }

    #[test]
    fn props_categorized_separately_from_wardrobe_and_sfx() {
        // A single paragraph with one of each: prop, wardrobe, SFX.
        let bd = breakdown_for_action(
            "She grabs the briefcase, ties her scarf, and runs from the smoke.",
        );
        assert!(bd.props.contains(&"Briefcase".to_string()));
        assert!(bd.wardrobe.contains(&"Scarf".to_string()));
        assert!(bd.special_effects.contains(&"Smoke".to_string()));
        // Scarf should NOT show up in props, briefcase should NOT in
        // wardrobe, etc.
        assert!(!bd.props.contains(&"Scarf".to_string()));
        assert!(!bd.wardrobe.contains(&"Briefcase".to_string()));
    }

    #[test]
    fn props_vehicle_words_categorized_as_vehicle_chip_but_props_field() {
        // Vehicle dictionary feeds the `Props` field but the per-element
        // chip uses the `Vehicle` category for color-coding.
        let bd = breakdown_for_action("A black sedan screeches around the corner.");
        assert!(bd.props.contains(&"Sedan".to_string()));
    }

    #[test]
    fn props_dictionary_word_must_be_standalone_not_in_url_or_email() {
        // A URL like "phonecompany.com" shouldn't trigger "phone".
        let props = detected_props("Visit phonecompany.com for details.");
        assert!(!props.contains(&"Phone".to_string()), "got {props:?}");
    }

    #[test]
    fn props_user_bank_scene_has_no_phantom_binoculars() {
        // Regression test using the user-reported scene text verbatim.
        // The scene mentions ski masks, a revolver, a pistol, cash
        // drawers, a key, a safe, a sofa, a clock — but never any kind
        // of optical instrument. Pure-tier-2 detection (no dedupe)
        // must not produce Binoculars.
        let action = "\
TWO SILHOUETTES. MEN. Hard to say who for two reasons: the \
morning sun through the glass door and the ski masks pulled over \
their faces. The robber kneels below the glare and points a \
revolver at her face. She scrambles to her feet and is escorted \
behind the counter. She opens the drawers. Sure enough, they're \
empty. The other robber grabs her by the arm and drags her \
around to the main foyer of the little bank. Points his pistol \
at her head. She does. Right in the middle of the bank.";

        let bd = breakdown_for_action(action);
        assert!(
            !bd.props
                .iter()
                .any(|p| p.eq_ignore_ascii_case("binoculars")),
            "phantom Binoculars in user-reported scene, got {:?}",
            bd.props
        );
        // Positive verification: the props that ARE present should be
        // detected.
        assert!(
            bd.props.iter().any(|p| p == "Revolver"),
            "should detect Revolver"
        );
        assert!(
            bd.props.iter().any(|p| p == "Pistol"),
            "should detect Pistol"
        );
    }

    #[test]
    fn scan_caps_introductions_skips_screenplay_directions() {
        let mut element = ScreenplayElement {
            id: "s1.e1".into(),
            kind: ElementKind::Action,
            text: "CUT TO: SUDDENLY MAYA enters.".into(),
            tags: vec![],
        };
        let scene = ParsedScene {
            scene_number: 1,
            heading: String::new(),
            body: String::new(),
            page_hint: None,
            elements: vec![],
        };
        let mut builder = Builder::new("t", &scene);
        let ctx = Context::empty();

        let text = element.text.clone();
        scan_caps_introductions(&mut builder, &mut element, &text, &ctx);

        // CUT TO and SUDDENLY filtered; MAYA emitted as Introduction.
        let intros: Vec<_> = element
            .tags
            .iter()
            .filter(|t| t.category == "Introduction")
            .filter_map(|t| t.value.clone())
            .collect();
        assert_eq!(intros, vec!["Maya".to_string()]);
    }
}
