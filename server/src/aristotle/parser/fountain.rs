//! Fountain parser.
//!
//! Fountain is plain-text screenplay markup. We walk it line-by-line with
//! a small state machine that distinguishes scene headings, character
//! cues, dialogue, parentheticals, transitions, and action.
//!
//! Inline `[[CAT: value]]` notes are captured as tier-0 [`Tag`]s and
//! attached to the surrounding element — that's the Fountain workflow for
//! manual breakdown markup.

use crate::aristotle::models::{ElementKind, ParsedScript, ScreenplayElement, Tag, TagSource};
use crate::aristotle::parser::{ParseError, build_scene, extract_metadata};
use regex::Regex;

pub fn parse(data: &[u8]) -> Result<ParsedScript, ParseError> {
    let text = String::from_utf8_lossy(data).to_string();
    let mut metadata = extract_metadata(&text);

    // Fountain title page: "Key: Value" lines before first double newline.
    let body_start = if let Some(title_block_end) = text.find("\n\n") {
        text[..title_block_end]
            .lines()
            .filter_map(|line| line.split_once(':'))
            .for_each(|(key, value)| {
                let value = value.trim().to_string();
                match key.trim().to_lowercase().as_str() {
                    "title" => metadata.title = Some(value),
                    "author" | "authors" => {
                        metadata.writers = value
                            .split([',', '&'])
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    "credit" => metadata.credit_line = Some(value),
                    "draft date" | "draft_date" => metadata.draft_date = Some(value),
                    "contact" => metadata.contact_info = Some(value),
                    "source" | "notes" => metadata.other_notes = Some(value),
                    _ => {}
                }
            });
        title_block_end + 2
    } else {
        0
    };

    let scenes = parse_body(&text[body_start..]);

    Ok(ParsedScript {
        metadata,
        scenes,
        raw_text: text,
    })
}

fn parse_body(text: &str) -> Vec<crate::aristotle::models::ParsedScene> {
    let heading_re = Regex::new(r"^[ \t]*(INT\.|EXT\.|INT/EXT\.|I/E\.|EST\.)").unwrap();
    let forced_heading_re = Regex::new(r"^\.[A-Za-z0-9]").unwrap();
    let transition_re = Regex::new(r"^[A-Z][A-Z0-9 ]+TO:\s*$").unwrap();
    let note_re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    // Fountain page-break / section separator: a run of `=` chars on
    // its own line. Treated like a blank line so it doesn't get picked
    // up as anything else.
    let separator_re = Regex::new(r"^[=\-]{3,}$").unwrap();

    // Strip /* boneyard */ before walking lines.
    let cleaned = strip_boneyard(text);

    let mut scenes: Vec<Vec<ScreenplayElement>> = Vec::new();
    let mut current: Vec<ScreenplayElement> = Vec::new();
    // Front-matter content before the first scene heading (FADE IN:,
    // transitions, etc.) is discarded — Fountain models scenes as
    // content rooted at a SceneHeading, and our schema does too.
    let mut seen_first_heading = false;

    let lines: Vec<&str> = cleaned.lines().collect();
    let mut prev_blank = true;
    let mut last_was_character = false;

    for (i, raw_line) in lines.iter().enumerate() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() || separator_re.is_match(line.trim()) {
            prev_blank = true;
            last_was_character = false;
            continue;
        }

        let (clean, inline_tags) = extract_inline_tags(line, &note_re);

        let next_blank = lines.get(i + 1).is_none_or(|l| {
            let t = l.trim();
            t.is_empty() || separator_re.is_match(t)
        });

        let kind = classify_line(
            &clean,
            prev_blank,
            next_blank,
            last_was_character,
            &heading_re,
            &forced_heading_re,
            &transition_re,
        );

        if matches!(kind, ElementKind::SceneHeading) {
            if !current.is_empty() && seen_first_heading {
                scenes.push(std::mem::take(&mut current));
            } else {
                // Discard any pre-first-heading content (FADE IN:, etc.).
                current.clear();
            }
            seen_first_heading = true;
        } else if !seen_first_heading {
            // Skip front matter so it doesn't become a phantom scene.
            prev_blank = false;
            last_was_character = matches!(kind, ElementKind::Character);
            continue;
        }

        let text = match kind {
            ElementKind::SceneHeading if clean.starts_with('.') => clean[1..].trim().to_string(),
            _ => clean.trim().to_string(),
        };

        last_was_character = matches!(kind, ElementKind::Character);
        prev_blank = false;

        current.push(ScreenplayElement {
            id: String::new(),
            kind,
            text,
            tags: inline_tags,
        });
    }

    if !current.is_empty() && seen_first_heading {
        scenes.push(current);
    }

    scenes
        .into_iter()
        .enumerate()
        .map(|(i, elements)| build_scene(i + 1, elements))
        .collect()
}

fn classify_line(
    line: &str,
    prev_blank: bool,
    next_blank: bool,
    last_was_character: bool,
    heading_re: &Regex,
    forced_heading_re: &Regex,
    transition_re: &Regex,
) -> ElementKind {
    let trimmed = line.trim();

    if heading_re.is_match(line) || forced_heading_re.is_match(line) {
        return ElementKind::SceneHeading;
    }

    if let Some(stripped) = trimmed.strip_prefix('>') {
        let _ = stripped;
        return ElementKind::Transition;
    }
    if prev_blank && next_blank && transition_re.is_match(trimmed) {
        return ElementKind::Transition;
    }

    if last_was_character {
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            return ElementKind::Parenthetical;
        }
        return ElementKind::Dialogue;
    }

    if prev_blank && is_character_cue(trimmed) {
        return ElementKind::Character;
    }

    if trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() > 2 {
        // mid-dialogue parenthetical with no prior cue
        return ElementKind::Parenthetical;
    }

    ElementKind::Action
}

/// A Fountain character cue is an all-caps line (letters + spaces + a few
/// punctuation marks) preceded by a blank line. Allow trailing `(V.O.)`,
/// `(O.S.)`, etc. Reject lines that match common screenplay
/// transition / structural words (`FADE IN:`, `FADE OUT.`, `THE END`,
/// `CUT TO`, etc.) — those are not character cues.
fn is_character_cue(s: &str) -> bool {
    let core = s.split('(').next().unwrap_or(s).trim();
    if core.len() < 2 {
        return false;
    }
    if !core.chars().any(|c| c.is_alphabetic()) {
        return false;
    }
    let core_ok = core.chars().all(|c| {
        c.is_uppercase() || !c.is_alphabetic() || matches!(c, ' ' | '\'' | '-' | '.' | ':')
    });
    if !core_ok {
        return false;
    }
    // Strip trailing punctuation before testing against the slug list
    // so `FADE IN:` / `FADE OUT.` / `THE END.` all match.
    let stripped = core.trim_end_matches([':', '.', ' ']);
    !crate::aristotle::parser::is_slug_word(stripped)
}

/// Pull `[[CAT: value]]` notes out of a line. Returns the cleaned line and
/// any tags produced. The note syntax `[[CAT: value]]` means category
/// `CAT`, value `value`. A bare `[[anything]]` becomes a `Note` tag.
fn extract_inline_tags(line: &str, note_re: &Regex) -> (String, Vec<Tag>) {
    let mut tags = Vec::new();
    let cleaned = note_re
        .replace_all(line, |caps: &regex::Captures<'_>| {
            let inner = caps[1].trim();
            if let Some((cat, val)) = inner.split_once(':') {
                tags.push(Tag {
                    category: cat.trim().to_string(),
                    value: Some(val.trim().to_string()),
                    source: TagSource::NativeFountain,
                    confidence: 1.0,
                });
            } else {
                tags.push(Tag {
                    category: "Note".to_string(),
                    value: Some(inner.to_string()),
                    source: TagSource::NativeFountain,
                    confidence: 1.0,
                });
            }
            String::new()
        })
        .into_owned();

    (cleaned, tags)
}

fn strip_boneyard(text: &str) -> String {
    let re = Regex::new(r"(?s)/\*.*?\*/").unwrap();
    re.replace_all(text, "").into_owned()
}
