import { StrictMode, useCallback, useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';
import { initBridge, notifyConfig, postError, uiCall } from './bridge';
import { setLocale, t } from './i18n';
import {
  IconBot,
  IconSparkles,
  IconMessageSquare,
  IconBraces,
  IconLayers,
  IconShuffle,
  IconSettings,
  IconCheck,
  IconCheckCircle,
  IconXCircle,
  IconPlus,
  IconTrash,
  IconKey,
  IconGlobe,
  IconClock,
  IconInfo,
} from './icons';
import type { ChatMessage, LlmProviderConfig, PluginIframeInitPayload, PluginIframeNodeSnapshot } from './types';

const PROVIDER_PRESETS: Record<string, { label: string; base_url: string; default_model: string }> = {
  openai: {
    label: 'OpenAI',
    base_url: 'https://api.openai.com/v1',
    default_model: 'gpt-4o',
  },
  deepseek: {
    label: 'DeepSeek',
    base_url: 'https://api.deepseek.com/v1',
    default_model: 'deepseek-chat',
  },
  deepseek_r1: {
    label: 'DeepSeek (R1 Reasoner)',
    base_url: 'https://api.deepseek.com/v1',
    default_model: 'deepseek-reasoner',
  },
  siliconflow: {
    label: 'SiliconFlow (硅基流动)',
    base_url: 'https://api.siliconflow.cn/v1',
    default_model: 'deepseek-ai/DeepSeek-V3',
  },
  qwen: {
    label: 'Qwen (通义千问 / DashScope)',
    base_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    default_model: 'qwen-plus',
  },
  ollama: {
    label: 'Ollama (Local)',
    base_url: 'http://localhost:11434/v1',
    default_model: 'llama3:latest',
  },
  vllm: {
    label: 'vLLM / Local OpenAI',
    base_url: 'http://localhost:8000/v1',
    default_model: 'default',
  },
  custom: {
    label: 'Custom Provider',
    base_url: '',
    default_model: '',
  },
};

const SCHEMA_TEMPLATES: Record<string, { label: string; schema: Record<string, unknown> }> = {
  entity: {
    label: 'Entity Extraction (实体提取)',
    schema: {
      type: 'object',
      properties: {
        entities: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              name: { type: 'string' },
              category: { type: 'string' },
              confidence: { type: 'number' },
            },
            required: ['name', 'category'],
          },
        },
      },
      required: ['entities'],
    },
  },
  classify: {
    label: 'Classification & Intent (意图分类)',
    schema: {
      type: 'object',
      properties: {
        category: { type: 'string' },
        sub_category: { type: 'string' },
        sentiment: { type: 'string', enum: ['positive', 'neutral', 'negative'] },
        confidence: { type: 'number' },
        reasoning: { type: 'string' },
      },
      required: ['category', 'sentiment', 'confidence'],
    },
  },
  summary: {
    label: 'Summary & Key Points (摘要与要点)',
    schema: {
      type: 'object',
      properties: {
        title: { type: 'string' },
        summary: { type: 'string' },
        key_points: {
          type: 'array',
          items: { type: 'string' },
        },
      },
      required: ['title', 'summary', 'key_points'],
    },
  },
};

type PanelState = {
  nodeType: string;
  config: Record<string, unknown>;
  nodes: PluginIframeNodeSnapshot[] | undefined;
  ready: boolean;
};

const initialDefaultConfig: Record<string, unknown> = {
  base_url: 'https://api.openai.com/v1',
  api_key: '',
  model: 'gpt-4o',
  timeout_ms: 60000,
  provider_uuid: '',
  override_model: '',
  provider: {
    base_url: 'https://api.openai.com/v1',
    api_key: '',
    model: 'gpt-4o',
    timeout_ms: 60000,
  },
  messages: [
    { role: 'system', content: 'You are a helpful assistant.' },
    { role: 'user', content: '' },
  ],
  parameters: {
    temperature: 0.7,
    stream: true,
  },
  json_schema: {
    type: 'object',
    properties: {
      summary: { type: 'string' },
    },
    required: ['summary'],
  },
  strict_validation: true,
  input: '',
  query: '',
  documents: [],
  top_n: 3,
  return_documents: true,
};

export function PanelApp() {
  const [state, setState] = useState<PanelState>({
    nodeType: 'llm:chat',
    config: initialDefaultConfig,
    nodes: undefined,
    ready: false,
  });

  const [testStatus, setTestStatus] = useState<{ ok?: boolean; message?: string; loading?: boolean }>({});
  const [modelsList, setModelsList] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [schemaText, setSchemaText] = useState<string>('');
  const [schemaError, setSchemaError] = useState<string | null>(null);
  const [documentsText, setDocumentsText] = useState<string>('');
  const [embeddingsInputText, setEmbeddingsInputText] = useState<string>('');

  useEffect(() => {
    return initBridge({
      onInit: (payload: PluginIframeInitPayload) => {
        if (payload.locale) setLocale(payload.locale);
        const incomingConfig = payload.config && typeof payload.config === 'object'
          ? (payload.config as Record<string, unknown>)
          : initialDefaultConfig;

        setState({
          nodeType: payload.nodeType || 'llm:chat',
          config: incomingConfig,
          nodes: payload.nodes,
          ready: true,
        });

        if (incomingConfig.json_schema) {
          setSchemaText(JSON.stringify(incomingConfig.json_schema, null, 2));
        }
        if (Array.isArray(incomingConfig.documents)) {
          setDocumentsText(incomingConfig.documents.join('\n'));
        }
        if (incomingConfig.input !== undefined && incomingConfig.input !== null) {
          setEmbeddingsInputText(
            typeof incomingConfig.input === 'string'
              ? incomingConfig.input
              : JSON.stringify(incomingConfig.input, null, 2),
          );
        }
      },
      onConfigUpdated: (next: unknown) => {
        if (next && typeof next === 'object') {
          const nextObj = next as Record<string, unknown>;
          setState((prev) => ({ ...prev, config: nextObj }));
          if (Array.isArray(nextObj.documents) && documentsText === '') {
            setDocumentsText(nextObj.documents.join('\n'));
          }
          if (nextObj.input !== undefined && embeddingsInputText === '') {
            setEmbeddingsInputText(
              typeof nextObj.input === 'string' ? nextObj.input : JSON.stringify(nextObj.input, null, 2),
            );
          }
        }
      },
    });
  }, []);

  const updateConfig = useCallback((updater: (prev: Record<string, unknown>) => Record<string, unknown>) => {
    setState((prev) => {
      const nextConfig = updater(prev.config);
      notifyConfig(nextConfig);
      return { ...prev, config: nextConfig };
    });
  }, []);

  const config = state.config;
  const nodeType = state.nodeType;
  const isProviderNode = nodeType === 'llm:provider';

  // Available provider nodes in the current flow
  const availableProviderNodes = useMemo(() => {
    if (!state.nodes || !Array.isArray(state.nodes)) return [];
    return state.nodes.filter((n) => n.type === 'llm:provider');
  }, [state.nodes]);

  // If node references a provider_uuid, find that node's snapshot
  const selectedProviderNode = useMemo(() => {
    const uuid = String(config.provider_uuid || '');
    if (!uuid) return null;
    return availableProviderNodes.find((n) => n.uuid === uuid) || null;
  }, [config.provider_uuid, availableProviderNodes]);

  // Provider configuration (inline or direct for llm:provider)
  const provider: LlmProviderConfig = isProviderNode
    ? {
        base_url: String(config.base_url || 'https://api.openai.com/v1'),
        api_key: config.api_key ? String(config.api_key) : '',
        model: String(config.model || 'gpt-4o'),
        timeout_ms: typeof config.timeout_ms === 'number' ? config.timeout_ms : 60000,
      }
    : (config.provider as LlmProviderConfig) || {
        base_url: 'https://api.openai.com/v1',
        api_key: '',
        model: 'gpt-4o',
        timeout_ms: 60000,
      };

  const messages = (config.messages as ChatMessage[]) || [];
  const parameters = (config.parameters as Record<string, unknown>) || {};

  const handleProviderPreset = (presetKey: string) => {
    const preset = PROVIDER_PRESETS[presetKey];
    if (!preset) return;
    if (isProviderNode) {
      updateConfig((prev) => ({
        ...prev,
        base_url: preset.base_url,
        model: preset.default_model || String(prev.model || 'gpt-4o'),
      }));
    } else {
      updateConfig((prev) => ({
        ...prev,
        provider: {
          ...(prev.provider as Record<string, unknown>),
          base_url: preset.base_url,
          model: preset.default_model || (prev.provider as Record<string, unknown>)?.model || 'gpt-4o',
        },
      }));
    }
  };

  const handleTestConnection = async () => {
    setTestStatus({ loading: true });
    try {
      const res = (await uiCall('llm.test_connection', { provider })) as {
        ok: boolean;
        latency_ms: number;
        models_count: number;
      };
      setTestStatus({
        ok: true,
        message: t(
          `连接成功！延迟: ${res.latency_ms}ms, 发现模型: ${res.models_count} 个`,
          `Connected! Latency: ${res.latency_ms}ms, Models found: ${res.models_count}`,
        ),
      });
    } catch (err: unknown) {
      setTestStatus({
        ok: false,
        message: String(err instanceof Error ? err.message : err),
      });
    }
  };

  const handleFetchModels = async () => {
    setLoadingModels(true);
    try {
      const res = (await uiCall('llm.list_models', { provider })) as { models: string[] };
      if (res && Array.isArray(res.models)) {
        setModelsList(res.models);
      }
    } catch (err: unknown) {
      alert(String(err instanceof Error ? err.message : err));
    } finally {
      setLoadingModels(false);
    }
  };

  const updateMessage = (index: number, field: keyof ChatMessage, val: string) => {
    const nextMsgs = [...messages];
    nextMsgs[index] = { ...nextMsgs[index], [field]: val };
    updateConfig((prev) => ({ ...prev, messages: nextMsgs }));
  };

  const addMessage = (role: 'system' | 'user' | 'assistant') => {
    updateConfig((prev) => ({
      ...prev,
      messages: [...messages, { role, content: '' }],
    }));
  };

  const deleteMessage = (index: number) => {
    const nextMsgs = messages.filter((_, i) => i !== index);
    updateConfig((prev) => ({ ...prev, messages: nextMsgs }));
  };

  const handleSchemaChange = (text: string) => {
    setSchemaText(text);
    try {
      const parsed = JSON.parse(text);
      setSchemaError(null);
      updateConfig((prev) => ({ ...prev, json_schema: parsed }));
    } catch (e: unknown) {
      setSchemaError(String(e instanceof Error ? e.message : e));
    }
  };

  const handleApplySchemaTemplate = (templateKey: string) => {
    const tmpl = SCHEMA_TEMPLATES[templateKey];
    if (!tmpl) return;
    const text = JSON.stringify(tmpl.schema, null, 2);
    setSchemaText(text);
    setSchemaError(null);
    updateConfig((prev) => ({ ...prev, json_schema: tmpl.schema }));
  };

  if (!state.ready) {
    return (
      <div className="panel">
        <div className="card">
          <div className="hint" style={{ textAlign: 'center', padding: '16px' }}>
            {t('正在加载配置面板...', 'Loading configuration panel...')}
          </div>
        </div>
      </div>
    );
  }

  // Common provider form section (used by llm:provider or inline custom mode)
  const renderProviderFormFields = (isInline: boolean) => (
    <>
      <div className="field">
        <label className="field-label">{t('服务商预设', 'Provider Preset')}</label>
        <select
          className="select"
          onChange={(e) => handleProviderPreset(e.target.value)}
          defaultValue=""
        >
          <option value="" disabled>
            {t('-- 选择快捷预设 --', '-- Select Provider Preset --')}
          </option>
          {Object.entries(PROVIDER_PRESETS).map(([key, item]) => (
            <option key={key} value={key}>
              {item.label}
            </option>
          ))}
        </select>
      </div>

      <div className="grid-2">
        <div className="field">
          <label className="field-label">
            <span className="row" style={{ gap: '4px' }}>
              <IconGlobe size={13} className="text-muted" />
              <span>{t('API 基础地址 (Base URL)', 'Base URL')}</span>
            </span>
            <span className="req">*</span>
          </label>
          <input
            type="text"
            className="input mono"
            placeholder="https://api.openai.com/v1"
            value={provider.base_url || ''}
            onChange={(e) => {
              const val = e.target.value;
              if (isInline) {
                updateConfig((prev) => ({
                  ...prev,
                  provider: { ...(prev.provider as Record<string, unknown>), base_url: val },
                }));
              } else {
                updateConfig((prev) => ({ ...prev, base_url: val }));
              }
            }}
          />
        </div>

        <div className="field">
          <label className="field-label">
            <span className="row" style={{ gap: '4px' }}>
              <IconKey size={13} className="text-muted" />
              <span>{t('API 密钥 (API Key)', 'API Key')}</span>
            </span>
          </label>
          <input
            type="password"
            className="input mono"
            placeholder="sk-..."
            value={provider.api_key || ''}
            onChange={(e) => {
              const val = e.target.value;
              if (isInline) {
                updateConfig((prev) => ({
                  ...prev,
                  provider: { ...(prev.provider as Record<string, unknown>), api_key: val },
                }));
              } else {
                updateConfig((prev) => ({ ...prev, api_key: val }));
              }
            }}
          />
        </div>
      </div>

      <div className="field">
        <div className="row-between" style={{ marginBottom: '2px' }}>
          <label className="field-label" style={{ margin: 0 }}>
            <span className="row" style={{ gap: '4px' }}>
              <IconSparkles size={13} className="text-muted" />
              <span>{t('模型名称 (Model)', 'Model Name')}</span>
            </span>
            <span className="req">*</span>
          </label>
          <div className="row" style={{ gap: '6px' }}>
            <button
              type="button"
              className="btn btn-sm"
              onClick={handleFetchModels}
              disabled={loadingModels}
            >
              {loadingModels ? t('拉取中...', 'Fetching...') : t('获取模型列表', 'Fetch Models')}
            </button>
            <button
              type="button"
              className="btn btn-sm btn-primary"
              onClick={handleTestConnection}
              disabled={testStatus.loading}
            >
              {testStatus.loading ? t('测试中...', 'Testing...') : t('测试连接', 'Test Connection')}
            </button>
          </div>
        </div>

        {modelsList.length > 0 ? (
          <select
            className="select mono"
            value={provider.model || ''}
            onChange={(e) => {
              const val = e.target.value;
              if (isInline) {
                updateConfig((prev) => ({
                  ...prev,
                  provider: { ...(prev.provider as Record<string, unknown>), model: val },
                }));
              } else {
                updateConfig((prev) => ({ ...prev, model: val }));
              }
            }}
          >
            {modelsList.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        ) : (
          <input
            type="text"
            className="input mono"
            placeholder="gpt-4o, deepseek-chat, qwen-plus..."
            value={provider.model || ''}
            onChange={(e) => {
              const val = e.target.value;
              if (isInline) {
                updateConfig((prev) => ({
                  ...prev,
                  provider: { ...(prev.provider as Record<string, unknown>), model: val },
                }));
              } else {
                updateConfig((prev) => ({ ...prev, model: val }));
              }
            }}
          />
        )}
      </div>

      {!isInline && (
        <div className="field">
          <label className="field-label">
            <span className="row" style={{ gap: '4px' }}>
              <IconClock size={13} className="text-muted" />
              <span>{t('请求超时 (Timeout ms)', 'Timeout (ms)')}</span>
            </span>
          </label>
          <input
            type="number"
            className="input mono"
            placeholder="60000"
            value={typeof config.timeout_ms === 'number' ? config.timeout_ms : 60000}
            onChange={(e) => updateConfig((prev) => ({ ...prev, timeout_ms: parseInt(e.target.value, 10) || 60000 }))}
          />
        </div>
      )}

      {testStatus.message && (
        <div className={`alert ${testStatus.ok ? 'alert-green' : 'alert-red'}`}>
          {testStatus.ok ? <IconCheckCircle size={14} /> : <IconXCircle size={14} />}
          <span>{testStatus.message}</span>
        </div>
      )}
    </>
  );

  return (
    <div className="panel">
      {/* 1. If this node is `llm:provider`: Render Provider Node Configuration */}
      {isProviderNode && (
        <div className="card">
          <div className="card-title">
            <IconBot size={16} />
            <span>{t('LLM 服务商连接设置 (Provider)', 'LLM Provider Configuration')}</span>
          </div>
          <p className="card-desc">
            {t('配置全局/可复用的 LLM 服务商连接，流程中后续对话、提取、向量、重排节点可直接选择本服务商。', 'Configure reusable LLM Provider. Subsequent chat, structured, embeddings, and rerank nodes in the flow can reference this provider.')}
          </p>
          {renderProviderFormFields(false)}
        </div>
      )}

      {/* 2. If this node is NOT `llm:provider`: Render Provider Selector / Inline Configuration Card */}
      {!isProviderNode && (
        <div className="card">
          <div className="card-title">
            <IconBot size={16} />
            <span>{t('模型服务商配置 (Provider)', 'Model Provider Configuration')}</span>
          </div>

          <div className="field">
            <label className="field-label">
              <span>{t('选择服务商节点 (Provider Reference)', 'Select Provider Node')}</span>
              {availableProviderNodes.length > 0 && (
                <span className="badge-emerald">
                  <IconCheck size={11} />
                  {availableProviderNodes.length} {t('个可用 Provider 节点', 'Provider nodes available')}
                </span>
              )}
            </label>
            <select
              className="select"
              value={String(config.provider_uuid || '')}
              onChange={(e) => {
                const val = e.target.value;
                updateConfig((prev) => ({ ...prev, provider_uuid: val }));
              }}
            >
              <option value="">
                {t('-- 自定义 / 节点内置配置 (Inline Provider Config) --', '-- Inline / Custom Provider Config --')}
              </option>
              {availableProviderNodes.map((n) => {
                const provConfig = (n.config || {}) as Record<string, unknown>;
                const modelName = String(provConfig.model || 'Default');
                return (
                  <option key={n.uuid} value={n.uuid}>
                    {n.label || n.uuid} ({modelName})
                  </option>
                );
              })}
            </select>
          </div>

          {/* When referencing a provider node */}
          {Boolean(config.provider_uuid) && (
            <div className="provider-info-box">
              <div className="provider-info-row">
                <span className="hint">{t('已绑定服务商节点:', 'Linked Provider Node:')}</span>
                <span className="mono" style={{ fontWeight: 600, color: 'var(--accent)', wordBreak: 'break-all' }}>
                  {selectedProviderNode?.label || String(config.provider_uuid || '')}
                </span>
              </div>
              {selectedProviderNode?.config && (
                <div className="provider-info-row">
                  <span className="hint">{t('服务商地址 / 默认模型:', 'Base URL / Default Model:')}</span>
                  <span className="mono text-muted" style={{ wordBreak: 'break-all' }}>
                    {String(selectedProviderNode.config.base_url || '')} ({String(selectedProviderNode.config.model || '')})
                  </span>
                </div>
              )}
              <div className="field" style={{ marginTop: '4px' }}>
                <label className="field-label">
                  <span className="row" style={{ gap: '4px' }}>
                    <IconSparkles size={12} className="text-muted" />
                    <span>{t('覆盖模型名称 (可选，留空则使用服务商默认模型)', 'Override Model (Optional)')}</span>
                  </span>
                </label>
                <input
                  type="text"
                  className="input mono input-sm"
                  placeholder={selectedProviderNode?.config?.model ? `如: ${selectedProviderNode.config.model}` : 'gpt-4o, deepseek-chat...'}
                  value={String(config.override_model || '')}
                  onChange={(e) => updateConfig((prev) => ({ ...prev, override_model: e.target.value }))}
                />
              </div>
            </div>
          )}

          {/* When NOT referencing a provider node (Inline configuration) */}
          {!config.provider_uuid && (
            <>
              {availableProviderNodes.length === 0 && (
                <div className="hint row" style={{ gap: '4px', color: 'var(--fg-muted)' }}>
                  <IconInfo size={13} />
                  <span>{t('提示：可在流程中添加一个 LLM Provider 节点统一管理服务商，无需在每个节点重复填写。', 'Tip: You can add an LLM Provider node in the flow to manage connections centrally.')}</span>
                </div>
              )}
              {renderProviderFormFields(true)}
            </>
          )}
        </div>
      )}

      {/* 3. Messages Card (for llm:chat and llm:structured) */}
      {(nodeType === 'llm:chat' || nodeType === 'llm:structured') && (
        <div className="card">
          <div className="row-between">
            <div className="card-title">
              <IconMessageSquare size={16} />
              <span>{t('对话消息流 (Messages)', 'Chat Messages')}</span>
            </div>
            <div className="row" style={{ gap: '4px' }}>
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => addMessage('system')}
              >
                <IconPlus size={12} /> System
              </button>
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => addMessage('user')}
              >
                <IconPlus size={12} /> User
              </button>
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => addMessage('assistant')}
              >
                <IconPlus size={12} /> Assistant
              </button>
            </div>
          </div>

          <div className="col">
            {messages.map((msg, index) => (
              <div key={index} className={`msg-item msg-role-${msg.role}`}>
                <div className="row-between">
                  <div className="row">
                    <select
                      className="select select-sm"
                      value={msg.role}
                      onChange={(e) => updateMessage(index, 'role', e.target.value as ChatMessage['role'])}
                    >
                      <option value="system">System</option>
                      <option value="user">User</option>
                      <option value="assistant">Assistant</option>
                      <option value="tool">Tool</option>
                    </select>
                    <span className="hint">#{index + 1}</span>
                  </div>
                  <button
                    type="button"
                    className="btn btn-sm btn-danger-hover"
                    onClick={() => deleteMessage(index)}
                    title={t('删除消息', 'Delete message')}
                  >
                    <IconTrash size={13} />
                  </button>
                </div>
                <textarea
                  className="textarea mono"
                  placeholder={t(
                    '输入消息内容，支持引用插值如 {{steps.node1.output}}...',
                    'Enter message content, supports {{steps.node1.output}}...',
                  )}
                  value={msg.content}
                  onChange={(e) => updateMessage(index, 'content', e.target.value)}
                />
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 4. JSON Schema Editor Card (for llm:structured) */}
      {nodeType === 'llm:structured' && (
        <div className="card">
          <div className="row-between">
            <div className="card-title">
              <IconBraces size={16} />
              <span>{t('结构化 JSON Schema 约束', 'Structured JSON Schema')}</span>
            </div>
            <select
              className="select select-sm"
              style={{ width: 'auto', maxWidth: '100%' }}
              onChange={(e) => handleApplySchemaTemplate(e.target.value)}
              defaultValue=""
            >
              <option value="" disabled>
                {t('-- 插入常用模板 --', '-- Insert Template --')}
              </option>
              {Object.entries(SCHEMA_TEMPLATES).map(([k, v]) => (
                <option key={k} value={k}>
                  {v.label}
                </option>
              ))}
            </select>
          </div>

          <div className="field">
            <textarea
              className="textarea mono"
              style={{ minHeight: '160px' }}
              value={schemaText || JSON.stringify(config.json_schema, null, 2)}
              onChange={(e) => handleSchemaChange(e.target.value)}
            />
            {schemaError && (
              <span className="hint req">{schemaError}</span>
            )}
          </div>

          <div className="row-between">
            <label className="switch">
              <input
                type="checkbox"
                checked={Boolean(config.strict_validation ?? true)}
                onChange={(e) => updateConfig((prev) => ({ ...prev, strict_validation: e.target.checked }))}
              />
              <span className="switch-track" />
              <span>{t('严格模式校验 (不符 Schema 则路由到 false 失败端口)', 'Strict validation (route to false port on schema mismatch)')}</span>
            </label>
          </div>
        </div>
      )}

      {/* 5. Embeddings Input Card (for llm:embeddings) */}
      {nodeType === 'llm:embeddings' && (
        <div className="card">
          <div className="card-title">
            <IconLayers size={16} />
            <span>{t('嵌入向量输入 (Embeddings Input)', 'Embeddings Input')}</span>
          </div>
          <p className="card-desc">
            {t('输入需要计算嵌入向量的文本内容，支持直接输入单段文本、多段 JSON 数组，或引用流程变量。', 'Enter text to calculate vector embeddings, supports single text, JSON array of strings, or flow variables.')}
          </p>
          <div className="field">
            <label className="field-label">
              <span>{t('文本内容或 JSON 数组 (Text or JSON Array)', 'Text string or JSON array')}</span>
              <span className="req">*</span>
            </label>
            <textarea
              className="textarea mono"
              style={{ minHeight: '120px' }}
              placeholder={t('输入需要计算嵌入向量的文本，如“人工智能发展趋势”，或变量 {{steps.fetch.text}}', 'Enter text to embed, e.g. "AI technology trends", or variable {{steps.fetch.text}}')}
              value={embeddingsInputText}
              onChange={(e) => {
                const val = e.target.value;
                setEmbeddingsInputText(val);
                try {
                  const parsed = JSON.parse(val);
                  if (Array.isArray(parsed)) {
                    updateConfig((prev) => ({ ...prev, input: parsed }));
                    return;
                  }
                } catch {
                  // Keep as raw string
                }
                updateConfig((prev) => ({ ...prev, input: val }));
              }}
            />
          </div>
        </div>
      )}

      {/* 6. Rerank Input Card (for llm:rerank) */}
      {nodeType === 'llm:rerank' && (
        <div className="card">
          <div className="card-title">
            <IconShuffle size={16} />
            <span>{t('文档重排设置 (Rerank Configuration)', 'Rerank Configuration')}</span>
          </div>
          <p className="card-desc">
            {t('使用 Cross-Encoder 重排模型根据 Query（查询词）对候选文档列表进行语义相关度评分并降序排列。', 'Use cross-encoder model to score and re-rank candidate documents against the query.')}
          </p>
          <div className="field">
            <label className="field-label">
              <span>{t('查询语句 (Query / 检索词)', 'Query String')}</span>
              <span className="req">*</span>
            </label>
            <input
              type="text"
              className="input"
              placeholder={t('输入搜索查询语句，如“如何配置流程执行器”，或引用 {{steps.input.query}}', 'Enter search query, e.g. "How to configure flow executor", or {{steps.input.query}}')}
              value={String(config.query || '')}
              onChange={(e) => updateConfig((prev) => ({ ...prev, query: e.target.value }))}
            />
          </div>
          <div className="field">
            <label className="field-label">
              <span>{t('候选文档列表 (每行一个文档或 JSON 数组)', 'Candidate Documents (one per line or JSON array)')}</span>
              <span className="req">*</span>
            </label>
            <textarea
              className="textarea mono"
              style={{ minHeight: '140px' }}
              placeholder={t('候选文档 1\n候选文档 2\n候选文档 3...\n\n或粘贴 JSON 字符串数组 ["文档1", "文档2"]', 'Document 1\nDocument 2\nDocument 3...\n\nOr paste JSON array ["doc 1", "doc 2"]')}
              value={documentsText}
              onChange={(e) => {
                const val = e.target.value;
                setDocumentsText(val);
                try {
                  const parsed = JSON.parse(val);
                  if (Array.isArray(parsed)) {
                    updateConfig((prev) => ({ ...prev, documents: parsed.map(String) }));
                    return;
                  }
                } catch {
                  // Not JSON array, treat as newline separated
                }
                const lines = val.split('\n').map((s) => s.trim()).filter(Boolean);
                updateConfig((prev) => ({ ...prev, documents: lines }));
              }}
            />
            <span className="hint">
              {t('提示：每行代表一段待评估的文档；也支持传入 JSON 数组或变量 {{steps.vector_db.documents}}', 'Tip: Each line represents one candidate doc; supports JSON array or variable {{steps.vector_db.documents}}')}
            </span>
          </div>
          <div className="grid-2">
            <div className="field">
              <label className="field-label">{t('Top N 返回数量 (保留最高分前 N 条)', 'Top N Results')}</label>
              <input
                type="number"
                className="input"
                min={1}
                max={100}
                value={Number(config.top_n || 3)}
                onChange={(e) => updateConfig((prev) => ({ ...prev, top_n: parseInt(e.target.value, 10) || 3 }))}
              />
            </div>
            <div className="field" style={{ justifyContent: 'center' }}>
              <label className="switch">
                <input
                  type="checkbox"
                  checked={Boolean(config.return_documents ?? true)}
                  onChange={(e) => updateConfig((prev) => ({ ...prev, return_documents: e.target.checked }))}
                />
                <span className="switch-track" />
                <span>{t('返回文档原文 (Return document texts)', 'Return document texts')}</span>
              </label>
            </div>
          </div>
        </div>
      )}

      {/* 7. Parameters Card */}
      {(nodeType === 'llm:chat' || nodeType === 'llm:structured') && (
        <div className="card">
          <div className="card-title">
            <IconSettings size={16} />
            <span>{t('推理参数设置 (Inference Parameters)', 'Inference Parameters')}</span>
          </div>

          <div className="grid-3">
            <div className="field">
              <label className="field-label">
                <span>Temperature ({Number(parameters.temperature ?? 0.7)})</span>
              </label>
              <input
                type="range"
                min="0"
                max="2"
                step="0.05"
                value={Number(parameters.temperature ?? 0.7)}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    parameters: {
                      ...(prev.parameters as Record<string, unknown>),
                      temperature: parseFloat(e.target.value),
                    },
                  }))
                }
              />
            </div>

            <div className="field">
              <label className="field-label">
                <span>Top P ({Number(parameters.top_p ?? 1.0)})</span>
              </label>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                value={Number(parameters.top_p ?? 1.0)}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    parameters: {
                      ...(prev.parameters as Record<string, unknown>),
                      top_p: parseFloat(e.target.value),
                    },
                  }))
                }
              />
            </div>

            <div className="field">
              <label className="field-label">{t('Max Tokens', 'Max Tokens')}</label>
              <input
                type="number"
                className="input input-sm"
                placeholder="4096"
                value={parameters.max_tokens ? String(parameters.max_tokens) : ''}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    parameters: {
                      ...(prev.parameters as Record<string, unknown>),
                      max_tokens: e.target.value ? parseInt(e.target.value, 10) : undefined,
                    },
                  }))
                }
              />
            </div>
          </div>

          <div className="row-between">
            <label className="switch">
              <input
                type="checkbox"
                checked={Boolean(parameters.stream ?? true)}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    parameters: {
                      ...(prev.parameters as Record<string, unknown>),
                      stream: e.target.checked,
                    },
                  }))
                }
              />
              <span className="switch-track" />
              <span>{t('SSE 实时流式响应 (Stream)', 'SSE Real-time Streaming')}</span>
            </label>
          </div>
        </div>
      )}
    </div>
  );
}

function renderPanel(): void {
  const container = document.getElementById('root');
  if (!container) return;
  createRoot(container).render(
    <StrictMode>
      <PanelApp />
    </StrictMode>,
  );
}

function main(): void {
  window.addEventListener('error', (event) => {
    postError(event.message || String(event.error ?? 'unknown panel error'));
  });
  window.addEventListener('unhandledrejection', (event) => {
    const reason = event.reason;
    postError(reason instanceof Error ? reason.message : String(reason ?? 'unhandled rejection'));
  });
  const container = document.getElementById('root');
  if (container) {
    renderPanel();
    return;
  }
  document.addEventListener('DOMContentLoaded', renderPanel);
}

main();
