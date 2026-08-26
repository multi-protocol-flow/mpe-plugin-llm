//! LLM sidecar plugin for MPE (Multi-Protocol Flow Executor).
//!
//! Provides OpenAI-compatible nodes:
//! - `llm:chat`: Streaming chat completion with reasoning/thinking capture and metrics.
//! - `llm:structured`: Structured output with strict JSON schema constraints and Rust-side validation.
//! - `llm:embeddings`: Vector embeddings generation.
//! - `llm:rerank`: Cross-encoder document reranking.

use mpe_plugin_sdk::prelude::*;
use std::future::Future;

pub mod client;
pub mod i18n;
pub mod nodes;
pub mod types;
pub mod ui;

use std::sync::Arc;
use mpe_plugin_sdk::pool::ConnectionPool;

/// LLM Plugin instance.
pub struct LlmPlugin {
    pool: Arc<ConnectionPool>,
}

impl Default for LlmPlugin {
    fn default() -> Self {
        Self {
            pool: Arc::new(ConnectionPool::new()),
        }
    }
}
fn operation_ports() -> Vec<PortDescription> {
    vec![
        PortDescription::new("in", i18n::t("输入", "Input"), PORT_KIND_IN),
        PortDescription::new("true", i18n::t("成功", "Success"), PORT_KIND_OUT),
        PortDescription::new("false", i18n::t("失败", "Failure"), PORT_KIND_OUT),
    ]
}

fn llm_node(
    type_id: &str,
    display_name: &str,
    icon: &str,
    default_config: serde_json::Value,
    properties: serde_json::Value,
    required: &[&str],
) -> NodeDescription {
    let mut node = NodeDescription::new(type_id, display_name);
    node.category = Some("ai".to_string());
    node.icon = Some(icon.to_string());
    node.color = Some("#10B981".to_string());
    node.ports = operation_ports();
    node.default_config = default_config;
    node.capabilities = PluginCapabilities {
        streaming: true,
        single_node: true,
    };
    node.config_schema = Some(serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    }));
    node.frontend = Some(FrontendDescription {
        kind: "inline".into(),
        content: Some(include_str!("../frontend/dist/panel.html").to_string()),
        url: None,
    });
    node.viewer = Some(FrontendDescription {
        kind: "inline".into(),
        content: Some(include_str!("../frontend/dist/viewer.html").to_string()),
        url: None,
    });
    node
}

impl Plugin for LlmPlugin {
    fn describe(&self) -> Vec<NodeDescription> {
        vec![
            // 0. `llm:provider`
            llm_node(
                "llm:provider",
                i18n::t("LLM 服务商", "LLM Provider"),
                "bot",
                serde_json::json!({
                    "base_url": "https://api.openai.com/v1",
                    "api_key": "",
                    "model": "gpt-4o",
                    "timeout_ms": 60000
                }),
                serde_json::json!({
                    "base_url": { "type": "string", "title": i18n::t("基础地址", "Base URL") },
                    "api_key": { "type": "string", "title": i18n::t("API 密钥", "API Key") },
                    "model": { "type": "string", "title": i18n::t("模型", "Model") },
                    "timeout_ms": { "type": "integer", "title": i18n::t("超时(毫秒)", "Timeout (ms)") },
                    "custom_headers": { "type": "object", "title": i18n::t("自定义请求头", "Custom Headers") }
                }),
                &["base_url", "model"],
            ),
            // 1. `llm:chat`
            llm_node(
                "llm:chat",
                i18n::t("LLM 对话", "LLM Chat"),
                "message-square",
                serde_json::json!({
                    "provider_uuid": "",
                    "provider": {
                        "base_url": "https://api.openai.com/v1",
                        "api_key": "",
                        "model": "gpt-4o",
                        "timeout_ms": 60000
                    },
                    "messages": [
                        { "role": "system", "content": "You are a helpful assistant." },
                        { "role": "user", "content": "" }
                    ],
                    "parameters": {
                        "temperature": 0.7,
                        "stream": true
                    }
                }),
                serde_json::json!({
                    "provider_uuid": { "type": "string", "title": i18n::t("服务商节点", "Provider Node") },
                    "provider": { "type": "object" },
                    "override_model": { "type": "string" },
                    "override_timeout_ms": { "type": "integer", "title": i18n::t("覆盖超时(毫秒)", "Override Timeout (ms)") },
                    "messages": { "type": "array" },
                    "parameters": { "type": "object" }
                }),
                &["messages"],
            ),
            // 2. `llm:structured`
            llm_node(
                "llm:structured",
                i18n::t("LLM 结构化提取", "LLM Structured"),
                "braces",
                serde_json::json!({
                    "provider_uuid": "",
                    "provider": {
                        "base_url": "https://api.openai.com/v1",
                        "api_key": "",
                        "model": "gpt-4o",
                        "timeout_ms": 60000
                    },
                    "messages": [
                        { "role": "system", "content": "Extract structured data from user input." },
                        { "role": "user", "content": "" }
                    ],
                    "json_schema": {
                        "type": "object",
                        "properties": {
                            "summary": { "type": "string" }
                        },
                        "required": ["summary"]
                    },
                    "parameters": {
                        "temperature": 0.2,
                        "stream": true
                    },
                    "strict_validation": true
                }),
                serde_json::json!({
                    "provider_uuid": { "type": "string", "title": i18n::t("服务商节点", "Provider Node") },
                    "provider": { "type": "object" },
                    "override_model": { "type": "string" },
                    "override_timeout_ms": { "type": "integer", "title": i18n::t("覆盖超时(毫秒)", "Override Timeout (ms)") },
                    "messages": { "type": "array" },
                    "json_schema": { "type": "object" },
                    "parameters": { "type": "object" },
                    "strict_validation": { "type": "boolean" }
                }),
                &["messages", "json_schema"],
            ),
            // 3. `llm:embeddings`
            llm_node(
                "llm:embeddings",
                i18n::t("LLM 向量嵌入", "LLM Embeddings"),
                "layers",
                serde_json::json!({
                    "provider_uuid": "",
                    "provider": {
                        "base_url": "https://api.openai.com/v1",
                        "api_key": "",
                        "model": "text-embedding-3-small",
                        "timeout_ms": 60000
                    },
                    "input": ""
                }),
                serde_json::json!({
                    "provider_uuid": { "type": "string", "title": i18n::t("服务商节点", "Provider Node") },
                    "provider": { "type": "object" },
                    "override_model": { "type": "string" },
                    "override_timeout_ms": { "type": "integer", "title": i18n::t("覆盖超时(毫秒)", "Override Timeout (ms)") },
                    "input": { "type": ["string", "array"] }
                }),
                &["input"],
            ),
            // 4. `llm:rerank`
            llm_node(
                "llm:rerank",
                i18n::t("LLM 文档重排", "LLM Rerank"),
                "shuffle",
                serde_json::json!({
                    "provider_uuid": "",
                    "provider": {
                        "base_url": "https://api.siliconflow.cn/v1",
                        "api_key": "",
                        "model": "BAAI/bge-reranker-v2-m3",
                        "timeout_ms": 60000
                    },
                    "query": "",
                    "documents": [],
                    "top_n": 3,
                    "return_documents": true
                }),
                serde_json::json!({
                    "provider_uuid": { "type": "string", "title": i18n::t("服务商节点", "Provider Node") },
                    "provider": { "type": "object" },
                    "override_model": { "type": "string" },
                    "override_timeout_ms": { "type": "integer", "title": i18n::t("覆盖超时(毫秒)", "Override Timeout (ms)") },
                    "query": { "type": "string" },
                    "documents": { "type": "array" },
                    "top_n": { "type": "integer" },
                    "return_documents": { "type": "boolean" }
                }),
                &["query", "documents"],
            ),
        ]
    }

    fn execute(&self, ctx: &mut ExecuteContext) -> impl Future<Output = ExecuteResult> + Send {
        let config = ctx.config().clone();
        let node_type = config
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| {
                if config.get("base_url").is_some()
                    && config.get("messages").is_none()
                    && config.get("input").is_none()
                    && config.get("query").is_none()
                {
                    "llm:provider"
                } else if config.get("json_schema").is_some() {
                    "llm:structured"
                } else if config.get("query").is_some() && config.get("documents").is_some() {
                    "llm:rerank"
                } else if config.get("input").is_some() && config.get("messages").is_none() {
                    "llm:embeddings"
                } else {
                    "llm:chat"
                }
            })
            .to_string();

        let pool = self.pool.clone();
        async move {
            match node_type.as_str() {
                "llm:provider" => nodes::execute_provider(ctx, &pool).await,
                "llm:chat" => nodes::execute_chat(ctx, &pool).await,
                "llm:structured" => nodes::execute_structured(ctx, &pool).await,
                "llm:embeddings" => nodes::execute_embeddings(ctx, &pool).await,
                "llm:rerank" => nodes::execute_rerank(ctx, &pool).await,
                other => ExecuteResult::fail(format!("Unsupported LLM node type: {other}")),
            }
        }
    }
    fn ui_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, String>> + Send {
        let method = method.to_string();
        async move { ui::handle_ui_call(&method, params).await }
    }
}
