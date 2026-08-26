//! Data models and schemas for LLM protocol plugin.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider connection and authentication configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    /// Base URL for the OpenAI compatible API (e.g. `https://api.openai.com/v1`, `https://api.deepseek.com/v1`, `http://localhost:11434/v1`).
    #[serde(default = "default_base_url")]
    pub base_url: String,

    /// API key for authentication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Target model identifier (e.g. `deepseek-chat`, `gpt-4o`, `qwen-plus`).
    #[serde(default)]
    pub model: String,

    /// Request timeout in milliseconds (default: 60000).
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Optional extra custom HTTP headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_headers: Option<HashMap<String, String>>,
}

fn default_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_timeout_ms() -> u64 {
    60_000
}

impl Default for LlmProviderConfig {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            api_key: None,
            model: "gpt-4o".to_string(),
            timeout_ms: default_timeout_ms(),
            custom_headers: None,
        }
    }
}

/// Chat message representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message role: `system`, `user`, `assistant`, `tool`.
    pub role: String,

    /// Text content of the message.
    pub content: String,

    /// Optional participant name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            name: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }
}

/// Response format specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// Format type: `text`, `json_object`, `json_schema`.
    #[serde(rename = "type")]
    pub kind: String,

    /// Structured JSON schema definition (used when `type == "json_schema"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

/// LLM inference tuning parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatParameters {
    /// Sampling temperature between 0 and 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling probability mass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Maximum tokens to generate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Whether to stream the response chunks.
    #[serde(default = "default_stream")]
    pub stream: bool,

    /// Response format requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    /// Stop sequences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

fn default_stream() -> bool {
    true
}

impl Default for ChatParameters {
    fn default() -> Self {
        Self {
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: true,
            response_format: None,
            stop: None,
        }
    }
}

/// Token usage statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub prompt_tokens: u32,

    #[serde(default)]
    pub completion_tokens: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,

    #[serde(default)]
    pub total_tokens: u32,
}

/// Latency breakdown metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LatencyMetrics {
    /// Time to first token in milliseconds (streaming only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u64>,

    /// Total execution duration in milliseconds.
    pub total_ms: u64,
}

/// Execution output for `llm:chat` and `llm:structured` nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatExecutionOutput {
    /// Model output text content.
    pub content: String,

    /// Thinking / reasoning process (e.g. DeepSeek-R1 / OpenAI reasoning models).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,

    /// Deserialized JSON payload when JSON mode is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed_json: Option<serde_json::Value>,

    /// Whether output conforms to the configured JSON schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_valid: Option<bool>,

    /// JSON schema validation errors if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_errors: Option<Vec<String>>,

    /// Token usage metrics.
    pub usage: TokenUsage,

    /// Latency metrics.
    pub latency_ms: LatencyMetrics,
}

/// Node configuration for `llm:provider`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderNodeConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    #[serde(default)]
    pub model: String,

    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_headers: Option<HashMap<String, String>>,
}

impl Default for ProviderNodeConfig {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            api_key: None,
            model: "gpt-4o".to_string(),
            timeout_ms: default_timeout_ms(),
            custom_headers: None,
        }
    }
}

impl From<ProviderNodeConfig> for LlmProviderConfig {
    fn from(p: ProviderNodeConfig) -> Self {
        Self {
            base_url: p.base_url,
            api_key: p.api_key,
            model: p.model,
            timeout_ms: p.timeout_ms,
            custom_headers: p.custom_headers,
        }
    }
}

/// Node configuration for `llm:chat`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChatNodeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_uuid: Option<String>,

    #[serde(default)]
    pub provider: LlmProviderConfig,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_timeout_ms: Option<u64>,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,

    #[serde(default)]
    pub parameters: ChatParameters,
}

/// Node configuration for `llm:structured`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StructuredNodeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_uuid: Option<String>,

    #[serde(default)]
    pub provider: LlmProviderConfig,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_timeout_ms: Option<u64>,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,

    /// Target JSON schema definition for output constraint and validation.
    #[serde(default)]
    pub json_schema: serde_json::Value,

    #[serde(default)]
    pub parameters: ChatParameters,

    /// If true, schema validation failure causes node execution failure (routing to false port).
    #[serde(default = "default_true")]
    pub strict_validation: bool,
}

fn default_true() -> bool {
    true
}

/// Node configuration for `llm:embeddings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EmbeddingsNodeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_uuid: Option<String>,

    #[serde(default)]
    pub provider: LlmProviderConfig,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_timeout_ms: Option<u64>,
    /// Input text or array of texts to embed.
    #[serde(default)]
    pub input: serde_json::Value,

    /// Optional target dimensions (for supported models).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,

    /// Optional end-user identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Item in embeddings output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingItem {
    pub index: usize,
    pub embedding: Vec<f32>,
}

/// Execution output for `llm:embeddings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingsOutput {
    pub data: Vec<Vec<f32>>,
    pub usage: TokenUsage,
    pub latency_ms: LatencyMetrics,
}

/// Node configuration for `llm:rerank`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RerankNodeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_uuid: Option<String>,

    #[serde(default)]
    pub provider: LlmProviderConfig,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_timeout_ms: Option<u64>,
    /// The query to rank documents against.
    #[serde(default)]
    pub query: String,

    /// Candidate documents to rank.
    #[serde(default)]
    pub documents: Vec<String>,

    /// Top N documents to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_n: Option<usize>,

    /// Whether to return document text in results.
    #[serde(default = "default_true")]
    pub return_documents: bool,
}

/// Ranked document item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RerankItem {
    pub index: usize,
    pub relevance_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
}

/// Execution output for `llm:rerank`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RerankOutput {
    pub results: Vec<RerankItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    pub latency_ms: LatencyMetrics,
}

/// Resolves the effective provider config given a pool, node instance uuid / provider_uuid,
/// inline config, and optional model override.
pub fn resolve_effective_provider(
    pool: &mpe_plugin_sdk::pool::ConnectionPool,
    exec_id: &str,
    provider_uuid: Option<&str>,
    inline_provider: &LlmProviderConfig,
    override_model: Option<&str>,
    override_timeout_ms: Option<u64>,
) -> Result<LlmProviderConfig, String> {
    let mut provider = if let Some(uuid) = provider_uuid.filter(|s| !s.trim().is_empty()) {
        let key = format!("{exec_id}:{uuid}");
        if let Some(pooled) = pool.get::<LlmProviderConfig>(&key) {
            (*pooled).clone()
        } else if let Some(pooled) = pool.get::<LlmProviderConfig>(uuid) {
            (*pooled).clone()
        } else if !inline_provider.base_url.trim().is_empty() && (!inline_provider.model.trim().is_empty() || override_model.is_some()) {
            inline_provider.clone()
        } else {
            return Err(crate::i18n::t(
                "未找到指定的 LLM 服务商配置，请确保在执行前添加并运行了 LLM Provider 节点，或配置内联服务商",
                "Referenced LLM Provider not found; ensure an LLM Provider node was executed or configure inline provider",
            ).to_string());
        }
    } else {
        inline_provider.clone()
    };

    if let Some(m) = override_model.filter(|s| !s.trim().is_empty()) {
        provider.model = m.to_string();
    }

    if let Some(t) = override_timeout_ms.filter(|&t| t > 0) {
        provider.timeout_ms = t;
    }

    if provider.base_url.trim().is_empty() {
        return Err(crate::i18n::t("缺少服务商 Base URL", "Missing provider Base URL").to_string());
    }

    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpe_plugin_sdk::pool::ConnectionPool;

    #[test]
    fn test_resolve_effective_provider_pool_inheritance_and_override() {
        let pool = ConnectionPool::new();
        let pooled_provider = LlmProviderConfig {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: Some("sk-test".to_string()),
            model: "gpt-4o".to_string(),
            timeout_ms: 45000,
            custom_headers: None,
        };
        let pooled_provider_clone = pooled_provider.clone();
        pool.get_or_insert("exec1:node-prov-1", move || pooled_provider_clone);

        let inline_empty = LlmProviderConfig::default();
        // 1. Inherit timeout_ms from pooled provider
        let resolved = resolve_effective_provider(
            &pool,
            "exec1",
            Some("node-prov-1"),
            &inline_empty,
            None,
            None,
        ).expect("should resolve pooled provider");
        assert_eq!(resolved.timeout_ms, 45000);
        assert_eq!(resolved.model, "gpt-4o");

        // 2. Override model and timeout_ms over pooled provider
        let resolved_override = resolve_effective_provider(
            &pool,
            "exec1",
            Some("node-prov-1"),
            &inline_empty,
            Some("deepseek-chat"),
            Some(120000),
        ).expect("should resolve with override");
        assert_eq!(resolved_override.timeout_ms, 120000);
        assert_eq!(resolved_override.model, "deepseek-chat");

        // 3. Inline provider with override
        let inline_custom = LlmProviderConfig {
            base_url: "https://api.deepseek.com/v1".to_string(),
            api_key: None,
            model: "deepseek-reasoner".to_string(),
            timeout_ms: 30000,
            custom_headers: None,
        };
        let resolved_inline = resolve_effective_provider(
            &pool,
            "exec1",
            None,
            &inline_custom,
            None,
            Some(90000),
        ).expect("should resolve inline provider");
        assert_eq!(resolved_inline.timeout_ms, 90000);
        assert_eq!(resolved_inline.model, "deepseek-reasoner");
    }
}
