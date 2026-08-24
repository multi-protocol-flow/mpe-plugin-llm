//! Offline stdio JSON-RPC roundtrip tests against the compiled `mpe_plugin_llm` binary.

use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const PLUGIN_BIN: &str = env!("CARGO_BIN_EXE_mpe_plugin_llm");

const EXPECTED_TYPE_IDS: [&str; 5] = [
    "llm:chat",
    "llm:embeddings",
    "llm:provider",
    "llm:rerank",
    "llm:structured",
];

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
        let frame = serde_json::to_string(&serde_json::json!({
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
        }
    }

    fn shutdown(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("plugin exit");
        assert!(status.success(), "plugin should exit cleanly");
    }
}

#[test]
fn describe_returns_all_expected_nodes() {
    let mut p = PluginProcess::spawn();
    let resp = p.request("describe", Value::Null);
    let result = resp.get("result").expect("result present");
    let nodes = result.as_array().expect("nodes array");

    let mut actual_ids: Vec<&str> = nodes
        .iter()
        .map(|n| {
            n.get("type_id")
                .and_then(|t| t.as_str())
                .expect("type_id string")
        })
        .collect();
    actual_ids.sort();

    assert_eq!(actual_ids, EXPECTED_TYPE_IDS);

    for node in nodes {
        let node_type = node.get("type_id").and_then(|t| t.as_str()).unwrap();

        // Color check
        assert_eq!(
            node.get("color").and_then(|c| c.as_str()),
            Some("#10B981"),
            "node {node_type} should have #10B981 color"
        );
        let ports = node
            .get("ports")
            .and_then(|p| p.as_array())
            .expect("ports array");
        let port_names: Vec<&str> = ports
            .iter()
            .map(|p| p.get("id").and_then(|n| n.as_str()).expect("port id"))
            .collect();
        assert_eq!(port_names, vec!["in", "true", "false"]);

        // Frontend check
        let frontend = node.get("frontend").expect("frontend present");
        assert_eq!(
            frontend.get("type").and_then(|t| t.as_str()),
            Some("inline")
        );
        let content = frontend
            .get("content")
            .and_then(|c| c.as_str())
            .expect("content string");
        assert!(
            content.contains("<!DOCTYPE html>"),
            "frontend content must be html"
        );

        // Viewer check
        let viewer = node.get("viewer").expect("viewer present");
        assert_eq!(viewer.get("type").and_then(|t| t.as_str()), Some("inline"));
        let vcontent = viewer
            .get("content")
            .and_then(|c| c.as_str())
            .expect("viewer content string");
        assert!(
            vcontent.contains("<!DOCTYPE html>"),
            "viewer content must be html"
        );
    }

    p.shutdown();
}

#[test]
fn execute_invalid_node_type_fails() {
    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "execute",
        serde_json::json!({
            "config": { "type": "llm:invalid" }
        }),
    );
    let result = resp.get("result").expect("result present");
    assert_eq!(result.get("success").and_then(|s| s.as_bool()), Some(false));
    let errors = result
        .get("errors")
        .and_then(|e| e.as_array())
        .expect("errors array");
    assert!(!errors.is_empty());
    p.shutdown();
}

#[test]
fn execute_provider_node_registers_and_succeeds() {
    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "execute",
        serde_json::json!({
            "execution_id": "test-exec-1",
            "node_instance_id": "prov-node-1",
            "config": {
                "type": "llm:provider",
                "base_url": "https://api.openai.com/v1",
                "api_key": "sk-test",
                "model": "gpt-4o",
                "timeout_ms": 30000
            }
        }),
    );
    let result = resp.get("result").expect("result present");
    assert_eq!(result.get("success").and_then(|s| s.as_bool()), Some(true));
    let output = result.get("output_data").expect("output_data present");
    assert_eq!(output.get("connected").and_then(|c| c.as_bool()), Some(true));
    assert_eq!(
        output.get("provider_uuid").and_then(|u| u.as_str()),
        Some("prov-node-1")
    );
    p.shutdown();
}

#[test]
fn ui_call_unknown_method_returns_error() {
    let mut p = PluginProcess::spawn();
    let resp = p.request(
        "uiCall",
        serde_json::json!({
            "method": "unknown.method",
            "params": {}
        }),
    );
    assert!(resp.get("error").is_some());
    p.shutdown();
}
