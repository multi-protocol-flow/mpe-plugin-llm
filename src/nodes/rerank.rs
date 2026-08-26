//! Executor for `llm:rerank` node.

use mpe_plugin_sdk::prelude::*;
use serde_json::json;
use std::time::Instant;

use crate::client::{build_client, build_request_summary, join_endpoint};
use crate::types::{LatencyMetrics, RerankItem, RerankNodeConfig, RerankOutput, TokenUsage};
/// Executes the `llm:rerank` node.
pub async fn execute_rerank(
    ctx: &mut ExecuteContext,
    pool: &std::sync::Arc<mpe_plugin_sdk::pool::ConnectionPool>,
) -> ExecuteResult {
    let config_val = ctx.config().clone();
    let config: RerankNodeConfig = match serde_json::from_value(config_val) {
        Ok(cfg) => cfg,
        Err(err) => return ExecuteResult::fail(format!("Invalid llm:rerank config: {err}")),
    };

    let exec_id = ctx.execution_id().unwrap_or("default");
    let provider = match crate::types::resolve_effective_provider(
        pool,
        exec_id,
        config.provider_uuid.as_deref(),
        &config.provider,
        config.override_model.as_deref(),
        config.override_timeout_ms,
    ) {
        Ok(p) => p,
        Err(err) => return ExecuteResult::fail(err),
    };

    if provider.model.trim().is_empty() {
        return ExecuteResult::fail("Model name is required for llm:rerank");
    }

    if config.query.trim().is_empty() {
        return ExecuteResult::fail("Query string is required for llm:rerank");
    }

    if config.documents.is_empty() {
        return ExecuteResult::fail("Documents list cannot be empty for llm:rerank");
    }

    let client = match build_client(&provider) {
        Ok(c) => c,
        Err(err) => return ExecuteResult::fail(err),
    };

    let url = join_endpoint(&provider.base_url, "rerank");

    let mut body = serde_json::Map::new();
    body.insert("model".into(), json!(provider.model));
    body.insert("query".into(), json!(config.query));
    body.insert("documents".into(), json!(config.documents));

    if let Some(top_n) = config.top_n {
        body.insert("top_n".into(), json!(top_n));
    }
    body.insert("return_documents".into(), json!(config.return_documents));

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

    let total_ms = start_time.elapsed().as_millis() as u64;

    let mut results = Vec::new();
    if let Some(res_arr) = res_json.get("results").and_then(|r| r.as_array()) {
        for item in res_arr {
            let index = item.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
            let score = item
                .get("relevance_score")
                .or_else(|| item.get("score"))
                .and_then(|s| s.as_f64())
                .unwrap_or(0.0) as f32;

            let doc_text = if config.return_documents {
                if let Some(doc_obj) = item.get("document") {
                    doc_obj
                        .get("text")
                        .and_then(|t| t.as_str())
                        .or_else(|| doc_obj.as_str())
                        .map(|s| s.to_string())
                } else if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    Some(text.to_string())
                } else if index < config.documents.len() {
                    Some(config.documents[index].clone())
                } else {
                    None
                }
            } else {
                None
            };

            results.push(RerankItem {
                index,
                relevance_score: score,
                document: doc_text,
            });
        }
    }

    let usage = res_json
        .get("usage")
        .and_then(|u| serde_json::from_value::<TokenUsage>(u.clone()).ok());

    let output = RerankOutput {
        results,
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
