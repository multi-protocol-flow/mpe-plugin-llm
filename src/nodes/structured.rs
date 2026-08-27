//! Executor for `llm:structured` node with JSON Schema constraints and validation.

use mpe_plugin_sdk::prelude::*;
use serde_json::json;

use crate::types::{ChatExecutionOutput, ChatNodeConfig, ResponseFormat, StructuredNodeConfig};

/// Executes the `llm:structured` node.
pub async fn execute_structured(
    ctx: &mut ExecuteContext,
    pool: &std::sync::Arc<mpe_plugin_sdk::pool::ConnectionPool>,
) -> ExecuteResult {
    let config_val = ctx.config().clone();
    let config: StructuredNodeConfig = match serde_json::from_value(config_val) {
        Ok(cfg) => cfg,
        Err(err) => return ExecuteResult::fail(format!("Invalid llm:structured config: {err}")),
    };

    if config.json_schema.is_null() || !config.json_schema.is_object() {
        return ExecuteResult::fail("A valid JSON Schema object is required for llm:structured");
    }

    // Pre-compile schema to check if it's valid
    let validator = match jsonschema::validator_for(&config.json_schema) {
        Ok(v) => v,
        Err(err) => {
            return ExecuteResult::fail(format!("Invalid JSON Schema definition: {err}"));
        }
    };

    // Prepare Chat parameters with json_schema response_format
    let mut params = config.parameters.clone();
    params.response_format = Some(ResponseFormat {
        kind: "json_schema".to_string(),
        json_schema: Some(json!({
            "name": "structured_output",
            "strict": true,
            "schema": config.json_schema,
        })),
    });

    let chat_config = ChatNodeConfig {
        provider_uuid: config.provider_uuid,
        provider: config.provider,
        override_model: config.override_model,
        override_timeout_ms: config.override_timeout_ms,
        messages: config.messages,
        parameters: params,
    };


    // Run inner chat execution
    // Run inner chat execution
    let chat_result = execute_chat_with_config(ctx, &chat_config, pool).await;
    if !chat_result.success {
        return chat_result;
    }

    let request_summary = chat_result
        .output_data
        .as_ref()
        .and_then(|v| v.get("request"))
        .cloned();

    let mut output: ChatExecutionOutput = match chat_result
        .output_data
        .and_then(|v| serde_json::from_value(v).ok())
    {
        Some(out) => out,
        None => return ExecuteResult::fail("Failed to extract chat output for validation"),
    };

    // Parse the generated content as JSON
    let parsed_json_result: Result<serde_json::Value, _> = serde_json::from_str(&output.content);
    let parsed_val = match parsed_json_result {
        Ok(val) => val,
        Err(err) => {
            let error_msg = format!(
                "Model output is not valid JSON: {err}. Raw output: {}",
                output.content
            );
            output.schema_valid = Some(false);
            output.schema_errors = Some(vec![error_msg.clone()]);
            let mut output_val = serde_json::to_value(&output).unwrap_or(json!({}));
            if let (Some(obj), Some(req)) = (output_val.as_object_mut(), request_summary.as_ref()) {
                obj.insert("request".into(), req.clone());
            }
            if config.strict_validation {
                return ExecuteResult {
                    success: false,
                    next_ports: vec!["false".into()],
                    output_data: Some(output_val.clone()),
                    errors: vec![error_msg],
                    report_data: Some(output_val),
                };
            } else {
                let mut res = ExecuteResult::ok(output_val.clone());
                res.report_data = Some(output_val);
                return res;
            }
        }
    };

    // Validate parsed JSON with compiled jsonschema validator
    let mut schema_errors = Vec::new();
    for error in validator.iter_errors(&parsed_val) {
        let path = error.instance_path.to_string();
        let path_str = if path.is_empty() {
            "root".to_string()
        } else {
            path
        };
        schema_errors.push(format!("At '{path_str}': {error}"));
    }

    if schema_errors.is_empty() {
        output.schema_valid = Some(true);
        output.schema_errors = None;
        output.parsed_json = Some(parsed_val);

        let mut output_val = match serde_json::to_value(&output) {
            Ok(v) => v,
            Err(e) => return ExecuteResult::fail(format!("Serialization error: {e}")),
        };
        if let (Some(obj), Some(req)) = (output_val.as_object_mut(), request_summary.as_ref()) {
            obj.insert("request".into(), req.clone());
        }
        let mut res = ExecuteResult::ok(output_val.clone());
        res.next_ports = vec!["true".into()];
        res.report_data = Some(output_val);
        res
    } else {
        output.schema_valid = Some(false);
        output.schema_errors = Some(schema_errors.clone());
        output.parsed_json = Some(parsed_val);

        let mut output_val = match serde_json::to_value(&output) {
            Ok(v) => v,
            Err(e) => return ExecuteResult::fail(format!("Serialization error: {e}")),
        };
        if let (Some(obj), Some(req)) = (output_val.as_object_mut(), request_summary.as_ref()) {
            obj.insert("request".into(), req.clone());
        }

        if config.strict_validation {
            let error_summary = format!(
                "JSON Schema validation failed with {} error(s): {}",
                schema_errors.len(),
                schema_errors.join("; ")
            );
            ExecuteResult {
                success: false,
                next_ports: vec!["false".into()],
                output_data: Some(output_val.clone()),
                errors: vec![error_summary],
                report_data: Some(output_val),
            }
        } else {
            let mut res = ExecuteResult::ok(output_val.clone());
            res.next_ports = vec!["true".into()];
            res.report_data = Some(output_val);
            res
        }
    }
}
async fn execute_chat_with_config(
    ctx: &mut ExecuteContext,
    node_config: &ChatNodeConfig,
    pool: &std::sync::Arc<mpe_plugin_sdk::pool::ConnectionPool>,
) -> ExecuteResult {
    let exec_id = ctx.execution_id().unwrap_or("default");
    let provider = match crate::types::resolve_effective_provider(
        pool,
        exec_id,
        node_config.provider_uuid.as_deref(),
        &node_config.provider,
        node_config.override_model.as_deref(),
        node_config.override_timeout_ms,
    ) {
        Ok(p) => p,
        Err(err) => return ExecuteResult::fail(err),
    };

    if provider.model.trim().is_empty() {
        return ExecuteResult::fail("Model name is required for llm:structured");
    }

    let client = match crate::client::build_client(&provider) {
        Ok(c) => c,
        Err(err) => return ExecuteResult::fail(err),
    };

    let url = crate::client::join_endpoint(&provider.base_url, "chat/completions");
    let mut messages_json = Vec::new();
    for msg in &node_config.messages {
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

    if let Some(temp) = node_config.parameters.temperature {
        body.insert("temperature".into(), json!(temp));
    }
    if let Some(top_p) = node_config.parameters.top_p {
        body.insert("top_p".into(), json!(top_p));
    }
    if let Some(max_tokens) = node_config.parameters.max_tokens {
        body.insert("max_tokens".into(), json!(max_tokens));
    }
    if let Some(stop) = &node_config.parameters.stop {
        body.insert("stop".into(), json!(stop));
    }
    if let Some(rf) = &node_config.parameters.response_format {
        let mut rf_obj = serde_json::Map::new();
        rf_obj.insert("type".into(), json!(rf.kind));
        if let Some(schema) = &rf.json_schema {
            rf_obj.insert("json_schema".into(), schema.clone());
        }
        body.insert("response_format".into(), serde_json::Value::Object(rf_obj));
    }

    let is_streaming = node_config.parameters.stream;
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
    let request_summary = crate::client::build_request_summary(
        &url,
        &provider.model,
        provider.api_key.as_deref(),
        &body_val,
    );

    let start_time = std::time::Instant::now();

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
        let headers_map: std::collections::HashMap<String, String> = res
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
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
                "headers": headers_map,
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
        // Reuse chat streaming logic
        crate::nodes::chat::execute_chat_stream_internal(ctx, res, start_time, node_config, request_summary).await
    } else {
        crate::nodes::chat::execute_chat_non_stream_internal(res, start_time, node_config, request_summary).await
    }
}
