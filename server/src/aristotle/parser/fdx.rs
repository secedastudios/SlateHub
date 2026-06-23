//! Final Draft FDX parser.
//!
//! FDX is XML. We stream through it and emit one [`ScreenplayElement`] per
//! `<Paragraph>`, grouping into scenes at `Type="Scene Heading"` boundaries.
//!
//! If the file was processed by Final Draft's Tagger, the `<TagData>` block
//! at the end of the document carries breakdown categories and tag
//! definitions. Inline `<Text>` runs reference those definitions via a
//! `TagId`/`TagNumber` attribute. We capture both during parsing and attach
//! [`Tag`]s to the elements that contain the referenced runs.

use crate::aristotle::models::{
    ElementKind, ParsedScript, ScreenplayElement, ScriptMetadata, Tag, TagSource,
};
use crate::aristotle::parser::{ParseError, build_scene};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use std::collections::HashMap;

pub fn parse(data: &[u8]) -> Result<ParsedScript, ParseError> {
    let xml_str = String::from_utf8_lossy(data);
    let mut reader = Reader::from_str(&xml_str);

    let mut state = FdxState::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => handle_start(&e, &mut state, false),
            Ok(Event::Empty(e)) => handle_start(&e, &mut state, true),
            Ok(Event::Text(e)) => {
                let txt = e.unescape().unwrap_or_default().to_string();
                if state.in_text {
                    if state.in_title_page && !state.current_tp_key.is_empty() {
                        state
                            .title_page_values
                            .push((state.current_tp_key.clone(), txt.clone()));
                    }
                    let styled = wrap_with_style(&txt, state.current_style.as_deref());
                    if !state.current_text.is_empty() {
                        state.current_text.push(' ');
                    }
                    state.current_text.push_str(&styled);
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                handle_end(&name, &mut state);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(ParseError::Parse(format!("XML parse error: {err}"))),
            _ => {}
        }
    }

    if let Some(acc) = state.current_scene.take() {
        state.scenes_acc.push(acc);
    }

    let mut metadata = ScriptMetadata::default();
    apply_metadata(&mut metadata, &state.title_page_values);
    // Fall back to positional heuristics when the file uses the
    // standard `<Paragraph><Text>…</Text></Paragraph>` title-page layout
    // (no `Type` attributes) — the writer-half of this module emits
    // exactly that shape, so this is what closes the round-trip.
    if metadata.title.is_none() && !state.title_page_lines.is_empty() {
        let blob = state.title_page_lines.join("\n");
        metadata = crate::aristotle::parser::extract_metadata(&blob);
    }

    let scenes: Vec<_> = state
        .scenes_acc
        .into_iter()
        .enumerate()
        .map(|(i, acc)| {
            let mut elements = acc.elements;
            attach_tags(
                &mut elements,
                &acc.tag_refs,
                &state.tag_categories,
                &state.tag_definitions,
            );
            build_scene(i + 1, elements)
        })
        .collect();

    let raw_text = scenes
        .iter()
        .map(|s| format!("{}\n{}\n", s.heading, s.body))
        .collect::<String>();

    Ok(ParsedScript {
        metadata,
        scenes,
        raw_text,
    })
}

// ── State ──

#[derive(Default)]
struct FdxState {
    /// Paragraph currently being collected (text only — element is built at End).
    current_type: String,
    current_text: String,
    /// Tag refs collected during this paragraph. Resolved to Tag entries
    /// when the paragraph closes.
    pending_tag_refs: Vec<String>,
    /// Inline run style for the currently-open `<Text>` element. FDX
    /// uses `Style="Bold"`, `Style="Italic"`, `Style="Bold+Italic"`, etc.
    /// We reset on every `<Text>` open and use it to wrap the inner
    /// text with markdown markers when it closes.
    current_style: Option<String>,
    in_text: bool,
    in_title_page: bool,
    current_tp_key: String,
    title_page_values: Vec<(String, String)>,
    /// Plain title-page paragraph text, used when the file omits
    /// `<Content Type="…">` and just stacks `<Paragraph>` blocks.
    title_page_lines: Vec<String>,
    /// Definitions and categories collected anywhere in the doc.
    tag_categories: HashMap<String, String>,
    tag_definitions: HashMap<String, TagDefinition>,
    /// Scene accumulator. `current_scene` is the in-flight scene; when a new
    /// scene heading is encountered it's pushed to `scenes_acc`.
    current_scene: Option<ParsedSceneAcc>,
    scenes_acc: Vec<ParsedSceneAcc>,
}

#[derive(Default)]
struct TagDefinition {
    category_id: String,
    label: Option<String>,
}

#[derive(Default)]
struct ParsedSceneAcc {
    elements: Vec<ScreenplayElement>,
    /// `(element_index, tag_ref_id)` pairs. Resolved against the tag-data
    /// tables once the whole document has been parsed.
    tag_refs: Vec<(usize, String)>,
}

// ── Handlers ──

fn handle_start(e: &BytesStart<'_>, state: &mut FdxState, self_closing: bool) {
    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
    match name.as_str() {
        "Paragraph" => {
            state.current_type.clear();
            state.current_text.clear();
            state.pending_tag_refs.clear();
            for attr in e.attributes().flatten() {
                if attr.key.as_ref() == b"Type" {
                    state.current_type = String::from_utf8_lossy(&attr.value).to_string();
                }
            }
        }
        "Text" => {
            state.in_text = true;
            state.current_style = None;
            for attr in e.attributes().flatten() {
                let key = attr.key.as_ref();
                let val = String::from_utf8_lossy(&attr.value).to_string();
                match key {
                    b"TagId" | b"TagNumber" | b"TagDef" => state.pending_tag_refs.push(val),
                    b"Style" => state.current_style = Some(val),
                    _ => {}
                }
            }
            if self_closing {
                state.in_text = false;
            }
        }
        "TitlePage" => state.in_title_page = true,
        "Content" if state.in_title_page => {
            for attr in e.attributes().flatten() {
                if attr.key.as_ref() == b"Type" {
                    state.current_tp_key = String::from_utf8_lossy(&attr.value).to_string();
                }
            }
        }
        // <Category Number="1" Name="Cast Members"/> or <TagCategory Id=".." Name=".."/>
        "Category" | "TagCategory" => capture_category(e, &mut state.tag_categories),
        // <Tag Number="3" CatId="9" Name="Glock"/> or <TagDefinition Id=".." CatId=".." Label=".."/>
        "Tag" | "TagDefinition" => capture_definition(e, &mut state.tag_definitions),
        _ => {}
    }
}

fn handle_end(name: &str, state: &mut FdxState) {
    match name {
        "Text" => state.in_text = false,
        "TitlePage" => state.in_title_page = false,
        "Content" => state.current_tp_key.clear(),
        "Paragraph" if !state.in_title_page => {
            let text = std::mem::take(&mut state.current_text);
            let refs = std::mem::take(&mut state.pending_tag_refs);
            let kind = type_to_kind(&state.current_type);
            push_element(state, kind, text, refs);
            state.current_type.clear();
        }
        "Paragraph" => {
            // In a title page: capture the paragraph text as one line.
            let text = std::mem::take(&mut state.current_text);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                state.title_page_lines.push(trimmed.to_string());
            }
            state.current_type.clear();
            state.pending_tag_refs.clear();
        }
        _ => {}
    }
}

fn push_element(state: &mut FdxState, kind: ElementKind, text: String, tag_refs: Vec<String>) {
    let text = text.trim().to_string();
    if text.is_empty() && !matches!(kind, ElementKind::SceneHeading) {
        return;
    }
    if matches!(kind, ElementKind::Other(ref s) if s.is_empty()) {
        return;
    }

    if matches!(kind, ElementKind::SceneHeading) {
        if let Some(prev) = state.current_scene.take() {
            state.scenes_acc.push(prev);
        }
        state.current_scene = Some(ParsedSceneAcc::default());
    }

    let scene = state
        .current_scene
        .get_or_insert_with(ParsedSceneAcc::default);
    let element_idx = scene.elements.len();
    scene.elements.push(ScreenplayElement {
        id: String::new(),
        kind,
        text,
        tags: vec![],
    });
    for tag_ref in tag_refs {
        scene.tag_refs.push((element_idx, tag_ref));
    }
}

fn type_to_kind(s: &str) -> ElementKind {
    match s {
        "Scene Heading" => ElementKind::SceneHeading,
        "Action" => ElementKind::Action,
        "Character" => ElementKind::Character,
        "Parenthetical" => ElementKind::Parenthetical,
        "Dialogue" => ElementKind::Dialogue,
        "Transition" => ElementKind::Transition,
        "Shot" => ElementKind::Shot,
        "" => ElementKind::Other(String::new()),
        other => ElementKind::Other(other.to_string()),
    }
}

fn capture_category(e: &BytesStart<'_>, out: &mut HashMap<String, String>) {
    let (mut id, mut name) = (None, None);
    for attr in e.attributes().flatten() {
        let val = String::from_utf8_lossy(&attr.value).to_string();
        match attr.key.as_ref() {
            b"Number" | b"Id" | b"id" => id = Some(val),
            b"Name" => name = Some(val),
            _ => {}
        }
    }
    if let (Some(id), Some(name)) = (id, name) {
        out.insert(id, name);
    }
}

fn capture_definition(e: &BytesStart<'_>, out: &mut HashMap<String, TagDefinition>) {
    let mut id = String::new();
    let mut def = TagDefinition::default();
    for attr in e.attributes().flatten() {
        let val = String::from_utf8_lossy(&attr.value).to_string();
        match attr.key.as_ref() {
            b"Number" | b"Id" | b"id" => id = val,
            b"CatId" | b"CategoryId" | b"Category" => def.category_id = val,
            b"Name" | b"Label" => def.label = Some(val),
            _ => {}
        }
    }
    if !id.is_empty() {
        out.insert(id, def);
    }
}

fn attach_tags(
    elements: &mut [ScreenplayElement],
    refs: &[(usize, String)],
    categories: &HashMap<String, String>,
    definitions: &HashMap<String, TagDefinition>,
) {
    for (idx, tag_ref) in refs {
        let Some(def) = definitions.get(tag_ref) else {
            continue;
        };
        let category = categories
            .get(&def.category_id)
            .cloned()
            .unwrap_or_else(|| "Uncategorized".into());

        if let Some(el) = elements.get_mut(*idx) {
            el.tags.push(Tag {
                category,
                value: def.label.clone(),
                source: TagSource::NativeFdx,
                confidence: 1.0,
            });
        }
    }
}

/// Wrap a text run with markdown markers based on the FDX `Style`
/// attribute. We only support Bold and Italic — Underline and other
/// rarer styles round-trip as plain text. Empty input or empty style
/// passes through unchanged so we don't litter the text with empty
/// markers.
pub(crate) fn wrap_with_style(text: &str, style: Option<&str>) -> String {
    let Some(style) = style else {
        return text.to_string();
    };
    if text.is_empty() {
        return text.to_string();
    }
    let parts: Vec<&str> = style.split('+').collect();
    let bold = parts.iter().any(|p| p.eq_ignore_ascii_case("Bold"));
    let italic = parts.iter().any(|p| p.eq_ignore_ascii_case("Italic"));
    match (bold, italic) {
        (true, true) => format!("***{text}***"),
        (true, false) => format!("**{text}**"),
        (false, true) => format!("_{text}_"),
        _ => text.to_string(),
    }
}

fn apply_metadata(meta: &mut ScriptMetadata, values: &[(String, String)]) {
    for (key, value) in values {
        match key.to_lowercase().as_str() {
            "title" => meta.title = Some(value.clone()),
            "written by" | "screenplay by" | "author" => {
                meta.credit_line = Some(key.clone());
                meta.writers.push(value.clone());
            }
            "draft date" => meta.draft_date = Some(value.clone()),
            "contact" => meta.contact_info = Some(value.clone()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paragraphs_into_typed_elements() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<FinalDraft Version="3">
  <Content>
    <Paragraph Type="Scene Heading"><Text>INT. KITCHEN - DAY</Text></Paragraph>
    <Paragraph Type="Action"><Text>Maya stirs a pot.</Text></Paragraph>
    <Paragraph Type="Character"><Text>MAYA</Text></Paragraph>
    <Paragraph Type="Dialogue"><Text>Almost ready.</Text></Paragraph>
  </Content>
</FinalDraft>"#;

        let parsed = parse(xml.as_bytes()).expect("parses");
        assert_eq!(parsed.scenes.len(), 1);
        let scene = &parsed.scenes[0];

        // Each Paragraph maps to exactly one ScreenplayElement of the
        // corresponding kind.
        let kinds: Vec<_> = scene.elements.iter().map(|e| &e.kind).collect();
        assert_eq!(kinds[0], &ElementKind::SceneHeading);
        assert_eq!(kinds[1], &ElementKind::Action);
        assert_eq!(kinds[2], &ElementKind::Character);
        assert_eq!(kinds[3], &ElementKind::Dialogue);
    }

    #[test]
    fn ingests_tagger_marked_spans() {
        // Final Draft Tagger emits TagData with Category + Tag entries
        // and inline TagNumber references on <Text>. The parser should
        // attach the right Tag to the right element.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<FinalDraft Version="3">
  <Content>
    <Paragraph Type="Scene Heading"><Text>INT. KITCHEN - DAY</Text></Paragraph>
    <Paragraph Type="Action">
      <Text>She picks up the </Text>
      <Text TagNumber="9">briefcase</Text>
      <Text>.</Text>
    </Paragraph>
  </Content>
  <TagData>
    <TagCategories>
      <Category Number="2" Name="Props"/>
    </TagCategories>
    <Tags>
      <Tag Number="9" CatId="2" Name="briefcase"/>
    </Tags>
  </TagData>
</FinalDraft>"#;

        let parsed = parse(xml.as_bytes()).expect("parses");
        let scene = &parsed.scenes[0];
        let action = scene
            .elements
            .iter()
            .find(|e| e.kind == ElementKind::Action)
            .expect("has action element");

        assert_eq!(action.tags.len(), 1);
        let tag = &action.tags[0];
        assert_eq!(tag.category, "Props");
        assert_eq!(tag.value.as_deref(), Some("briefcase"));
        assert_eq!(tag.source, TagSource::NativeFdx);
    }

    #[test]
    fn standard_title_page_round_trips() {
        // No <Content Type="…"> attributes — just centered paragraphs.
        // This is the format our own FDX exporter emits.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<FinalDraft Version="3">
  <Content>
    <Paragraph Type="Scene Heading"><Text>INT. ROOM - DAY</Text></Paragraph>
  </Content>
  <TitlePage>
    <Content>
      <Paragraph Alignment="Center"><Text>SAMPLE SCRIPT</Text></Paragraph>
      <Paragraph Alignment="Center"><Text>Written by</Text></Paragraph>
      <Paragraph Alignment="Center"><Text>Jane Doe</Text></Paragraph>
    </Content>
  </TitlePage>
</FinalDraft>"#;

        let parsed = parse(xml.as_bytes()).expect("parses");
        assert_eq!(parsed.metadata.title.as_deref(), Some("SAMPLE SCRIPT"));
        assert_eq!(parsed.metadata.writers, vec!["Jane Doe".to_string()]);
    }

    #[test]
    fn parses_bold_italic_style_into_markdown_markers() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<FinalDraft Version="3">
  <Content>
    <Paragraph Type="Scene Heading"><Text>INT. ROOM - DAY</Text></Paragraph>
    <Paragraph Type="Action">
      <Text>She enters and shouts </Text>
      <Text Style="Bold">stop!</Text>
      <Text> in </Text>
      <Text Style="Italic">complete</Text>
      <Text> silence — </Text>
      <Text Style="Bold+Italic">absolute</Text>
      <Text> silence.</Text>
    </Paragraph>
  </Content>
</FinalDraft>"#;

        let parsed = parse(xml.as_bytes()).expect("parses");
        let scene = &parsed.scenes[0];
        let action = scene
            .elements
            .iter()
            .find(|e| e.kind == ElementKind::Action)
            .expect("action element");

        assert!(action.text.contains("**stop!**"));
        assert!(action.text.contains("_complete_"));
        assert!(action.text.contains("***absolute***"));
    }

    #[test]
    fn wrap_with_style_handles_known_styles() {
        assert_eq!(wrap_with_style("x", Some("Bold")), "**x**");
        assert_eq!(wrap_with_style("x", Some("Italic")), "_x_");
        assert_eq!(wrap_with_style("x", Some("Bold+Italic")), "***x***");
        assert_eq!(wrap_with_style("x", Some("Italic+Bold")), "***x***");
        // Underline alone drops to plain — we don't support it yet.
        assert_eq!(wrap_with_style("x", Some("Underline")), "x");
        assert_eq!(wrap_with_style("x", None), "x");
        assert_eq!(wrap_with_style("", Some("Bold")), "");
    }

    #[test]
    fn typed_content_title_page_still_works() {
        // The other historical format: <Content Type="title">…</Content>.
        // Our parser supports both; verify the legacy path didn't
        // regress when we added the standard-paragraph fallback.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<FinalDraft Version="3">
  <Content>
    <Paragraph Type="Scene Heading"><Text>INT. ROOM - DAY</Text></Paragraph>
  </Content>
  <TitlePage>
    <Content Type="Title"><Text>LEGACY TITLE</Text></Content>
    <Content Type="Written by"><Text>Old Author</Text></Content>
  </TitlePage>
</FinalDraft>"#;

        let parsed = parse(xml.as_bytes()).expect("parses");
        assert_eq!(parsed.metadata.title.as_deref(), Some("LEGACY TITLE"));
        assert!(parsed.metadata.writers.contains(&"Old Author".to_string()));
    }
}
