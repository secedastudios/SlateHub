//! PDF screenplay parser.
//!
//! Uses `pdf-extract` to pull text out of the PDF, then runs the
//! generic indent-based [`classify_screenplay_lines`] on each scene
//! body. Whether character cues are recovered depends entirely on
//! whether `pdf-extract` preserved the original whitespace from a
//! column-aligned screenplay layout. We log a text preview so a quick
//! `RUST_LOG=aristotle=info` run shows what came out of the PDF.

use crate::aristotle::models::{ElementKind, ParsedScript};
use crate::aristotle::parser::{ParseError, extract_metadata, split_into_scenes};
use tracing::{info, warn};

pub fn parse(data: &[u8]) -> Result<ParsedScript, ParseError> {
    let text = pdf_extract::extract_text_from_mem(data)
        .map_err(|e| ParseError::Parse(format!("PDF extraction failed: {e}")))?;

    let preview: String = text.chars().take(400).collect();
    info!(
        chars = text.len(),
        preview = %preview.replace('\n', "\\n"),
        "pdf text extracted"
    );

    let metadata = extract_metadata(&text);
    let scenes = split_into_scenes(&text);

    let mut char_total = 0usize;
    let mut action_total = 0usize;
    let mut dialogue_total = 0usize;
    for scene in &scenes {
        for el in &scene.elements {
            match el.kind {
                ElementKind::Character => char_total += 1,
                ElementKind::Action => action_total += 1,
                ElementKind::Dialogue => dialogue_total += 1,
                _ => {}
            }
        }
    }
    info!(
        scenes = scenes.len(),
        characters = char_total,
        actions = action_total,
        dialogue = dialogue_total,
        "pdf parsed"
    );
    if char_total == 0 && scenes.len() > 1 {
        warn!(
            "no character cues detected — PDF text may have lost \
             whitespace during extraction. Cast will be empty unless \
             characters are tagged manually."
        );
    }

    Ok(ParsedScript {
        metadata,
        scenes,
        raw_text: text,
    })
}
