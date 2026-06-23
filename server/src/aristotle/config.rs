//! App configuration, loaded entirely from environment variables.
//!
//! Every field has a sensible default so the service can start with just
//! a running SurrealDB and Ollama instance. See `.env.example` for the
//! full list of knobs.

use crate::aristotle::breakdown::Policy as BreakdownPolicy;
use std::env;

/// Which backend to use for LLM chat completions.
/// Embeddings always go through Ollama regardless of this setting.
#[derive(Debug, Clone, PartialEq)]
pub enum LlmProvider {
    Ollama,
    Anthropic,
}

/// All runtime configuration. Built once at startup via [`Config::from_env`].
#[derive(Debug, Clone)]
pub struct Config {
    pub llm_provider: LlmProvider,
    pub ollama_url: String,
    pub ollama_model: String,
    pub ollama_embed_model: String,
    pub ollama_num_ctx: usize,
    pub anthropic_api_key: Option<String>,
    pub anthropic_model: String,
    pub anthropic_max_tokens: usize,
    pub surreal_url: String,
    pub surreal_user: String,
    pub surreal_pass: String,
    pub surreal_ns: String,
    pub surreal_db: String,
    pub max_concurrent: usize,
    pub listen_addr: String,
    pub embed_dimension: usize,
    /// Target pages per shoot day. Default 5.0 (indie pace).
    /// Studio films typically target ~1 page/day.
    pub daily_page_target: f64,
    /// How aggressively the breakdown pipeline calls the LLM. Default is
    /// `DeterministicOnly` — produce the breakdown from parser + structural
    /// rules with no LLM in the loop.
    pub breakdown_policy: BreakdownPolicy,
}

impl Config {
    /// Read all config from the environment. Missing vars fall back to defaults.
    /// Call after `dotenvy::dotenv()` so `.env` values are available.
    pub fn from_env() -> Self {
        let anthropic_api_key = env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty());

        let llm_provider = match env::var("LLM_PROVIDER")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "anthropic" | "claude" => LlmProvider::Anthropic,
            _ => LlmProvider::Ollama,
        };

        Self {
            llm_provider,
            ollama_url: env_or("OLLAMA_URL", "http://localhost:11434"),
            ollama_model: env_or("OLLAMA_MODEL", "qwen3.5:27b"),
            ollama_embed_model: env_or("OLLAMA_EMBED_MODEL", "nomic-embed-text"),
            ollama_num_ctx: env_parse("OLLAMA_NUM_CTX", 65536),
            anthropic_api_key,
            anthropic_model: env_or("ANTHROPIC_MODEL", "claude-sonnet-4-6"),
            anthropic_max_tokens: env_parse("ANTHROPIC_MAX_TOKENS", 8192),
            surreal_url: env_or("SURREAL_URL", "ws://localhost:8000"),
            surreal_user: env_or("SURREAL_USER", "root"),
            surreal_pass: env_or("SURREAL_PASS", "root"),
            surreal_ns: env_or("SURREAL_NS", "aristotle"),
            surreal_db: env_or("SURREAL_DB", "aristotle"),
            max_concurrent: env_parse("MAX_CONCURRENT", 2),
            listen_addr: env_or("LISTEN_ADDR", "0.0.0.0:3000"),
            embed_dimension: env_parse("EMBED_DIMENSION", 768),
            daily_page_target: env_parse("DAILY_PAGE_TARGET", 5.0),
            breakdown_policy: BreakdownPolicy::parse(
                &env::var("BREAKDOWN_POLICY").unwrap_or_default(),
            ),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.into())
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
