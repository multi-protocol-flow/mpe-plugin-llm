//! UI Call handlers for design-time operations (model listing, connection testing).

use crate::client::{build_client, join_endpoint};
use crate::types::LlmProviderConfig;
use serde_json::json;
use std::time::Instant;

/// Handles `uiCall` design-time requests dispatched from the host / config panel.
pub async fn handle_ui_call(
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    match method {
        "llm.test_connection" => test_connection(params).await,
        "llm.list_models" => list_models(params).await,
        other => Err(format!("Unknown UI call method: {other}")),
    }
}

async fn extract_provider_config(params: &serde_json::Value) -> Result<LlmProviderConfig, String> {
    if let Some(provider_val) = params.get("provider") {
        serde_json::from_value(provider_val.clone())
            .map_err(|e| format!("Invalid provider config: {e}"))
    } else {
        serde_json::from_value(params.clone()).map_err(|e| format!("Invalid config: {e}"))
    }
}

async fn test_connection(params: serde_json::Value) -> Result<serde_json::Value, String> {
    let provider = extract_provider_config(&params).await?;
    let client = build_client(&provider)?;
    let url = join_endpoint(&provider.base_url, "models");

    let start = Instant::now();
    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Connection test failed: {e}"))?;

    let latency_ms = start.elapsed().as_millis() as u64;

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".into());
        return Err(format!("Server returned error {status}: {err_text}"));
    }

    let json_val: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let models_count = json_val
        .get("data")
        .and_then(|d| d.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    Ok(json!({
        "ok": true,
        "latency_ms": latency_ms,
        "models_count": models_count,
    }))
}

async fn list_models(params: serde_json::Value) -> Result<serde_json::Value, String> {
    let provider = extract_provider_config(&params).await?;
    let client = build_client(&provider)?;
    let url = join_endpoint(&provider.base_url, "models");

    let res = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {e}"))?;

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_else(|_| "Unknown error".into());
        return Err(format!("Server returned error {status}: {err_text}"));
    }

    let json_val: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    let mut model_ids = Vec::new();
    if let Some(data_arr) = json_val.get("data").and_then(|d| d.as_array()) {
        for item in data_arr {
            if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                model_ids.push(id.to_string());
            }
        }
    }

    model_ids.sort();

    Ok(json!({
        "models": model_ids,
    }))
}
