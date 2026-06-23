//! LLM client with pluggable backends (Ollama and Anthropic).
//!
//! Both backends stream responses token-by-token, printing to stdout as they
//! arrive so you can watch the LLM think in the terminal. The full response
//! is accumulated and returned as a single string.
//!
//! Embeddings always go through Ollama (nomic-embed-text by default) since
//! Anthropic doesn't have an embedding API. Input text is truncated to
//! [`EMBED_CHAR_LIMIT`] characters to stay within the embedding model's
//! context window.

use crate::aristotle::config::{Config, LlmProvider};
use crate::aristotle::models::{
    OllamaChatRequest, OllamaEmbedRequest, OllamaEmbedResponse, OllamaMessage, OllamaOptions,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::Write;
use tracing::info;

/// nomic-embed-text has an 8192-token context window. At ~1.3 chars/token
/// for English, 6000 chars keeps us comfortably under.
const EMBED_CHAR_LIMIT: usize = 6000;

#[derive(Clone)]
pub struct LlmClient {
    http: reqwest::Client,
    provider: LlmProvider,
    ollama_url: String,
    ollama_model: String,
    embed_model: String,
    num_ctx: usize,
    anthropic_key: Option<String>,
    anthropic_model: String,
    anthropic_max_tokens: usize,
}

// ── Ollama streaming types ──

#[derive(Debug, Deserialize)]
struct OllamaStreamChunk {
    message: Option<OllamaStreamMessage>,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamMessage {
    content: String,
}

// ── Anthropic API types ──

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: usize,
    system: String,
    messages: Vec<AnthropicMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    text: Option<String>,
}

impl LlmClient {
    /// Build from app config. Copies out the fields it needs so the
    /// config doesn't need to live as long as the client.
    pub fn new(cfg: &Config) -> Self {
        Self {
            http: reqwest::Client::new(),
            provider: cfg.llm_provider.clone(),
            ollama_url: cfg.ollama_url.clone(),
            ollama_model: cfg.ollama_model.clone(),
            embed_model: cfg.ollama_embed_model.clone(),
            num_ctx: cfg.ollama_num_ctx,
            anthropic_key: cfg.anthropic_api_key.clone(),
            anthropic_model: cfg.anthropic_model.clone(),
            anthropic_max_tokens: cfg.anthropic_max_tokens,
        }
    }

    /// Send a system + user message and get back the full response text.
    /// Routes to Ollama or Anthropic based on `LLM_PROVIDER` config.
    /// Tokens stream to stdout in real time.
    pub async fn chat(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        match self.provider {
            LlmProvider::Ollama => self.chat_ollama(system_prompt, user_prompt).await,
            LlmProvider::Anthropic => self.chat_anthropic(system_prompt, user_prompt).await,
        }
    }

    /// Generate a 768-dimensional embedding vector via Ollama.
    /// Always uses the Ollama embed model regardless of the chat provider.
    /// Input is truncated to [`EMBED_CHAR_LIMIT`] chars.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, reqwest::Error> {
        let truncated: String = text.chars().take(EMBED_CHAR_LIMIT).collect();
        let req = OllamaEmbedRequest {
            model: self.embed_model.clone(),
            input: truncated,
        };

        self.http
            .post(format!("{}/api/embed", self.ollama_url))
            .json(&req)
            .send()
            .await?
            .json::<OllamaEmbedResponse>()
            .await
            .map(|resp| resp.embeddings.into_iter().next().unwrap_or_default())
    }

    // ── Ollama backend ──

    /// Rough estimate of how many tokens the prompts need, rounded up to
    /// the nearest 4096 with 30% headroom for output. Keeps us from always
    /// allocating the full 65k context when a scene only needs 8k.
    fn estimate_ctx(&self, system_prompt: &str, user_prompt: &str) -> usize {
        let input_tokens = (system_prompt.len() + user_prompt.len()) / 4;
        let needed = ((input_tokens as f64 * 1.3) as usize).next_multiple_of(4096);
        needed.clamp(8192, self.num_ctx)
    }

    async fn chat_ollama(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let num_ctx = self.estimate_ctx(system_prompt, user_prompt);
        let req = OllamaChatRequest {
            model: self.ollama_model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".into(),
                    content: system_prompt.into(),
                },
                OllamaMessage {
                    role: "user".into(),
                    content: user_prompt.into(),
                },
            ],
            stream: true,
            options: OllamaOptions { num_ctx },
        };

        info!(provider = "ollama", model = %self.ollama_model, num_ctx, "streaming LLM response");

        let resp = self
            .http
            .post(format!("{}/api/chat", self.ollama_url))
            .json(&req)
            .send()
            .await?;

        let mut full_response = String::new();
        let mut stream = resp.bytes_stream();

        while let Some(bytes) = stream.next().await {
            let bytes = bytes?;
            String::from_utf8_lossy(&bytes)
                .lines()
                .filter(|line| !line.is_empty())
                .filter_map(|line| serde_json::from_str::<OllamaStreamChunk>(line).ok())
                .for_each(|chunk| {
                    if let Some(msg) = &chunk.message {
                        print!("{}", msg.content);
                        std::io::stdout().flush().ok();
                        full_response.push_str(&msg.content);
                    }
                    if chunk.done {
                        println!();
                    }
                });
        }

        Ok(full_response)
    }

    // ── Anthropic backend ──

    async fn chat_anthropic(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let api_key = self
            .anthropic_key
            .as_deref()
            .ok_or("ANTHROPIC_API_KEY not set")?;

        let req = AnthropicRequest {
            model: self.anthropic_model.clone(),
            max_tokens: self.anthropic_max_tokens,
            system: system_prompt.to_string(),
            messages: vec![AnthropicMessage {
                role: "user".into(),
                content: user_prompt.into(),
            }],
            stream: true,
        };

        info!(provider = "anthropic", model = %self.anthropic_model, "streaming LLM response");

        let resp = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Anthropic API error {status}: {body}").into());
        }

        let mut full_response = String::new();
        let mut stream = resp.bytes_stream();

        while let Some(bytes) = stream.next().await {
            let bytes = bytes?;
            for line in String::from_utf8_lossy(&bytes).lines() {
                let Some(json) = line.trim().strip_prefix("data: ") else {
                    continue;
                };
                if json == "[DONE]" {
                    break;
                }
                let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(json) else {
                    continue;
                };

                if let Some(text) = event.delta.as_ref().and_then(|d| d.text.as_ref())
                    && event.event_type == "content_block_delta"
                {
                    print!("{text}");
                    std::io::stdout().flush().ok();
                    full_response.push_str(text);
                }
            }
        }

        println!();
        Ok(full_response)
    }
}
