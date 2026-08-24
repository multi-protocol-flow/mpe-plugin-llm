export interface LlmProviderConfig {
  base_url: string;
  api_key?: string;
  model: string;
  timeout_ms?: number;
  custom_headers?: Record<string, string>;
}

export interface ProviderNodeConfig {
  base_url: string;
  api_key?: string;
  model: string;
  timeout_ms?: number;
  custom_headers?: Record<string, string>;
}

export interface ChatMessage {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
  name?: string;
}

export interface ResponseFormat {
  type: 'text' | 'json_object' | 'json_schema';
  json_schema?: Record<string, unknown>;
}

export interface ChatParameters {
  temperature?: number;
  top_p?: number;
  max_tokens?: number;
  stream?: boolean;
  response_format?: ResponseFormat;
  stop?: string[];
}

export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  reasoning_tokens?: number;
  total_tokens: number;
}

export interface LatencyMetrics {
  ttft_ms?: number;
  total_ms: number;
}

export interface ChatExecutionOutput {
  content: string;
  reasoning_content?: string;
  parsed_json?: unknown;
  schema_valid?: boolean;
  schema_errors?: string[];
  usage: TokenUsage;
  latency_ms: LatencyMetrics;
}

export interface EmbeddingsOutput {
  data: number[][];
  usage: TokenUsage;
  latency_ms: LatencyMetrics;
}

export interface RerankItem {
  index: number;
  relevance_score: number;
  document?: string;
}

export interface RerankOutput {
  results: RerankItem[];
  usage?: TokenUsage;
  latency_ms: LatencyMetrics;
}

export interface NodeExecutionReport {
  node_uuid?: string;
  node_name?: string;
  node_type?: string;
  status?: string;
  duration_ms?: number;
  output_data?: unknown;
  plugin_data?: unknown;
  error?: string;
}

export interface PluginIframeNodeSnapshot {
  uuid: string;
  label?: string;
  type: string;
  config?: Record<string, unknown>;
}

export interface PluginIframeInitPayload {
  nodeType?: string;
  locale?: string;
  config?: Record<string, unknown>;
  variables?: Record<string, unknown>;
  host_api?: Record<string, unknown>;
  node_report?: NodeExecutionReport;
  outputData?: unknown;
  reportData?: unknown;
  executionId?: string;
  nodeInstanceId?: string;
  nodes?: PluginIframeNodeSnapshot[];
}

export interface PluginStreamMessage {
  call_id?: string;
  kind?: string;
  data?: unknown;
  delta_content?: string;
  delta_reasoning?: string;
  text?: string;
  fullContent?: string;
  fullReasoning?: string;
  timestamp?: string;
}
