//! HTTP client builder and request utilities for OpenAI-compatible LLM endpoints.

use crate::types::LlmProviderConfig;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use std::time::Duration;

/// Builds a configured `reqwest::Client` with rustls and custom headers.
pub fn build_client(config: &LlmProviderConfig) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(api_key) = &config.api_key {
        let trimmed = api_key.trim();
        if !trimmed.is_empty() {
            let auth_str = if trimmed.starts_with("Bearer ") {
                trimmed.to_string()
            } else {
                format!("Bearer {}", trimmed)
            };
            let mut val = HeaderValue::from_str(&auth_str)
                .map_err(|e| format!("Invalid API key header: {e}"))?;
            val.set_sensitive(true);
            headers.insert(AUTHORIZATION, val);
        }
    }

    if let Some(custom) = &config.custom_headers {
        for (k, v) in custom {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                headers.insert(name, val);
            }
        }
    }

    let timeout = Duration::from_millis(config.timeout_ms.max(1000));

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(timeout)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

/// Normalizes endpoint URL by joining path to base_url without double slashes.
pub fn join_endpoint(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let ep = endpoint.trim_start_matches('/');
    format!("{}/{}", base, ep)
}

/// Masks an API key for safe logging and report display (e.g. `sk-a1****bc23`).
pub fn mask_key(key: Option<&str>) -> String {
    match key {
        Some(k) if k.len() > 8 => {
            let start = &k[..4];
            let end = &k[k.len() - 4..];
            format!("{start}****{end}")
        }
        Some(k) if !k.is_empty() => "****".to_string(),
        _ => "(none)".to_string(),
    }
}

/// Builds a structured request descriptor for inspectability and debug reports.
pub fn build_request_summary(
    url: &str,
    model: &str,
    api_key: Option<&str>,
    body: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "url": url,
        "method": "POST",
        "model": model,
        "api_key_masked": mask_key(api_key),
        "payload": body,
        "body": body,
    })
}
