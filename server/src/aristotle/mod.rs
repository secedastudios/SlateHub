//! Aristotle — screenplay breakdown module.
//!
//! Parse screenplays (PDF, Fountain, FDX, Fade In), run a deterministic
//! breakdown pipeline (with optional LLM-assisted classification), and
//! get structured output plus a shooting schedule.
//!
//! Originally a standalone Axum + SurrealDB service, then a workspace
//! crate, now folded inline as a module of the slatehub crate. The
//! self-contained boundary is preserved so this module can be extracted
//! back into its own crate later if needed — internal references all go
//! through `crate::aristotle::*`.
//!
//! # Usage
//!
//! ```text
//! use crate::aristotle::{breakdown, parser};
//!
//! let bytes = std::fs::read("script.fountain")?;
//! let mut parsed = parser::parse_screenplay("script.fountain", &bytes)?;
//! let context = breakdown::Context::from_scenes(&parsed.scenes);
//!
//! for scene in parsed.scenes.iter_mut() {
//!     let _bd = breakdown::run(
//!         "job-id",
//!         scene,
//!         &context,
//!         breakdown::Policy::DeterministicOnly,
//!     );
//!     // scene.elements now carry tier 0–2 tags.
//! }
//! ```

pub mod breakdown;
pub mod config;
pub mod llm;
pub mod models;
pub mod parser;

// ── Top-level re-exports — the canonical public surface ──────────────────
// Callers can do `aristotle::parse_screenplay(...)`, `aristotle::Policy`,
// `aristotle::ParsedScript`, etc. without poking into submodule paths.

pub use breakdown::{Context as BreakdownContext, Policy as BreakdownPolicy, run as run_breakdown};
pub use config::{Config, LlmProvider};
pub use llm::LlmClient;
pub use models::{
    ElementKind, OllamaChatRequest, OllamaEmbedRequest, OllamaEmbedResponse, OllamaMessage,
    OllamaOptions, ParsedScene, ParsedScript, SceneBreakdown, ScreenplayElement, ScriptMetadata,
    Tag, TagSource,
};
pub use parser::{ParseError, parse_screenplay};
