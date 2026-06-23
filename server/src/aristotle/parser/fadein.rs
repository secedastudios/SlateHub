//! Fade In (.fadein / .fdr) parser.
//!
//! Delegates to the `osf` crate, which exposes scenes as `(heading, body)`
//! pairs — it doesn't expose paragraph-level structure. For each scene we
//! emit a `SceneHeading` element followed by one `Action` element holding
//! the body. The structural breakdown tier mines the body text directly.
//!
//! TODO: re-parse the body with the Fountain classifier to recover typed
//! Character/Dialogue/Parenthetical elements. The osf paragraph stream is
//! internal to the crate; either upstream a richer API or re-derive from
//! the indented body string here.

use crate::aristotle::models::{ElementKind, ParsedScript, ScreenplayElement, ScriptMetadata};
use crate::aristotle::parser::{ParseError, build_scene};

pub fn parse(data: &[u8]) -> Result<ParsedScript, ParseError> {
    match osf::parse(data) {
        Ok(doc) => Ok(osf_to_parsed_script(doc)),
        Err(_) => {
            // Fallback: treat as plain text (exported files).
            let text = String::from_utf8_lossy(data).to_string();
            let metadata = crate::aristotle::parser::extract_metadata(&text);
            let scenes = crate::aristotle::parser::split_into_scenes(&text);
            Ok(ParsedScript {
                metadata,
                scenes,
                raw_text: text,
            })
        }
    }
}

fn osf_to_parsed_script(doc: osf::OsfDocument) -> ParsedScript {
    let metadata = ScriptMetadata {
        title: doc.title_page.title,
        writers: doc.title_page.authors,
        draft_date: doc.title_page.draft,
        contact_info: doc.title_page.contact,
        credit_line: None,
        subtitle_or_quote: None,
        other_notes: doc.title_page.copyright,
    };

    let scenes = doc
        .scenes
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let mut elements = vec![ScreenplayElement {
                id: String::new(),
                kind: ElementKind::SceneHeading,
                text: s.heading.clone(),
                tags: vec![],
            }];
            if !s.body.trim().is_empty() {
                elements.push(ScreenplayElement {
                    id: String::new(),
                    kind: ElementKind::Action,
                    text: s.body.clone(),
                    tags: vec![],
                });
            }
            let mut scene = build_scene(i + 1, elements);
            scene.page_hint = s.page.map(|p| p.to_string());
            // Preserve original scene numbering if the source provided one.
            scene.scene_number = s.number;
            scene
        })
        .collect();

    ParsedScript {
        metadata,
        scenes,
        raw_text: doc.raw_text,
    }
}
