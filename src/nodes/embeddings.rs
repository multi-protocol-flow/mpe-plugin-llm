//! Executor for `llm:embeddings` node.

use mpe_plugin_sdk::prelude::*;
use serde_json::json;
use std::time::Instant;

use crate::client::{build_client, build_request_summary, join_endpoint};
use crate::types::{EmbeddingsNodeConfig, EmbeddingsOutput, LatencyMetrics, TokenUsage};
/// Executes the `llm:embeddings` node.
pub async fn execute_embeddings(
    ctx: &mut ExecuteContext,
    pool: &std::sync::Arc<mpe_plugin_sdk::pool::ConnectionPool>,
) -> ExecuteResult {
    let config_val = ctx.config().clone();
    let config: EmbeddingsNodeConfig = match serde_json::from_value(config_val) {
        Ok(cfg) => cfg,
        Err(err) => return ExecuteResult::fail(format!("Invalid llm:embeddings config: {err}")),
    };

    let exec_id = ctx.execution_id().unwrap_or("default");
    let provider = match crate::types::resolve_effective_provider(
        pool,
        exec_id,
        config.provider_uuid.as_deref(),
        &config.provider,
        config.override_model.as_deref(),
    ) {
        Ok(p) => p,
        Err(err) => return ExecuteResult::fail(err),
    };

    if provider.model.trim().is_empty() {
        return ExecuteResult::fail("Model name is required for llm:embeddings");
    }

    let is_empty_input = match &config.input {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.trim().is_empty(),
        serde_json::Value::Array(a) => a.is_empty(),
        _ => false,
    };

    if is_empty_input {
        return ExecuteResult::fail("Input text or array is required and cannot be empty for llm:embeddings");
    }

    let client = match build_client(&provider) {
        Ok(c) => c,
        Err(err) => return ExecuteResult::fail(err),
    };

    let url = join_endpoint(&provider.base_url, "embeddings");

    let mut body = serde_json::Map::new();
    body.insert("model".into(), json!(provider.model));
    body.insert("input".into(), config.input.clone());

    if let Some(dim) = config.dimensions {
        body.insert("dimensions".into(), json!(dim));
    }
    if let Some(user) = &config.user {
        body.insert("user".into(), json!(user));
    }

    let body_val = serde_json::Value::Object(body.clone());
    let request_summary = build_request_summary(
        &url,
        &provider.model,
        provider.api_key.as_deref(),
        &body_val,
    );

    let start_time = Instant::now();

    let res = match client.post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(err) => {
            let total_ms = start_time.elapsed().as_millis() as u64;
            let error_msg = format!("HTTP request failed: {err}");
            let err_output = json!({
                "error": error_msg,
                "request": request_summary,
                "timing": { "total_ms": total_ms }
            });
            return ExecuteResult {
                success: false,
                next_ports: vec!["false".into()],
                output_data: Some(err_output.clone()),
                errors: vec![error_msg],
                report_data: Some(err_output),
            };
        }
    };

    let status = res.status();
    let total_ms = start_time.elapsed().as_millis() as u64;

    if !status.is_success() {
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".into());
        let parsed_err: serde_json::Value =
            serde_json::from_str(&err_text).unwrap_or_else(|_| json!(err_text));
        let error_msg = format!("API error (status {status}): {err_text}");
        ctx.log("error", &error_msg);
        let err_output = json!({
            "error": error_msg,
            "request": request_summary,
            "response": {
                "status": status.as_u16(),
                "body": parsed_err,
            },
            "timing": { "total_ms": total_ms }
        });
        return ExecuteResult {
            success: false,
            next_ports: vec!["false".into()],
            output_data: Some(err_output.clone()),
            errors: vec![error_msg],
            report_data: Some(err_output),
        };
    }

    let res_json: serde_json::Value = match res.json().await {
        Ok(v) => v,
        Err(err) => return ExecuteResult::fail(format!("Failed to parse response JSON: {err}")),
    };
    let mut embeddings = Vec::new();
    if let Some(data_arr) = res_json.get("data").and_then(|d| d.as_array()) {
        for item in data_arr {
            if let Some(vec) = item.get("embedding").and_then(|v| v.as_array()) {
                let f_vec: Vec<f32> = vec
                    .iter()
                    .filter_map(|n| n.as_f64().map(|f| f as f32))
                    .collect();
                embeddings.push(f_vec);
            }
        }
    }

    let mut usage = TokenUsage::default();
    if let Some(u) = res_json.get("usage") {
        if let Ok(parsed_u) = serde_json::from_value::<TokenUsage>(u.clone()) {
            usage = parsed_u;
        }
    }

    let output = EmbeddingsOutput {
        data: embeddings,
        usage,
        latency_ms: LatencyMetrics {
            ttft_ms: None,
            total_ms,
        },
    };

    let mut output_val = match serde_json::to_value(&output) {
        Ok(v) => v,
        Err(err) => return ExecuteResult::fail(format!("Serialization error: {err}")),
    };

    if let Some(obj) = output_val.as_object_mut() {
        obj.insert("request".into(), request_summary);
    }

    let mut res = ExecuteResult::ok(output_val.clone());
    res.report_data = Some(output_val);
    res
}
