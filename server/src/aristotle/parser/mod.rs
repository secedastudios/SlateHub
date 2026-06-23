//! Screenplay file parsers. Supports PDF, Fountain, Final Draft (FDX),
//! and Fade In (.fadein/.fdr via the `osf` crate).
//!
//! Each format has its own submodule, but they all produce the same
//! [`ParsedScript`] struct. The public entry point is [`parse_screenplay`],
//! which auto-detects the format from the file extension.
//!
//! Shared helpers here:
//! - [`extract_metadata`] - regex heuristics for title-page info (used by
//!   formats that don't have structured metadata, like PDF)
//! - [`split_into_scenes`] - splits raw text at scene headings (INT./EXT.)

mod fadein;
mod fdx;
mod fountain;
mod pdf;

use crate::aristotle::models::{
    ElementKind, ParsedScene, ParsedScript, ScreenplayElement, ScriptMetadata,
};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unsupported file format: {0}")]
    UnsupportedFormat(String),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Detect the screenplay format from the filename extension and parse.
/// Returns an error for unknown extensions. The detection is case-insensitive.
pub fn parse_screenplay(filename: &str, data: &[u8]) -> Result<ParsedScript, ParseError> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "pdf" => pdf::parse(data),
        "fountain" => fountain::parse(data),
        "fdx" => fdx::parse(data),
        "fadein" | "fdr" => fadein::parse(data),
        other => Err(ParseError::UnsupportedFormat(other.to_string())),
    }
}

/// Best-effort metadata extraction from the first 40 lines of text.
/// Looks for title (first non-empty line), credit lines ("Written by"),
/// draft dates, contact info (email/phone patterns), quotes, and WGA numbers.
/// The LLM does a better job on a second pass, but this works as a fallback.
pub fn extract_metadata(text: &str) -> ScriptMetadata {
    let lines: Vec<&str> = text.lines().take(40).collect();
    let top_block = lines.join("\n");

    let title = lines
        .iter()
        .map(|l| l.trim())
        .find(|l| l.len() > 1)
        .map(|l| l.to_string());

    let mut meta = ScriptMetadata {
        title,
        ..Default::default()
    };

    // Captures both "Written by [Author]" inline and "Written by\n…\nAuthor"
    // forms. Group 1 is the credit phrase, group 2 is any trailing
    // author name on the same line (may be empty).
    let re_written_by = regex::Regex::new(
        r"(?i)^(written\s+by|screenplay\s+by|story\s+by|teleplay\s+by|by)\b[\s:.-]*(.*)$",
    )
    .unwrap();
    let re_draft = regex::Regex::new(r"(?i)(draft|revision|rev\.?)").unwrap();
    let re_phone = regex::Regex::new(r"\d{3}[.\-]\d{3}").unwrap();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if meta.writers.is_empty()
            && let Some(caps) = re_written_by.captures(trimmed)
        {
            meta.credit_line = Some(caps[1].to_string());
            let same_line = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            let writer_text = if !same_line.is_empty() {
                same_line.to_string()
            } else {
                // Find the next non-blank line as the author name.
                lines
                    .iter()
                    .skip(i + 1)
                    .map(|l| l.trim())
                    .find(|l| !l.is_empty())
                    .unwrap_or("")
                    .to_string()
            };
            meta.writers = split_writers(&writer_text);
        }

        if re_draft.is_match(trimmed) {
            meta.draft_date = Some(trimmed.to_string());
        }

        if meta.contact_info.is_none() && (trimmed.contains('@') || re_phone.is_match(trimmed)) {
            meta.contact_info = Some(trimmed.to_string());
        }
    }

    meta.subtitle_or_quote = lines
        .iter()
        .skip(1)
        .take(3)
        .map(|l| l.trim())
        .find(|l| l.starts_with('"') || l.starts_with('\u{201C}'))
        .map(|l| l.to_string());

    if let Some(pos) = top_block.to_lowercase().find("wga") {
        let snippet = &top_block[pos..];
        meta.other_notes = Some(
            snippet
                .find('\n')
                .map_or(snippet, |end| &snippet[..end])
                .trim()
                .to_string(),
        );
    }

    meta
}

/// Split a writer credit string into individual names. Handles the
/// common collaborator separators `&` (shared credit) and `and`
/// (co-writer), plus commas. Drops anything that looks like extra
/// metadata (parentheticals after the names).
fn split_writers(s: &str) -> Vec<String> {
    let cleaned = s.split('(').next().unwrap_or(s).trim();
    let normalized = cleaned.replace(" and ", " & ");
    normalized
        .split(['&', ','])
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Split raw screenplay text at scene headings (lines starting with
/// INT., EXT., INT/EXT., or I/E.) and tokenize each scene's body into
/// typed elements via [`classify_screenplay_lines`].
///
/// This is the fallback path for formats that don't carry typed paragraph
/// structure (PDF, plain text). It uses indentation + ALL-CAPS
/// heuristics to recover Character/Dialogue/Parenthetical/Transition
/// elements from layout — that's what unlocks cast detection on PDFs.
/// Format-specific parsers (FDX, Fountain, Fade In) should populate
/// `elements` directly via [`build_scene`] instead.
pub fn split_into_scenes(text: &str) -> Vec<ParsedScene> {
    let heading_re = regex::Regex::new(r"(?m)^[ \t]*(INT\.|EXT\.|INT/EXT\.|I/E\.)[^\n]+").unwrap();

    let matches: Vec<_> = heading_re.find_iter(text).collect();

    matches
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let scene_number = i + 1;
            let heading = m.as_str().trim().to_string();
            let end = matches.get(i + 1).map_or(text.len(), |n| n.start());
            let body = text[m.end()..end].trim().to_string();

            let mut elements = vec![ScreenplayElement {
                id: String::new(),
                kind: ElementKind::SceneHeading,
                text: heading.clone(),
                tags: vec![],
            }];
            elements.extend(classify_screenplay_lines(&body));

            let scene = build_scene(scene_number, elements);
            ParsedScene {
                scene_number: scene.scene_number,
                heading,
                body,
                page_hint: None,
                elements: scene.elements,
            }
        })
        .collect()
}

/// Walk indented screenplay text and classify each non-blank line as
/// `Character` / `Parenthetical` / `Dialogue` / `Transition` / `Action`
/// using layout heuristics. Used for PDF-extracted text and other
/// unstructured plaintext sources.
///
/// Element IDs are left empty; the caller assigns them via
/// [`build_scene`]. Returned elements do NOT include `SceneHeading` —
/// the caller already has that from the slug-line regex.
///
/// Heuristics:
/// - **Character**: blank-line-preceded, non-blank-followed, ≥5 char
///   leading whitespace, ALL CAPS letters only (with `'` `-` `.` and
///   trailing `(V.O.)` / `(CONT'D)` modifiers allowed), 2–40 chars,
///   not a screenplay slug word.
/// - **Parenthetical**: inside a dialogue block, wrapped in `(...)`.
/// - **Dialogue**: inside a dialogue block, not a parenthetical.
/// - **Transition**: ALL CAPS line ending in `TO:` (with `:` optional)
///   or starting with `>`.
/// - **Action**: anything else, lines joined within a paragraph (blank
///   line ends the paragraph).
pub fn classify_screenplay_lines(text: &str) -> Vec<ScreenplayElement> {
    let lines: Vec<&str> = text.lines().collect();
    let mut elements: Vec<ScreenplayElement> = Vec::new();
    let mut action_buf: Vec<String> = Vec::new();
    let mut in_dialogue_block = false;
    let mut prev_blank = true;

    for (i, raw) in lines.iter().enumerate() {
        if raw.trim().is_empty() {
            flush_action(&mut elements, &mut action_buf);
            prev_blank = true;
            in_dialogue_block = false;
            continue;
        }

        let trimmed = raw.trim();
        let leading_ws = raw.chars().take_while(|c| matches!(c, ' ' | '\t')).count();
        let next_blank = lines.get(i + 1).is_none_or(|l| l.trim().is_empty());

        // Transition: ALL CAPS ending in "TO:" with heavy indent or
        // its own paragraph, OR explicit `>` prefix.
        if let Some(rest) = trimmed.strip_prefix('>') {
            flush_action(&mut elements, &mut action_buf);
            elements.push(ScreenplayElement {
                id: String::new(),
                kind: ElementKind::Transition,
                text: rest.trim().to_string(),
                tags: vec![],
            });
            prev_blank = false;
            in_dialogue_block = false;
            continue;
        }
        if (leading_ws >= 30 || (prev_blank && next_blank))
            && (trimmed.ends_with("TO:") || trimmed.ends_with("OUT:"))
            && is_all_caps(trimmed)
        {
            flush_action(&mut elements, &mut action_buf);
            elements.push(ScreenplayElement {
                id: String::new(),
                kind: ElementKind::Transition,
                text: trimmed.to_string(),
                tags: vec![],
            });
            prev_blank = false;
            in_dialogue_block = false;
            continue;
        }

        // Character cue: blank-line-preceded, non-blank-followed, ALL
        // CAPS name on its own line. Indent is a strong signal when
        // present but we don't require it — many PDFs lose leading
        // whitespace during text extraction.
        if prev_blank && !next_blank && is_character_cue(trimmed) {
            let _ = leading_ws; // kept for future heuristics
            flush_action(&mut elements, &mut action_buf);
            elements.push(ScreenplayElement {
                id: String::new(),
                kind: ElementKind::Character,
                text: trimmed.to_string(),
                tags: vec![],
            });
            prev_blank = false;
            in_dialogue_block = true;
            continue;
        }

        // Inside a dialogue block: parenthetical or dialogue.
        if in_dialogue_block {
            let kind = if trimmed.starts_with('(') && trimmed.ends_with(')') {
                ElementKind::Parenthetical
            } else {
                ElementKind::Dialogue
            };
            elements.push(ScreenplayElement {
                id: String::new(),
                kind,
                text: trimmed.to_string(),
                tags: vec![],
            });
            prev_blank = false;
            continue;
        }

        // Action — accumulate into current paragraph.
        action_buf.push(trimmed.to_string());
        prev_blank = false;
    }

    flush_action(&mut elements, &mut action_buf);
    elements
}

fn flush_action(elements: &mut Vec<ScreenplayElement>, buffer: &mut Vec<String>) {
    if buffer.is_empty() {
        return;
    }
    let text = buffer.join(" ");
    buffer.clear();
    elements.push(ScreenplayElement {
        id: String::new(),
        kind: ElementKind::Action,
        text,
        tags: vec![],
    });
}

/// Is `s` an ALL CAPS character name (with allowed punctuation), short
/// enough to be a cue, and not a screenplay direction word? Strips any
/// trailing `(V.O.)`/`(CONT'D)` style modifier before checking.
fn is_character_cue(s: &str) -> bool {
    let core = s.split('(').next().unwrap_or(s).trim();
    if !(2..=40).contains(&core.len()) {
        return false;
    }
    if !core.chars().any(|c| c.is_alphabetic()) {
        return false;
    }
    let core_ok = core
        .chars()
        .all(|c| c.is_uppercase() || !c.is_alphabetic() || matches!(c, ' ' | '\'' | '-' | '.'));
    if !core_ok {
        return false;
    }
    !is_slug_word(core)
}

pub(crate) fn is_slug_word(s: &str) -> bool {
    // Exact matches for transition/director shorthand.
    let exact = matches!(
        s,
        "CUT TO"
            | "FADE IN"
            | "FADE OUT"
            | "DISSOLVE TO"
            | "SMASH CUT"
            | "MATCH CUT"
            | "INTERCUT"
            | "FLASHBACK"
            | "END"
            | "THE END"
            | "POV"
            | "MORE"
            | "CONT'D"
            | "CONTINUED"
            | "BEAT"
            | "MONTAGE"
            | "SERIES OF SHOTS"
            | "TITLE"
            | "TITLES"
            | "SUPER"
            | "SUPERIMPOSE"
            | "CHYRON"
            | "PRELAP"
            | "OMITTED"
    );
    if exact {
        return true;
    }
    // Prefix matches for slug lines and shot directions that are
    // followed by their target ("ANGLE ON THE DOOR", "CLOSE ON HER
    // FACE", "INSERT - PHOTO").
    s.starts_with("INT.")
        || s.starts_with("EXT.")
        || s.starts_with("INT/EXT")
        || s.starts_with("I/E")
        || s.starts_with("ANGLE ON")
        || s.starts_with("CLOSE ON")
        || s.starts_with("CLOSE-UP")
        || s.starts_with("WIDE ON")
        || s.starts_with("WIDE SHOT")
        || s.starts_with("BACK TO")
        || s.starts_with("INSERT")
        || s.starts_with("MONTAGE")
        || s.starts_with("SERIES OF")
        || s.starts_with("TITLE OVER")
        || s.starts_with("FADE")
        || s.starts_with("DISSOLVE")
}

fn is_all_caps(s: &str) -> bool {
    s.chars().any(|c| c.is_alphabetic())
        && s.chars().all(|c| !c.is_alphabetic() || c.is_uppercase())
}

/// Build a [`ParsedScene`] from a sequence of typed elements. The first
/// `SceneHeading` element becomes the scene heading; the rest concatenate
/// into `body` (preserving the heading-less text used by legacy consumers).
///
/// Element IDs are assigned sequentially in the form `s{scene}.e{idx}`.
pub fn build_scene(scene_number: usize, mut elements: Vec<ScreenplayElement>) -> ParsedScene {
    for (i, el) in elements.iter_mut().enumerate() {
        el.id = element_id(scene_number, i);
    }

    let heading = elements
        .iter()
        .find(|e| e.kind == ElementKind::SceneHeading)
        .map(|e| e.text.clone())
        .unwrap_or_default();

    let body = elements
        .iter()
        .filter(|e| e.kind != ElementKind::SceneHeading)
        .map(|e| e.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    ParsedScene {
        scene_number,
        heading,
        body,
        page_hint: None,
        elements,
    }
}

/// Stable element ID: `s{scene}.e{idx}`.
pub fn element_id(scene_number: usize, idx: usize) -> String {
    format!("s{scene_number}.e{idx}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample of typical PDF-extracted screenplay text. Indentation is
    /// what `pdf_extract` usually preserves from a Final Draft / Movie
    /// Magic style PDF export — slug lines at column 0, character cues
    /// roughly centered, dialogue indented.
    const PDF_SAMPLE: &str = "\
INT. KITCHEN - NIGHT

Rain hammers the windows. MAYA, 30s, paces.

                    MAYA
          (whispering)
          Where are you?

She grabs a knife.

                    VIKTOR (O.S.)
          Right behind you.

                                                      CUT TO:

EXT. ALLEY - CONTINUOUS

A crowd gathers.
";

    #[test]
    fn classify_screenplay_lines_finds_character_cues() {
        let elements = classify_screenplay_lines(PDF_SAMPLE);
        let chars: Vec<&str> = elements
            .iter()
            .filter(|e| e.kind == ElementKind::Character)
            .map(|e| e.text.as_str())
            .collect();
        assert!(chars.contains(&"MAYA"));
        assert!(chars.iter().any(|c| c.starts_with("VIKTOR")));
    }

    #[test]
    fn classify_screenplay_lines_extracts_dialogue_and_parenthetical() {
        let elements = classify_screenplay_lines(PDF_SAMPLE);
        let kinds: Vec<&ElementKind> = elements.iter().map(|e| &e.kind).collect();

        assert!(kinds.contains(&&ElementKind::Parenthetical));
        assert!(kinds.contains(&&ElementKind::Dialogue));
        assert!(kinds.contains(&&ElementKind::Transition));
    }

    #[test]
    fn split_into_scenes_emits_character_elements_for_pdf_style_text() {
        // Wrapping the PDF sample in a heading-bearing document ensures
        // split_into_scenes finds scene boundaries and runs the new
        // classifier on each body.
        let scenes = split_into_scenes(PDF_SAMPLE);
        assert_eq!(scenes.len(), 2);

        let first = &scenes[0];
        assert!(first.heading.starts_with("INT. KITCHEN"));
        let char_count = first
            .elements
            .iter()
            .filter(|e| e.kind == ElementKind::Character)
            .count();
        assert!(
            char_count >= 2,
            "expected ≥2 character cues, got {char_count}"
        );
    }

    #[test]
    fn split_writers_handles_collaborator_separators() {
        assert_eq!(split_writers("Jane Doe"), vec!["Jane Doe"]);
        assert_eq!(
            split_writers("Jane Doe & John Smith"),
            vec!["Jane Doe", "John Smith"]
        );
        assert_eq!(
            split_writers("Jane Doe and John Smith"),
            vec!["Jane Doe", "John Smith"]
        );
        assert_eq!(
            split_writers("A, B & C"),
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
        // Trailing parenthetical metadata gets dropped.
        assert_eq!(split_writers("Jane Doe (WGAW)"), vec!["Jane Doe"]);
    }

    #[test]
    fn extract_metadata_finds_inline_by_author() {
        let text = "\
THE MOVIE

by Jane Doe

Draft 3.0
";
        let meta = extract_metadata(text);
        assert_eq!(meta.title.as_deref(), Some("THE MOVIE"));
        assert_eq!(meta.writers, vec!["Jane Doe".to_string()]);
    }

    #[test]
    fn extract_metadata_finds_author_after_blank_line() {
        // The classic title page layout: title, blank, "Written by",
        // blank, author.
        let text = "\
THE MOVIE


Written by


Jane Doe
";
        let meta = extract_metadata(text);
        assert_eq!(meta.writers, vec!["Jane Doe".to_string()]);
    }

    #[test]
    fn extract_metadata_splits_cowriters() {
        let text = "\
THE MOVIE

Screenplay by Jane Doe & John Smith
";
        let meta = extract_metadata(text);
        assert_eq!(
            meta.writers,
            vec!["Jane Doe".to_string(), "John Smith".to_string()]
        );
    }
}
