# MPE LLM Plugin

> **Positioning**: this plugin is one of MPE's **official protocol plugins** — it
> depends on none of the host repository's code, only the public
> `mpe-plugin-sdk` (sidecar process + JSON-RPC over stdio, git tag `v0.2.2`).
> The host scans its plugin directory at startup, runs the `describe` handshake,
> registers the node types, and calls this plugin process via the `execute` RPC.
>
> This repository is independent of the host (the host repository's `.gitignore`
> ignores `/plugins/` and the host never builds it). It is built and released by
> this repository's own CI (GitHub Actions → Release artifacts).

---

## 0. How the plugin works in one minute

```
Host (mpe / mpe-cli)                    Plugin process (this crate)
   │  scans plugins/ dir                       │
   │  ── describe ───────────────────────────► │  returns node descriptions (type, ports, config schema)
   │  ◄─────────── node list ─────────────────  │
   │  ── execute(config, execution_id) ──────► │  runs the LLM operation (streaming / metrics)
   │  ◄─────────── result / stream events ────  │
   │  ── flowEnded(execution_id) ────────────► │  releases the per-execution connection pool
```

- **Transport**: stdin/stdout, one JSON document per line (JSON-RPC 2.0, LF-framed)
- **Resident**: `capabilities.streaming: true` → the process stays alive, connections and HTTP clients are pooled
- **Single-node verification**: `llm:provider` and all operational nodes declare `capabilities.single_node: true` → the host's `mpe run-node` / GUI test button can verify connectivity without any host code change
- **No shared memory**: the plugin is a separate process; the host passes parameters and receives outputs as JSON

## 1. Project structure

```
mpe-plugin-llm/
├── Cargo.toml            # standalone package, no host workspace dependency
├── plugin.json           # manifest scanned by the host (launch description, residency mode)
├── .github/workflows/ci.yml  # 3-platform build + tests + Release packaging
├── src/
│   ├── main.rs           # binary entry point (rustls install + SDK event loop)
│   ├── lib.rs            # LlmPlugin: Plugin trait impl + node descriptions
│   ├── i18n.rs           # MPE_LOCALE-driven zh-CN / en-US copy
│   ├── client.rs         # HTTP client builder + SSE streaming parser
│   ├── types.rs          # LLM domain types (request/response/metrics/reasoning)
│   ├── ui.rs             # design-time UI call handlers (test_connection, list_models)
│   └── nodes/
│       ├── provider.rs   # llm:provider (connectivity test + config carrier)
│       ├── chat.rs       # llm:chat (streaming chat completion + reasoning + token metrics)
│       ├── structured.rs # llm:structured (JSON Schema constrained output + Rust validation)
│       ├── embeddings.rs # llm:embeddings (vector embeddings generation)
│       └── rerank.rs     # llm:rerank (cross-encoder document reranking)
├── frontend/
│   ├── package.json
│   ├── vite.config.ts
│   ├── vite.config.viewer.ts
│   ├── src/
│   │   ├── viewer.tsx / panel.tsx / bridge.ts / styles.css
│   │   ├── i18n.ts / types.ts / icons.tsx
│   ├── viewer.html
│   ├── panel.html
│   └── scripts/inline.mjs
└── tests/
    ├── roundtrip.rs      # offline stdio roundtrip tests (describe / schema validation)
    └── mock_llm_e2e.rs   # wiremock-based end-to-end tests for all node types
```

## 2. Node types

| type_id | ports | description |
|---------|-------|-------------|
| `llm:provider` | in/true/false | Configure LLM Provider credentials (base_url, api_key, model, timeout_ms, custom_headers); declares `single_node: true` |
| `llm:chat` | in/true/false | OpenAI-compatible chat completions with SSE streaming, reasoning / thought capture, and token usage metrics |
| `llm:structured` | in/true/false | Structured JSON output with strict JSON schema constraints and Rust-side validation |
| `llm:embeddings` | in/true/false | Vector embeddings generation for text inputs |
| `llm:rerank` | in/true/false | Cross-encoder document reranking (SiliconFlow / Cohere compatible) |

## 3. Build and test

```bash
# Frontend build (config panel + viewer)
cd frontend && npm install && npm run build
cd ..

# Build release binary
cargo build --release

# Offline unit + roundtrip + mock E2E tests
cargo test
```
