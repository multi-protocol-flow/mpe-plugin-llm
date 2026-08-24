# MPE LLM 插件

> **定位**：本仓库是 MPE 的**官方协议插件之一**——它不依赖宿主仓库的任何代码，只依赖公开的 `mpe-plugin-sdk`（sidecar 进程 + stdio 上的 JSON-RPC，git tag `v0.2.2`）。宿主在启动时扫描插件目录，执行 `describe` 握手，注册节点类型，再通过 `execute` RPC 调用本插件进程。
>
> 本仓库与宿主相互独立（宿主仓库的 `.gitignore` 忽略了 `/plugins/`，且宿主从不构建它）。它由本仓库自己的 CI（GitHub Actions → Release artifacts）完成构建与发布。

---

## 0. 一分钟了解插件工作方式

```
Host (mpe / mpe-cli)                    Plugin process (本 crate)
   │  scans plugins/ dir                       │
   │  ── describe ───────────────────────────► │  返回节点描述（type、ports、config schema）
   │  ◄─────────── node list ─────────────────  │
   │  ── execute(config, execution_id) ──────► │  执行 LLM 操作（流式、思考捕获、指标统计）
   │  ◄─────────── result / stream events ────  │
   │  ── flowEnded(execution_id) ────────────► │  释放该次执行的连接池与资源
```

- **传输**：stdin/stdout，每行一个 JSON 文档（JSON-RPC 2.0，LF 帧）
- **驻留模式**：`capabilities.streaming: true` → 进程常驻，HTTP 连接池跨多次执行复用
- **单节点连通性验证**：`llm:provider` 及所有操作节点均声明了 `capabilities.single_node: true`，宿主会在 `mpe run-node` / GUI 测试按钮中放行，无需宿主侧额外代码
- **无共享内存**：插件是独立进程，宿主通过 JSON 传递参数并接收结果

## 1. 项目结构

```
mpe-plugin-llm/
├── Cargo.toml            # 独立包，不依赖宿主 workspace
├── plugin.json           # 宿主扫描的清单文件（启动描述、驻留模式）
├── .github/workflows/ci.yml  # 3 平台构建 + 测试 + Release 打包
├── src/
│   ├── main.rs           # 二进制入口（安装 rustls + SDK 事件循环）
│   ├── lib.rs            # LlmPlugin：Plugin trait 实现 + 节点描述
│   ├── i18n.rs           # 基于 MPE_LOCALE 的中/英文案
│   ├── client.rs         # HTTP 客户端构建 + SSE 流式解析
│   ├── types.rs          # LLM 领域类型（请求/响应/思考过程/指标）
│   ├── ui.rs             # 设计期 UI 交互（测试连接、模型列表拉取）
│   └── nodes/
│       ├── provider.rs   # llm:provider（连通性测试 + 配置载体）
│       ├── chat.rs       # llm:chat（流式对话 + 思考链捕获 + Token 指标）
│       ├── structured.rs # llm:structured（JSON Schema 结构化提取 + Rust 侧校验）
│       ├── embeddings.rs # llm:embeddings（文本向量嵌入生成）
│       └── rerank.rs     # llm:rerank（交叉编码器文档重排序）
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
    ├── roundtrip.rs      # 离线 stdio roundtrip 测试（describe / 失败路径）
    └── mock_llm_e2e.rs   # 基于 wiremock 的全节点类型端到端测试
```

## 2. 节点类型

| type_id | ports | 说明 |
|---------|-------|------|
| `llm:provider` | in/true/false | 配置 LLM 服务商凭据（base_url、api_key、model、timeout_ms、custom_headers），声明 `single_node: true` |
| `llm:chat` | in/true/false | OpenAI 兼容聊天补全，支持 SSE 流式传输、思考链（reasoning）提取与 Token 耗时指标统计 |
| `llm:structured` | in/true/false | 结构化输出提取，支持严格 JSON Schema 约束与 Rust 侧实时校验 |
| `llm:embeddings` | in/true/false | 生成输入文本的向量嵌入（Embeddings） |
| `llm:rerank` | in/true/false | 交叉编码器文档重排序（兼容 SiliconFlow / Cohere 等 API 协议） |

## 3. 构建与测试

```bash
# 前端构建（配置面板 + 报告查看器）
cd frontend && npm install && npm run build
cd ..

# 构建 release 二进制（宿主通过 plugin.json entry.command 启动它）
cargo build --release

# 运行离线单元测试与 Mock 端到端测试
cargo test
```
