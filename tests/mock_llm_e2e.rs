//! Mock HTTP end-to-end integration tests for LLM plugin over JSON-RPC 2.0 stdio transport.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PLUGIN_BIN: &str = env!("CARGO_BIN_EXE_mpe_plugin_llm");

struct PluginProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl PluginProcess {
    fn spawn() -> PluginProcess {
        let mut child = Command::new(PLUGIN_BIN)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn mpe_plugin_llm");
        let stdin = child.stdin.take().expect("plugin stdin unavailable");
        let stdout = child.stdout.take().expect("plugin stdout unavailable");
        PluginProcess {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            next_id: 0,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let frame = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))
        .expect("request serializable");
        let stdin = self.stdin.as_mut().expect("stdin alive");
        writeln!(stdin, "{frame}").expect("request write");
        stdin.flush().expect("request flush");

        let mut line = String::new();
        loop {
            line.clear();
            let n = self
                .stdout
                .read_line(&mut line)
                .expect("read response frame");
            assert!(n > 0, "plugin stdout closed early");
            let frame: Value = serde_json::from_str(&line).expect("frame is valid JSON");
            if frame.get("id").is_some() {
                return frame;
            }
            // Notifications (e.g. llm.stream / log) have no id; reader continues
        }
    }

    fn shutdown(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("plugin exit");
        assert!(status.success(), "plugin should exit cleanly");
    }
}

#[tokio::test]
async fn test_chat_streaming_with_reasoning() {
    let server = MockServer::start().await;

    let sse_body = "\
data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"I need to greet the user.\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"world!\"}}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n\
data: [DONE]\n\n";

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body),
        )
        .mount(&server)
        .await;

    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "execute",
        json!({
            "config": {
                "type": "llm:chat",
                "provider": {
                    "base_url": server.uri(),
                    "api_key": "test-key",
                    "model": "deepseek-reasoner",
                },
                "messages": [
                    { "role": "user", "content": "Hi" }
                ],
                "parameters": {
                    "stream": true,
                    "temperature": 0.5
                }
            }
        }),
    );

    let result = resp.get("result").expect("result present");
    assert_eq!(result.get("success").and_then(|s| s.as_bool()), Some(true));

    let output = result.get("output_data").expect("output_data present");
    assert_eq!(
        output.get("content").and_then(|c| c.as_str()),
        Some("Hello world!")
    );
    assert_eq!(
        output.get("reasoning_content").and_then(|r| r.as_str()),
        Some("I need to greet the user.")
    );

    let usage = output.get("usage").expect("usage present");
    assert_eq!(usage.get("total_tokens").and_then(|t| t.as_u64()), Some(15));

    let latency = output.get("latency_ms").expect("latency present");
    assert!(latency.get("ttft_ms").is_some(), "ttft should be recorded");

    let req = output.get("request").expect("request info present");
    assert_eq!(req.get("method").and_then(|m| m.as_str()), Some("POST"));
    let payload = req.get("payload").expect("payload present");
    assert_eq!(payload.get("model").and_then(|m| m.as_str()), Some("deepseek-reasoner"));
    assert_eq!(payload.get("stream").and_then(|s| s.as_bool()), Some(true));

    let report_data = result.get("report_data").expect("report_data present");
    assert_eq!(report_data.get("request"), output.get("request"));

    p.shutdown();
}

#[tokio::test]
async fn test_chat_non_stream() {
    let server = MockServer::start().await;

    let resp_json = json!({
        "id": "chatcmpl-123",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Non-streamed response",
                    "reasoning_content": "Internal thought"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 8,
            "total_tokens": 20
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resp_json))
        .mount(&server)
        .await;

    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "execute",
        json!({
            "config": {
                "type": "llm:chat",
                "provider": {
                    "base_url": server.uri(),
                    "api_key": "test-key",
                    "model": "gpt-4o",
                },
                "messages": [
                    { "role": "user", "content": "Hello" }
                ],
                "parameters": {
                    "stream": false
                }
            }
        }),
    );

    let result = resp.get("result").expect("result present");
    assert_eq!(result.get("success").and_then(|s| s.as_bool()), Some(true));

    let output = result.get("output_data").expect("output_data present");
    assert_eq!(
        output.get("content").and_then(|c| c.as_str()),
        Some("Non-streamed response")
    );
    assert_eq!(
        output.get("reasoning_content").and_then(|r| r.as_str()),
        Some("Internal thought")
    );

    let req = output.get("request").expect("request info present");
    assert_eq!(req.get("method").and_then(|m| m.as_str()), Some("POST"));
    let body = req.get("body").expect("body present");
    assert_eq!(body.get("model").and_then(|m| m.as_str()), Some("gpt-4o"));
    assert_eq!(body.get("stream").and_then(|s| s.as_bool()), Some(false));

    let report_data = result.get("report_data").expect("report_data present");
    assert!(report_data.get("request").is_some());

    p.shutdown();
}
#[tokio::test]
async fn test_structured_success_schema_validation() {
    let server = MockServer::start().await;

    let target_json = json!({
        "sentiment": "positive",
        "score": 0.95,
        "keywords": ["fast", "reliable"]
    });

    let resp_json = json!({
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": target_json.to_string()
                }
            }
        ],
        "usage": {
            "prompt_tokens": 20,
            "completion_tokens": 15,
            "total_tokens": 35
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resp_json))
        .mount(&server)
        .await;

    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "execute",
        json!({
            "config": {
                "type": "llm:structured",
                "provider": {
                    "base_url": server.uri(),
                    "model": "gpt-4o",
                },
                "messages": [
                    { "role": "user", "content": "Analyze product review" }
                ],
                "json_schema": {
                    "type": "object",
                    "properties": {
                        "sentiment": { "type": "string", "enum": ["positive", "negative", "neutral"] },
                        "score": { "type": "number", "minimum": 0, "maximum": 1 },
                        "keywords": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["sentiment", "score", "keywords"]
                },
                "parameters": {
                    "stream": false
                },
                "strict_validation": true
            }
        }),
    );

    let result = resp.get("result").expect("result present");
    assert_eq!(result.get("success").and_then(|s| s.as_bool()), Some(true));
    assert_eq!(
        result.get("next_ports").and_then(|p| p.as_array()),
        Some(&vec![json!("true")])
    );

    let output = result.get("output_data").expect("output_data present");
    assert_eq!(
        output.get("schema_valid").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(output.get("parsed_json"), Some(&target_json));

    let req = output.get("request").expect("request present in structured output");
    let payload = req.get("payload").expect("payload present");
    assert_eq!(payload.get("model").and_then(|m| m.as_str()), Some("gpt-4o"));
    assert!(payload.get("response_format").is_some());

    let report_data = result.get("report_data").expect("report_data present in structured output");
    assert!(report_data.get("request").is_some());

    p.shutdown();
}
#[tokio::test]
async fn test_structured_schema_validation_failure() {
    let server = MockServer::start().await;

    // Returns score > 1.0 violating schema maximum constraint
    let invalid_json = json!({
        "sentiment": "unknown_value",
        "score": 99.5
    });

    let resp_json = json!({
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": invalid_json.to_string()
                }
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resp_json))
        .mount(&server)
        .await;

    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "execute",
        json!({
            "config": {
                "type": "llm:structured",
                "provider": {
                    "base_url": server.uri(),
                    "model": "gpt-4o",
                },
                "messages": [
                    { "role": "user", "content": "Analyze product review" }
                ],
                "json_schema": {
                    "type": "object",
                    "properties": {
                        "sentiment": { "type": "string", "enum": ["positive", "negative", "neutral"] },
                        "score": { "type": "number", "minimum": 0, "maximum": 1 }
                    },
                    "required": ["sentiment", "score"]
                },
                "parameters": {
                    "stream": false
                },
                "strict_validation": true
            }
        }),
    );

    let result = resp.get("result").expect("result present");
    assert_eq!(
        result.get("success").and_then(|s| s.as_bool()),
        Some(false),
        "strict validation failure must yield success: false"
    );
    assert_eq!(
        result.get("next_ports").and_then(|p| p.as_array()),
        Some(&vec![json!("false")]),
        "must route to false port on schema failure"
    );

    let output = result.get("output_data").expect("output_data present");
    assert_eq!(
        output.get("schema_valid").and_then(|v| v.as_bool()),
        Some(false)
    );
    let errors = output
        .get("schema_errors")
        .and_then(|e| e.as_array())
        .expect("errors present");
    assert!(!errors.is_empty(), "should report schema errors");

    p.shutdown();
}

#[tokio::test]
async fn test_embeddings_node() {
    let server = MockServer::start().await;

    let resp_json = json!({
        "data": [
            { "embedding": [0.1, 0.2, 0.3], "index": 0 },
            { "embedding": [0.4, 0.5, 0.6], "index": 1 }
        ],
        "usage": {
            "prompt_tokens": 16,
            "total_tokens": 16
        }
    });

    Mock::given(method("POST"))
        .and(path("/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resp_json))
        .mount(&server)
        .await;

    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "execute",
        json!({
            "config": {
                "type": "llm:embeddings",
                "provider": {
                    "base_url": server.uri(),
                    "model": "text-embedding-3-small"
                },
                "input": ["sentence 1", "sentence 2"]
            }
        }),
    );

    let result = resp.get("result").expect("result present");
    assert_eq!(result.get("success").and_then(|s| s.as_bool()), Some(true));

    let output = result.get("output_data").expect("output_data present");
    let request = output.get("request").expect("request info present");
    assert_eq!(request.get("model").and_then(|m| m.as_str()), Some("text-embedding-3-small"));
    assert!(request.get("payload").is_some());
    let data = output
        .get("data")
        .and_then(|d| d.as_array())
        .expect("data array");
    assert_eq!(data.len(), 2);
    p.shutdown();
}

#[tokio::test]
async fn test_rerank_node() {
    let server = MockServer::start().await;

    let resp_json = json!({
        "results": [
            { "index": 1, "relevance_score": 0.98, "document": { "text": "Doc B text" } },
            { "index": 0, "relevance_score": 0.35, "document": { "text": "Doc A text" } }
        ],
        "usage": {
            "total_tokens": 80
        }
    });

    Mock::given(method("POST"))
        .and(path("/rerank"))
        .respond_with(ResponseTemplate::new(200).set_body_json(resp_json))
        .mount(&server)
        .await;

    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "execute",
        json!({
            "config": {
                "type": "llm:rerank",
                "provider": {
                    "base_url": server.uri(),
                    "model": "BAAI/bge-reranker-v2-m3"
                },
                "query": "find fastest way",
                "documents": ["Doc A text", "Doc B text"],
                "top_n": 2,
                "return_documents": true
            }
        }),
    );

    let result = resp.get("result").expect("result present");
    assert_eq!(result.get("success").and_then(|s| s.as_bool()), Some(true));

    let output = result.get("output_data").expect("output_data present");
    let request = output.get("request").expect("request info present");
    assert_eq!(request.get("model").and_then(|m| m.as_str()), Some("BAAI/bge-reranker-v2-m3"));
    let results = output
        .get("results")
        .and_then(|r| r.as_array())
        .expect("results array");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].get("index").and_then(|i| i.as_u64()), Some(1));
    p.shutdown();
}

#[tokio::test]
async fn test_ui_calls() {
    let server = MockServer::start().await;

    let models_json = json!({
        "data": [
            { "id": "gpt-4o" },
            { "id": "deepseek-chat" }
        ]
    });

    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(models_json))
        .mount(&server)
        .await;

    let mut p = PluginProcess::spawn();

    // Test test_connection
    let test_res = p.request(
        "uiCall",
        json!({
            "method": "llm.test_connection",
            "params": {
                "provider": {
                    "base_url": server.uri(),
                    "api_key": "sk-xxx"
                }
            }
        }),
    );
    let test_result = test_res.get("result").expect("result present");
    assert_eq!(test_result.get("ok").and_then(|o| o.as_bool()), Some(true));
    assert_eq!(
        test_result.get("models_count").and_then(|c| c.as_u64()),
        Some(2)
    );

    // Test list_models
    let list_res = p.request(
        "uiCall",
        json!({
            "method": "llm.list_models",
            "params": {
                "provider": {
                    "base_url": server.uri(),
                    "api_key": "sk-xxx"
                }
            }
        }),
    );
    let list_result = list_res.get("result").expect("result present");
    let models = list_result
        .get("models")
        .and_then(|m| m.as_array())
        .expect("models list");
    assert_eq!(models.len(), 2);

    p.shutdown();
}

#[tokio::test]
async fn test_provider_node_reference_in_chat() {
    let server = MockServer::start().await;

    let response_body = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "Hello from pooled provider!"
            }
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 6,
            "total_tokens": 16
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&server)
        .await;

    let mut p = PluginProcess::spawn();

    // 1. Execute `llm:provider` node
    let prov_resp = p.request(
        "execute",
        json!({
            "execution_id": "flow-exec-100",
            "node_instance_id": "prov-instance-abc",
            "config": {
                "type": "llm:provider",
                "base_url": server.uri(),
                "api_key": "secret-key",
                "model": "gpt-4o",
                "timeout_ms": 10000
            }
        }),
    );
    let prov_res = prov_resp.get("result").expect("prov result present");
    assert_eq!(prov_res.get("success").and_then(|s| s.as_bool()), Some(true));

    // 2. Execute `llm:chat` referencing `provider_uuid = "prov-instance-abc"`
    let chat_resp = p.request(
        "execute",
        json!({
            "execution_id": "flow-exec-100",
            "node_instance_id": "chat-node-2",
            "config": {
                "type": "llm:chat",
                "provider_uuid": "prov-instance-abc",
                "messages": [
                    { "role": "user", "content": "Hi!" }
                ],
                "parameters": {
                    "stream": false
                }
            }
        }),
    );
    let chat_res = chat_resp.get("result").expect("chat result present");
    assert_eq!(chat_res.get("success").and_then(|s| s.as_bool()), Some(true));
    let output = chat_res.get("output_data").expect("output_data present");
    assert_eq!(
        output.get("content").and_then(|c| c.as_str()),
        Some("Hello from pooled provider!")
    );

    p.shutdown();
}
