//! Executor for `llm:provider` node.

use std::sync::Arc;
use std::time::Instant;
use mpe_plugin_sdk::pool::ConnectionPool;
use mpe_plugin_sdk::prelude::*;
use serde_json::json;

use crate::client::build_client;
use crate::types::{LlmProviderConfig, ProviderNodeConfig};

/// Executes the `llm:provider` node.
pub async fn execute_provider(ctx: &mut ExecuteContext, pool: &Arc<ConnectionPool>) -> ExecuteResult {
    let config_val = ctx.config().clone();
    let config: ProviderNodeConfig = match serde_json::from_value(config_val) {
        Ok(cfg) => cfg,
        Err(err) => return ExecuteResult::fail(format!("Invalid llm:provider config: {err}")),
    };

    let exec_id = ctx.execution_id().unwrap_or("default");
    let instance_id = ctx.node_instance_id().unwrap_or("default");

    let provider: LlmProviderConfig = config.into();

    let start = Instant::now();

    // Validate client can be built
    if let Err(err) = build_client(&provider) {
        return ExecuteResult::fail(err);
    }

    // Store in pool for subsequent nodes in this flow execution
    let key = format!("{exec_id}:{instance_id}");
    pool.get_or_insert(key, || provider.clone());
    pool.get_or_insert(instance_id.to_string(), || provider.clone());

    let latency_ms = start.elapsed().as_millis() as u64;

    ctx.log(
        "info",
        format!(
            "{}: {} (model: {})",
            crate::i18n::t("LLM 服务商已注册", "LLM Provider registered"),
            provider.base_url,
            provider.model
        ),
    );
    let request_summary = crate::client::build_request_summary(
        &provider.base_url,
        &provider.model,
        provider.api_key.as_deref(),
        &serde_json::to_value(&provider).unwrap_or_default(),
    );

    let output = json!({
        "connected": true,
        "provider_uuid": instance_id,
        "base_url": provider.base_url,
        "model": provider.model,
        "latency_ms": latency_ms,
        "request": request_summary,
    });
    let mut res = ExecuteResult::ok(output.clone());
    res.report_data = Some(output);
    res
}
