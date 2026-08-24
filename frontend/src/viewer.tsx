import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';
import { initBridge, postError } from './bridge';
import { setLocale, t } from './i18n';
import {
  IconBot,
  IconSparkles,
  IconMessageSquare,
  IconBraces,
  IconShuffle,
  IconBrain,
  IconBarChart,
  IconCopy,
  IconCheck,
  IconCheckCircle,
  IconX,
  IconXCircle,
  IconGlobe,
  IconClock,
  IconInfo,
} from './icons';
import type {
  ChatExecutionOutput,
  EmbeddingsOutput,
  PluginIframeInitPayload,
  RerankOutput,
} from './types';

export function ViewerApp() {
  const [nodeType, setNodeType] = useState<string>('llm:chat');
  const [output, setOutput] = useState<unknown>(null);
  const [streamContent, setStreamContent] = useState<string>('');
  const [streamReasoning, setStreamReasoning] = useState<string>('');
  const [isStreaming, setIsStreaming] = useState<boolean>(false);
  const [showReasoning, setShowReasoning] = useState<boolean>(true);
  const [showRequestDetails, setShowRequestDetails] = useState<boolean>(true);
  const [copied, setCopied] = useState<boolean>(false);
  const [copiedBody, setCopiedBody] = useState<boolean>(false);
  const [ready, setReady] = useState<boolean>(false);

  useEffect(() => {
    return initBridge({
      onInit: (payload: PluginIframeInitPayload) => {
        if (payload.locale) setLocale(payload.locale);
        if (payload.nodeType) setNodeType(payload.nodeType);
        if (payload.node_report) {
          const isRunning = payload.node_report.status === 'running';
          const data = payload.node_report.plugin_data || payload.node_report.output_data;
          if (data) {
            setOutput(data);
            setIsStreaming(false);
          } else {
            setIsStreaming(isRunning);
          }
        } else if (payload.outputData) {
          setOutput(payload.outputData);
          setIsStreaming(false);
        } else if (payload.reportData) {
          setOutput(payload.reportData);
          setIsStreaming(false);
        }
        setReady(true);
      },
      onStream: (stream) => {
        setIsStreaming(true);
        if (typeof stream.fullContent === 'string') {
          setStreamContent(stream.fullContent);
        } else if (typeof stream.delta_content === 'string' && stream.delta_content) {
          setStreamContent((prev) => prev + stream.delta_content);
        } else if (typeof stream.text === 'string' && stream.kind === 'content') {
          setStreamContent((prev) => prev + stream.text);
        }

        if (typeof stream.fullReasoning === 'string') {
          setStreamReasoning(stream.fullReasoning);
        } else if (typeof stream.delta_reasoning === 'string' && stream.delta_reasoning) {
          setStreamReasoning((prev) => prev + stream.delta_reasoning);
        } else if (typeof stream.text === 'string' && stream.kind === 'reasoning') {
          setStreamReasoning((prev) => prev + stream.text);
        }
      },
    });
  }, []);

  const handleCopy = (text: string) => {
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        navigator.clipboard.writeText(text).catch(() => {});
      }
    } catch {
      // Ignored
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const hasLiveStream = Boolean(streamContent) || Boolean(streamReasoning) || isStreaming;
  if (!ready || (!output && !hasLiveStream) || (output && typeof output !== 'object')) {
    return (
      <div className="panel">
        <div className="card">
          <div className="hint" style={{ textAlign: 'center', padding: '24px' }}>
            {t('等待执行或无输出数据...', 'Waiting for execution or no output data...')}
          </div>
        </div>
      </div>
    );
  }

  const rawRecord = output as Record<string, unknown>;
  const requestInfo = (rawRecord.request as Record<string, unknown>) || null;
  const errorMsg = typeof rawRecord.error === 'string' ? rawRecord.error : null;
  const responseInfo = (rawRecord.response as Record<string, unknown>) || null;

  // Shared Request/Response Inspector Card
  const renderRequestInspector = () => {
    if (!requestInfo && !responseInfo && !errorMsg) return null;

    return (
      <div className="card">
        <div
          className="row-between"
          style={{ cursor: 'pointer' }}
          onClick={() => setShowRequestDetails(!showRequestDetails)}
        >
          <div className="card-title" style={{ color: 'var(--fg-muted)', fontSize: '12px' }}>
            <IconInfo size={14} />
            <span>{t('实际请求体与服务商信息 (HTTP Request & Details)', 'HTTP Request & Details')}</span>
          </div>
          <div className="row">
            <button
              type="button"
              className="btn btn-sm"
              onClick={(e) => {
                e.stopPropagation();
                handleCopy(JSON.stringify(output, null, 2));
              }}
            >
              {copied ? (
                <>
                  <IconCheck size={11} /> {t('已复制', 'Copied')}
                </>
              ) : (
                <>
                  <IconCopy size={11} /> {t('复制全部 JSON', 'Copy All JSON')}
                </>
              )}
            </button>
            <button type="button" className="btn btn-sm">
              {showRequestDetails ? t('收起', 'Collapse') : t('展开', 'Expand')}
            </button>
          </div>
        </div>

        {showRequestDetails && (
          <div className="col" style={{ gap: '8px', marginTop: '6px' }}>
            {requestInfo && (
              <div className="provider-info-box">
                {Boolean(requestInfo.method) && (
                  <div className="provider-info-row">
                    <span className="hint">Method:</span>
                    <span className="mono">{String(requestInfo.method || 'POST')}</span>
                  </div>
                )}
                <div className="provider-info-row">
                  <span className="hint">URL:</span>
                  <span className="mono">{String(requestInfo.url || '')}</span>
                </div>
                <div className="provider-info-row">
                  <span className="hint">Model:</span>
                  <span className="mono">{String(requestInfo.model || '')}</span>
                </div>
                <div className="provider-info-row">
                  <span className="hint">API Key:</span>
                  <span className="mono">{String(requestInfo.api_key_masked || '')}</span>
                </div>
                {Boolean(requestInfo.payload || requestInfo.body) && (
                  <div className="field" style={{ marginTop: '4px' }}>
                    <div className="row-between" style={{ marginBottom: '4px' }}>
                      <span className="field-label">{t('实际请求体 (HTTP Request Body)', 'HTTP Request Body')}</span>
                      <button
                        type="button"
                        className="btn btn-sm"
                        onClick={(e) => {
                          e.stopPropagation();
                          const bodyText = JSON.stringify(requestInfo.payload || requestInfo.body, null, 2);
                          try {
                            if (typeof navigator !== 'undefined' && navigator.clipboard) {
                              navigator.clipboard.writeText(bodyText).catch(() => {});
                            }
                          } catch {}
                          setCopiedBody(true);
                          setTimeout(() => setCopiedBody(false), 2000);
                        }}
                      >
                        {copiedBody ? (
                          <>
                            <IconCheck size={11} /> {t('已复制请求体', 'Copied Body')}
                          </>
                        ) : (
                          <>
                            <IconCopy size={11} /> {t('复制请求体', 'Copy Request Body')}
                          </>
                        )}
                      </button>
                    </div>
                    <pre className="code-pre">
                      {JSON.stringify(requestInfo.payload || requestInfo.body, null, 2)}
                    </pre>
                  </div>
                )}
              </div>
            )}

            {responseInfo && (
              <div className="field">
                <span className="field-label" style={{ color: 'var(--danger)' }}>
                  {t('服务商返回 (Response Status / Body)', 'Provider Response')}
                </span>
                <pre className="code-pre">
                  {JSON.stringify(responseInfo, null, 2)}
                </pre>
              </div>
            )}
          </div>
        )}
      </div>
    );
  };

  // If node execution ended with error
  if (errorMsg) {
    return (
      <div className="panel">
        <div className="card" style={{ borderColor: 'var(--danger)' }}>
          <div className="card-title" style={{ color: 'var(--danger)' }}>
            <IconXCircle size={16} />
            <span>{t('执行失败 (Execution Error)', 'Execution Error')}</span>
          </div>
          <div className="alert alert-red">
            <span>{errorMsg}</span>
          </div>
        </div>
        {renderRequestInspector()}
      </div>
    );
  }

  // 1. LLM Provider Node Output
  if (nodeType === 'llm:provider') {
    const provOutput = rawRecord;
    const connected = provOutput.connected === true;
    const latency = typeof provOutput.latency_ms === 'number' ? provOutput.latency_ms : 0;
    const baseUrl = String(provOutput.base_url || '');
    const model = String(provOutput.model || '');

    return (
      <div className="panel">
        <div className="metrics-bar">
          <div className="metric-card">
            <span className="metric-label">{t('连接状态', 'Connection Status')}</span>
            <span className="metric-value" style={{ color: connected ? 'var(--accent)' : 'var(--danger)', display: 'flex', alignItems: 'center', gap: '4px' }}>
              {connected ? <IconCheckCircle size={14} /> : <IconXCircle size={14} />}
              {connected ? t('已连接 / 已注册', 'Connected') : t('连接失败', 'Failed')}
            </span>
          </div>
          <div className="metric-card">
            <span className="metric-label">{t('响应延迟', 'Latency')}</span>
            <span className="metric-value">{latency}ms</span>
          </div>
          <div className="metric-card">
            <span className="metric-label">{t('默认模型', 'Default Model')}</span>
            <span className="metric-value mono" style={{ fontSize: '12px' }}>{model}</span>
          </div>
        </div>

        <div className="card">
          <div className="card-title">
            <IconBot size={16} />
            <span>{t('服务商注册信息 (Provider Details)', 'Provider Details')}</span>
          </div>
          <div className="provider-info-box">
            <div className="provider-info-row">
              <span className="hint row" style={{ gap: '4px' }}>
                <IconGlobe size={12} />
                <span>Base URL:</span>
              </span>
              <span className="mono">{baseUrl}</span>
            </div>
            <div className="provider-info-row">
              <span className="hint row" style={{ gap: '4px' }}>
                <IconSparkles size={12} />
                <span>Model:</span>
              </span>
              <span className="mono">{model}</span>
            </div>
            <div className="provider-info-row">
              <span className="hint row" style={{ gap: '4px' }}>
                <IconClock size={12} />
                <span>Latency:</span>
              </span>
              <span className="mono">{latency}ms</span>
            </div>
          </div>
        </div>

        {renderRequestInspector()}
      </div>
    );
  }

  // 2. Embeddings Node Output
  if (nodeType === 'llm:embeddings') {
    const embOutput = output as EmbeddingsOutput;
    const vectors = embOutput.data || [];
    const dim = vectors[0]?.length || 0;

    return (
      <div className="panel">
        <div className="metrics-bar">
          <div className="metric-card">
            <span className="metric-label">{t('向量数量', 'Vector Count')}</span>
            <span className="metric-value">{vectors.length}</span>
          </div>
          <div className="metric-card">
            <span className="metric-label">{t('向量维度', 'Dimensions')}</span>
            <span className="metric-value">{dim}</span>
          </div>
          <div className="metric-card">
            <span className="metric-label">{t('消耗 Token', 'Total Tokens')}</span>
            <span className="metric-value">{embOutput.usage?.total_tokens || 0}</span>
          </div>
          <div className="metric-card">
            <span className="metric-label">{t('总耗时', 'Latency')}</span>
            <span className="metric-value">{embOutput.latency_ms?.total_ms || 0}ms</span>
          </div>
        </div>

        <div className="card">
          <div className="row-between">
            <div className="card-title">
              <IconBarChart size={16} />
              <span>{t('向量数据预览 (Embeddings Data)', 'Embeddings Preview')}</span>
            </div>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => handleCopy(JSON.stringify(vectors, null, 2))}
            >
              {copied ? (
                <>
                  <IconCheck size={12} /> {t('已复制', 'Copied')}
                </>
              ) : (
                <>
                  <IconCopy size={12} /> {t('复制向量 JSON', 'Copy Vectors')}
                </>
              )}
            </button>
          </div>
          <div className="col">
            {vectors.map((vec, i) => (
              <div key={i} className="field">
                <span className="hint">Vector #{i + 1} ({vec.length} dimensions):</span>
                <pre className="code-pre">
                  {JSON.stringify(vec.slice(0, 8))} {vec.length > 8 ? `... +${vec.length - 8} items` : ''}
                </pre>
              </div>
            ))}
          </div>
        </div>

        {renderRequestInspector()}
      </div>
    );
  }

  // 3. Rerank Node Output
  if (nodeType === 'llm:rerank') {
    const rerankOutput = output as RerankOutput;
    const results = rerankOutput.results || [];

    return (
      <div className="panel">
        <div className="metrics-bar">
          <div className="metric-card">
            <span className="metric-label">{t('候选文档数', 'Candidates')}</span>
            <span className="metric-value">{results.length}</span>
          </div>
          <div className="metric-card">
            <span className="metric-label">{t('总耗时', 'Latency')}</span>
            <span className="metric-value">{rerankOutput.latency_ms?.total_ms || 0}ms</span>
          </div>
          {rerankOutput.usage && (
            <div className="metric-card">
              <span className="metric-label">{t('消耗 Token', 'Tokens')}</span>
              <span className="metric-value">{rerankOutput.usage.total_tokens || 0}</span>
            </div>
          )}
        </div>

        <div className="card">
          <div className="card-title">
            <IconShuffle size={16} />
            <span>{t('重排结果 (Ranked Results)', 'Ranked Results')}</span>
          </div>
          <div className="col">
            {results.map((item, idx) => (
              <div key={idx} className="rerank-card">
                <div className="row-between">
                  <div className="row">
                    <span className="badge badge-blue">Rank #{idx + 1}</span>
                    <span className="hint">(Orig #{item.index + 1})</span>
                  </div>
                  <span className="metric-value" style={{ fontSize: '13px', color: 'var(--accent)' }}>
                    Score: {(item.relevance_score).toFixed(4)}
                  </span>
                </div>
                <div
                  className="score-bar"
                  style={{ width: `${Math.max(0, Math.min(100, item.relevance_score * 100))}%` }}
                />
                {item.document && (
                  <div className="hint" style={{ color: 'var(--fg)', marginTop: '4px' }}>
                    {item.document}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>

        {renderRequestInspector()}
      </div>
    );
  }

  // 4. Chat / Structured node output
  const chatOutput = (output as ChatExecutionOutput) || {
    content: streamContent,
    reasoning_content: streamReasoning || undefined,
    usage: { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 },
    latency_ms: { total_ms: 0 },
  };
  const effectiveContent = (output ? (output as ChatExecutionOutput).content : streamContent) || streamContent;
  const effectiveReasoning = (output ? (output as ChatExecutionOutput).reasoning_content : streamReasoning) || streamReasoning;
  const usage = chatOutput.usage || { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
  const latency = chatOutput.latency_ms || { total_ms: 0 };
  const isActuallyStreaming = isStreaming && !output;

  return (
    <div className="panel">
      <div className="metrics-bar">
        {isActuallyStreaming && (
          <div className="metric-card" style={{ borderColor: 'var(--accent)' }}>
            <span className="metric-label">{t('状态', 'Status')}</span>
            <span className="metric-value" style={{ color: 'var(--accent)', display: 'flex', alignItems: 'center', gap: '4px' }}>
              <IconSparkles size={13} />
              <span style={{ fontSize: '12px' }}>{t('实时生成中...', 'Streaming...')}</span>
            </span>
          </div>
        )}
        {latency.ttft_ms !== undefined && latency.ttft_ms !== null && (
          <div className="metric-card">
            <span className="metric-label">{t('首字延迟 (TTFT)', 'TTFT')}</span>
            <span className="metric-value">{latency.ttft_ms}ms</span>
          </div>
        )}
        {latency.total_ms > 0 && (
          <div className="metric-card">
            <span className="metric-label">{t('总耗时', 'Latency')}</span>
            <span className="metric-value">{latency.total_ms}ms</span>
          </div>
        )}
        {usage.prompt_tokens > 0 && (
          <div className="metric-card">
            <span className="metric-label">{t('输入 Token', 'Prompt Tokens')}</span>
            <span className="metric-value">{usage.prompt_tokens}</span>
          </div>
        )}
        {usage.completion_tokens > 0 && (
          <div className="metric-card">
            <span className="metric-label">{t('输出 Token', 'Completion Tokens')}</span>
            <span className="metric-value">{usage.completion_tokens}</span>
          </div>
        )}
        {usage.reasoning_tokens ? (
          <div className="metric-card">
            <span className="metric-label">{t('思考 Token', 'Reasoning Tokens')}</span>
            <span className="metric-value">{usage.reasoning_tokens}</span>
          </div>
        ) : null}
        {usage.total_tokens > 0 && (
          <div className="metric-card">
            <span className="metric-label">{t('总 Token', 'Total Tokens')}</span>
            <span className="metric-value">{usage.total_tokens}</span>
          </div>
        )}
      </div>

      {effectiveReasoning && (
        <div className="card">
          <div className="row-between" style={{ cursor: 'pointer' }} onClick={() => setShowReasoning(!showReasoning)}>
            <div className="card-title" style={{ color: 'var(--fg-muted)' }}>
              <IconBrain size={16} />
              <span>{t('深度思考过程 (Thinking Process)', 'Thinking Process')}</span>
            </div>
            <button type="button" className="btn btn-sm">
              {showReasoning ? t('收起', 'Collapse') : t('展开', 'Expand')}
            </button>
          </div>
          {showReasoning && (
            <div className="reasoning-box">
              {effectiveReasoning}
              {isActuallyStreaming && !effectiveContent && (
                <span className="typewriter-cursor typewriter-cursor-purple" />
              )}
            </div>
          )}
        </div>
      )}

      {nodeType === 'llm:structured' && (
        <div className="card">
          <div className="row-between">
            <div className="row">
              <div className="card-title">
                <IconBraces size={16} />
                <span>{t('结构化 JSON 提取结果', 'Structured JSON Output')}</span>
              </div>
              {chatOutput.schema_valid === true && (
                <span className="badge badge-green">
                  <IconCheck size={11} /> {t('模式校验通过', 'Schema Valid')}
                </span>
              )}
              {chatOutput.schema_valid === false && (
                <span className="badge badge-red">
                  <IconX size={11} /> {t('模式校验失败', 'Schema Invalid')}
                </span>
              )}
            </div>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => handleCopy(JSON.stringify(chatOutput.parsed_json, null, 2))}
            >
              {copied ? (
                <>
                  <IconCheck size={12} /> {t('已复制', 'Copied')}
                </>
              ) : (
                <>
                  <IconCopy size={12} /> {t('复制 JSON', 'Copy JSON')}
                </>
              )}
            </button>
          </div>

          {chatOutput.schema_errors && chatOutput.schema_errors.length > 0 && (
            <div className="alert alert-red">
              <div className="col">
                <strong>{t('Schema 校验错误列表:', 'Schema Validation Errors:')}</strong>
                {chatOutput.schema_errors.map((err, i) => (
                  <span key={i}>• {err}</span>
                ))}
              </div>
            </div>
          )}

          <pre className="code-pre">
            {JSON.stringify(chatOutput.parsed_json || chatOutput.content, null, 2)}
          </pre>
        </div>
      )}

      {nodeType !== 'llm:structured' && (
        <div className="card">
          <div className="row-between">
            <div className="card-title">
              <IconMessageSquare size={16} />
              <span>{t('生成回答 (Response)', 'Response Content')}</span>
            </div>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => handleCopy(chatOutput.content)}
            >
              {copied ? (
                <>
                  <IconCheck size={12} /> {t('已复制', 'Copied')}
                </>
              ) : (
                <>
                  <IconCopy size={12} /> {t('复制内容', 'Copy Content')}
                </>
              )}
            </button>
          </div>
          <div className="output-box">
            {effectiveContent}
            {isActuallyStreaming && (
              <span className="typewriter-cursor" />
            )}
          </div>

          {Boolean(chatOutput.parsed_json) && (
            <div className="field" style={{ marginTop: '8px' }}>
              <span className="field-label">{t('解析的 JSON 对象 (Parsed JSON)', 'Parsed JSON Object')}</span>
              <pre className="code-pre">
                {JSON.stringify(chatOutput.parsed_json, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}

      {renderRequestInspector()}
    </div>
  );
}

function renderViewer(): void {
  const container = document.getElementById('root');
  if (!container) return;
  createRoot(container).render(
    <StrictMode>
      <ViewerApp />
    </StrictMode>,
  );
}

function main(): void {
  window.addEventListener('error', (event) => {
    postError(event.message || String(event.error ?? 'unknown viewer error'));
  });
  window.addEventListener('unhandledrejection', (event) => {
    const reason = event.reason;
    postError(reason instanceof Error ? reason.message : String(reason ?? 'unhandled rejection'));
  });
  const container = document.getElementById('root');
  if (container) {
    renderViewer();
    return;
  }
  document.addEventListener('DOMContentLoaded', renderViewer);
}

main();
