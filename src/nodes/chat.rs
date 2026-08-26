//! Executor for `llm:chat` node.

use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use mpe_plugin_sdk::prelude::*;
use serde_json::json;
use std::time::Instant;

use crate::client::{build_client, build_request_summary, join_endpoint};
use crate::types::{ChatExecutionOutput, ChatNodeConfig, LatencyMetrics, TokenUsage};

/// Executes the `llm:chat` node logic.
pub async fn execute_chat(
    ctx: &mut ExecuteContext,
    pool: &std::sync::Arc<mpe_plugin_sdk::pool::ConnectionPool>,
) -> ExecuteResult {
    let config_val = ctx.config().clone();
    let config: ChatNodeConfig = match serde_json::from_value(config_val) {
        Ok(cfg) => cfg,
        Err(err) => return ExecuteResult::fail(format!("Invalid llm:chat config: {err}")),
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
        return ExecuteResult::fail("Model name is required for llm:chat");
    }

    let client = match build_client(&provider) {
        Ok(c) => c,
        Err(err) => return ExecuteResult::fail(err),
    };

    let url = join_endpoint(&provider.base_url, "chat/completions");
    // Construct request body
    let mut messages_json = Vec::new();
    for msg in &config.messages {
        let mut obj = serde_json::Map::new();
        obj.insert("role".into(), json!(msg.role));
        obj.insert("content".into(), json!(msg.content));
        if let Some(name) = &msg.name {
            obj.insert("name".into(), json!(name));
        }
        messages_json.push(serde_json::Value::Object(obj));
    }

    let mut body = serde_json::Map::new();
    body.insert("model".into(), json!(provider.model));
    body.insert("messages".into(), json!(messages_json));

    if let Some(temp) = config.parameters.temperature {
        body.insert("temperature".into(), json!(temp));
    }
    if let Some(top_p) = config.parameters.top_p {
        body.insert("top_p".into(), json!(top_p));
    }
    if let Some(max_tokens) = config.parameters.max_tokens {
        body.insert("max_tokens".into(), json!(max_tokens));
    }
    if let Some(stop) = &config.parameters.stop {
        body.insert("stop".into(), json!(stop));
    }
    if let Some(rf) = &config.parameters.response_format {
        let mut rf_obj = serde_json::Map::new();
        rf_obj.insert("type".into(), json!(rf.kind));
        if let Some(schema) = &rf.json_schema {
            rf_obj.insert("json_schema".into(), schema.clone());
        }
        body.insert("response_format".into(), serde_json::Value::Object(rf_obj));
    }

    let is_streaming = config.parameters.stream;
    body.insert("stream".into(), json!(is_streaming));

    if is_streaming {
        let mut stream_opts = serde_json::Map::new();
        stream_opts.insert("include_usage".into(), json!(true));
        body.insert(
            "stream_options".into(),
            serde_json::Value::Object(stream_opts),
        );
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

    if is_streaming {
        execute_chat_stream_internal(ctx, res, start_time, &config, request_summary).await
    } else {
        execute_chat_non_stream_internal(res, start_time, &config, request_summary).await
    }
}

pub(crate) async fn execute_chat_stream_internal(
    ctx: &ExecuteContext,
    res: reqwest::Response,
    start_time: Instant,
    config: &ChatNodeConfig,
    request_summary: serde_json::Value,
) -> ExecuteResult {
    let mut stream = res.bytes_stream().eventsource();
    let mut full_content = String::new();
    let mut full_reasoning = String::new();
    let mut ttft_ms: Option<u64> = None;
    let mut usage = TokenUsage::default();

    while let Some(event_res) = stream.next().await {
        match event_res {
            Ok(event) => {
                let data = event.data.trim();
                if data == "[DONE]" {
                    break;
                }
                if data.is_empty() {
                    continue;
                }

                let chunk_val: serde_json::Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                // Check for usage in chunk
                if let Some(u) = chunk_val.get("usage") {
                    if let Ok(parsed_u) = serde_json::from_value::<TokenUsage>(u.clone()) {
                        usage = parsed_u;
                    }
                }

                // Extract delta choices
                if let Some(choices) = chunk_val.get("choices").and_then(|c| c.as_array()) {
                    if let Some(first_choice) = choices.first() {
                        let delta = first_choice.get("delta");

                        // Content chunk
                        let content_chunk = delta
                            .and_then(|d| d.get("content"))
                            .and_then(|c| c.as_str());

                        // Reasoning chunk (DeepSeek-R1 reasoning_content or reasoning)
                        let reasoning_chunk = delta
                            .and_then(|d| d.get("reasoning_content").or_else(|| d.get("reasoning")))
                            .and_then(|r| r.as_str());

                        let has_content = content_chunk.map(|s| !s.is_empty()).unwrap_or(false);
                        let has_reasoning = reasoning_chunk.map(|s| !s.is_empty()).unwrap_or(false);

                        if (has_content || has_reasoning) && ttft_ms.is_none() {
                            ttft_ms = Some(start_time.elapsed().as_millis() as u64);
                        }

                        if let Some(c) = content_chunk {
                            full_content.push_str(c);
                        }

                        if let Some(r) = reasoning_chunk {
                            full_reasoning.push_str(r);
                        }

                        // Emit stream update notification to host
                        if has_content || has_reasoning {
                            ctx.emit(
                                "stream",
                                json!({
                                    "kind": if has_content { "content" } else { "reasoning" },
                                    "delta_content": content_chunk.unwrap_or(""),
                                    "delta_reasoning": reasoning_chunk.unwrap_or(""),
                                    "accumulated_content_len": full_content.len(),
                                    "accumulated_reasoning_len": full_reasoning.len(),
                                }),
                            )
                            .await;
                        }
                    }
                }
            }
            Err(err) => {
                return ExecuteResult::fail(format!("SSE stream read error: {err}"));
            }
        }
    }

    let total_ms = start_time.elapsed().as_millis() as u64;

    // If total tokens is 0, provide a reasonable estimate
    if usage.total_tokens == 0 {
        let approx_comp = (full_content.len() / 4) as u32;
        let approx_reasoning = (full_reasoning.len() / 4) as u32;
        usage.completion_tokens = approx_comp + approx_reasoning;
        if approx_reasoning > 0 {
            usage.reasoning_tokens = Some(approx_reasoning);
        }
        usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
    }

    let mut parsed_json = None;
    if let Some(rf) = &config.parameters.response_format {
        if rf.kind == "json_object" || rf.kind == "json_schema" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&full_content) {
                parsed_json = Some(val);
            }
        }
    }

    let reasoning_opt = if full_reasoning.is_empty() {
        None
    } else {
        Some(full_reasoning)
    };

    let output = ChatExecutionOutput {
        content: full_content,
        reasoning_content: reasoning_opt,
        parsed_json,
        schema_valid: None,
        schema_errors: None,
        usage,
        latency_ms: LatencyMetrics { ttft_ms, total_ms },
    };

    let mut output_val = match serde_json::to_value(&output) {
        Ok(val) => val,
        Err(err) => return ExecuteResult::fail(format!("Serialization error: {err}")),
    };

    if let Some(obj) = output_val.as_object_mut() {
        obj.insert("request".into(), request_summary);
    }

    let mut res = ExecuteResult::ok(output_val.clone());
    res.report_data = Some(output_val);
    res
}
pub(crate) async fn execute_chat_non_stream_internal(
    res: reqwest::Response,
    start_time: Instant,
    config: &ChatNodeConfig,
    request_summary: serde_json::Value,
) -> ExecuteResult {
    let res_json: serde_json::Value = match res.json().await {
        Ok(v) => v,
        Err(err) => return ExecuteResult::fail(format!("Failed to parse response JSON: {err}")),
    };

    let mut full_content = String::new();
    let mut reasoning_opt = None;

    if let Some(choices) = res_json.get("choices").and_then(|c| c.as_array()) {
        if let Some(first_choice) = choices.first() {
            if let Some(msg) = first_choice.get("message") {
                if let Some(c) = msg.get("content").and_then(|c| c.as_str()) {
                    full_content = c.to_string();
                }
                if let Some(r) = msg
                    .get("reasoning_content")
                    .or_else(|| msg.get("reasoning"))
                    .and_then(|r| r.as_str())
                {
                    if !r.is_empty() {
                        reasoning_opt = Some(r.to_string());
                    }
                }
            }
        }
    }

    let mut usage = TokenUsage::default();
    if let Some(u) = res_json.get("usage") {
        if let Ok(parsed_u) = serde_json::from_value::<TokenUsage>(u.clone()) {
            usage = parsed_u;
        }
    }

    let total_ms = start_time.elapsed().as_millis() as u64;

    let mut parsed_json = None;
    if let Some(rf) = &config.parameters.response_format {
        if rf.kind == "json_object" || rf.kind == "json_schema" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&full_content) {
                parsed_json = Some(val);
            }
        }
    }

    let output = ChatExecutionOutput {
        content: full_content,
        reasoning_content: reasoning_opt,
        parsed_json,
        schema_valid: None,
        schema_errors: None,
        usage,
        latency_ms: LatencyMetrics {
            ttft_ms: None,
            total_ms,
        },
    };

    let mut output_val = match serde_json::to_value(&output) {
        Ok(val) => val,
        Err(err) => return ExecuteResult::fail(format!("Serialization error: {err}")),
    };

    if let Some(obj) = output_val.as_object_mut() {
        obj.insert("request".into(), request_summary);
    }

    let mut res = ExecuteResult::ok(output_val.clone());
    res.report_data = Some(output_val);
    res
}
