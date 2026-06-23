//! Data types for the aristotle library.
//!
//! Three categories:
//!
//! - **Parser output** (`ParsedScript`, `ParsedScene`, `ScriptMetadata`) — the
//!   intermediate representation between file parsing and the breakdown
//!   pipeline. Format-agnostic; every parser (PDF, Fountain, FDX, Fade In)
//!   produces these.
//! - **Breakdown markup** (`ScreenplayElement`, `ElementKind`, `Tag`,
//!   `TagSource`) — typed paragraph stream + breakdown annotations carried
//!   on elements. Populated by the parsers (native FDX tags, Fountain
//!   notes) and by the breakdown pipeline tiers.
//! - **Pipeline output** (`SceneBreakdown`) — what the breakdown pipeline
//!   produces for each scene. Pure data; callers persist as they see fit.
//! - **Ollama API types** — request/response shapes for the LLM client.
//!
//! No SurrealDB derives. Aristotle is storage-agnostic; callers map these
//! types into their own DB representation at the boundary.

use serde::{Deserialize, Serialize};

// ── Parsed screenplay structures ──

/// The result of parsing any screenplay file. Every format parser
/// (PDF, Fountain, FDX, Fade In) produces one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedScript {
    pub metadata: ScriptMetadata,
    pub scenes: Vec<ParsedScene>,
    pub raw_text: String,
}

/// Title-page info pulled from the script. The parser takes a first pass
/// with regex heuristics; an optional LLM pass on the first 2 pages can
/// improve the result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScriptMetadata {
    pub title: Option<String>,
    pub writers: Vec<String>,
    pub draft_date: Option<String>,
    pub contact_info: Option<String>,
    pub subtitle_or_quote: Option<String>,
    pub credit_line: Option<String>,
    pub other_notes: Option<String>,
}

/// One scene from a parsed screenplay. The `heading` is the slug line
/// ("INT. KITCHEN - NIGHT") and `body` is everything until the next heading.
///
/// `elements` is the typed paragraph stream when the source format carries
/// it (FDX, Fountain, Fade In). PDFs and unstructured text produce a single
/// `Action` element for the whole body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedScene {
    pub scene_number: usize,
    pub heading: String,
    pub body: String,
    pub page_hint: Option<String>,
    #[serde(default)]
    pub elements: Vec<ScreenplayElement>,
}

/// A typed paragraph inside a scene. Element IDs are stable within a parse
/// and follow the pattern `s{scene}.e{index}` so callers can reference the
/// same element when editing or rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayElement {
    pub id: String,
    pub kind: ElementKind,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

/// Screenplay paragraph kinds. Mirrors the standard set used by Final Draft,
/// Fade In, and Fountain. `Other` carries an unknown style name verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    SceneHeading,
    Action,
    Character,
    Parenthetical,
    Dialogue,
    Transition,
    Shot,
    DualDialogue,
    Note,
    Other(String),
}

impl ElementKind {
    /// Stable string form for serialization / persistence by the caller.
    /// Known variants use their snake_case names; `Other(s)` round-trips
    /// through `s`.
    pub fn as_db_str(&self) -> String {
        match self {
            Self::SceneHeading => "scene_heading".into(),
            Self::Action => "action".into(),
            Self::Character => "character".into(),
            Self::Parenthetical => "parenthetical".into(),
            Self::Dialogue => "dialogue".into(),
            Self::Transition => "transition".into(),
            Self::Shot => "shot".into(),
            Self::DualDialogue => "dual_dialogue".into(),
            Self::Note => "note".into(),
            Self::Other(s) => s.clone(),
        }
    }

    pub fn parse_db_str(s: &str) -> Self {
        match s {
            "scene_heading" => Self::SceneHeading,
            "action" => Self::Action,
            "character" => Self::Character,
            "parenthetical" => Self::Parenthetical,
            "dialogue" => Self::Dialogue,
            "transition" => Self::Transition,
            "shot" => Self::Shot,
            "dual_dialogue" => Self::DualDialogue,
            "note" => Self::Note,
            other => Self::Other(other.to_string()),
        }
    }
}

/// A breakdown annotation attached to a screenplay element. Categories
/// follow the production-breakdown vocabulary (Cast, Props, Wardrobe, …).
/// `source` records which tier of the pipeline produced the tag so the UI
/// can show confidence and let the user override anything other than `User`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub category: String,
    pub value: Option<String>,
    pub source: TagSource,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

fn default_confidence() -> f32 {
    1.0
}

/// Where a [`Tag`] came from. Higher variants in the list are higher trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagSource {
    NativeFdx,
    NativeFadein,
    NativeFountain,
    Structural,
    Pos,
    Cluster,
    Llm,
    User,
}

impl TagSource {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::NativeFdx => "native_fdx",
            Self::NativeFadein => "native_fadein",
            Self::NativeFountain => "native_fountain",
            Self::Structural => "structural",
            Self::Pos => "pos",
            Self::Cluster => "cluster",
            Self::Llm => "llm",
            Self::User => "user",
        }
    }

    pub fn parse_db_str(s: &str) -> Option<Self> {
        Some(match s {
            "native_fdx" => Self::NativeFdx,
            "native_fadein" => Self::NativeFadein,
            "native_fountain" => Self::NativeFountain,
            "structural" => Self::Structural,
            "pos" => Self::Pos,
            "cluster" => Self::Cluster,
            "llm" => Self::Llm,
            "user" => Self::User,
            _ => return None,
        })
    }
}

// ── Breakdown pipeline output ──

/// Per-scene breakdown produced by the pipeline. Plain data — callers store
/// it (or not) in whatever shape they want. `job_id` is a free-form
/// correlation string for logging across multiple calls; the original PoC
/// used it to key into its own database but library callers can ignore it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneBreakdown {
    pub job_id: String,
    pub scene_number: i64,
    pub scene_heading: String,
    pub int_ext: Option<String>,
    pub location: String,
    pub time_of_day: String,
    pub page_length: String,
    pub cast: Vec<String>,
    pub extras_background: Vec<String>,
    pub speaking_cast: Vec<String>,
    pub props: Vec<String>,
    pub wardrobe: Vec<String>,
    pub makeup_hair: Vec<String>,
    pub special_effects: Vec<String>,
    pub stunts: Vec<String>,
    pub animals: Vec<String>,
    pub sound_effects: Vec<String>,
    pub music: Vec<String>,
    pub visual_effects: Vec<String>,
    pub created_at: String,
}

// ── Ollama API types ──

#[derive(Debug, Serialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    pub stream: bool,
    pub options: OllamaOptions,
}

#[derive(Debug, Serialize)]
pub struct OllamaOptions {
    pub num_ctx: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct OllamaEmbedRequest {
    pub model: String,
    pub input: String,
}

#[derive(Debug, Deserialize)]
pub struct OllamaEmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
}
