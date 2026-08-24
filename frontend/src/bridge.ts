import type {
  PluginIframeInitPayload,
  PluginIframeNodeSnapshot,
  PluginStreamMessage,
} from './types';
import { setLocale } from './i18n';

export interface BridgeOptions {
  onInit: (payload: PluginIframeInitPayload) => void;
  onConfigUpdated?: (config: unknown) => void;
  onStream?: (stream: PluginStreamMessage) => void;
}

interface PendingAction {
  resolve: (value: unknown) => void;
  reject: (err: Error) => void;
  timer: number;
}

const ACTION_TIMEOUT_MS = 30000;
let locale = 'en-US';
let nodeType = 'llm:chat';
let nodes: PluginIframeNodeSnapshot[] | undefined = undefined;
let initConfig: Record<string, unknown> = {};

const pendingActions = new Map<string, PendingAction>();

// --- outbound -------------------------------------------------------------

function post(type: string, payload: Record<string, unknown> = {}): void {
  try {
    if (typeof window !== 'undefined' && window.parent && window.parent !== window) {
      window.parent.postMessage({ type, payload }, '*');
    }
  } catch (err) {
    console.error('[llm] failed to post message', type, err);
  }
}

/** iframe -> host: `ready` — panel initialized (sent after `init`). */
export function postReady(): void {
  post('ready', {});
}

/** iframe -> host: `resize` — panel height changed. */
export function postResize(height: number): void {
  post('resize', { height });
}

/** iframe -> host: `error` — panel internal error report. */
export function postError(message: string): void {
  post('error', { message });
}

/** iframe -> host: `requestAction` — ask the host for a whitelisted action. */
function requestAction(action: string, params: unknown): Promise<unknown> {
  return new Promise<unknown>((resolve, reject) => {
    const requestId =
      typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
        ? crypto.randomUUID()
        : String(Date.now()) + '-' + Math.random().toString(36).slice(2);

    const timer = window.setTimeout(() => {
      pendingActions.delete(requestId);
      reject(new Error(`Action '${action}' timed out after ${ACTION_TIMEOUT_MS}ms`));
    }, ACTION_TIMEOUT_MS);

    pendingActions.set(requestId, { resolve, reject, timer });
    post('requestAction', { requestId, action, params });
  });
}

/**
 * Call a design-time plugin sub-method (`llm.test_connection`, `llm.list_models`, ...).
 * The host relays the request to the plugin's `ui_call` handler and returns
 * the payload via `actionResult`.
 */
export function uiCall(method: string, payload: unknown): Promise<unknown> {
  return requestAction('uiCall', { nodeType, method, payload });
}

// --- configChanged (debounced) ---------------------------------------------

let configTimer: number | null = null;
let lastReportedJson = '';

/** iframe -> host: `configChanged` — full config snapshot, debounced 300ms. */
export function notifyConfig(config: unknown): void {
  if (configTimer !== null) {
    window.clearTimeout(configTimer);
  }
  configTimer = window.setTimeout(() => {
    configTimer = null;
    const json = JSON.stringify(config);
    if (json === lastReportedJson) return;
    lastReportedJson = json;
    post('configChanged', { config });
  }, 300);
}

/** Immediately flush any pending configChanged. */
export function flushConfig(): void {
  if (configTimer !== null) {
    window.clearTimeout(configTimer);
    configTimer = null;
  }
}

// --- inbound ---------------------------------------------------------------

/**
 * Install the window message listener. Call once at startup; `ready` is
 * posted as soon as `init` arrives and the handlers have run.
 */
export function initBridge(options: BridgeOptions): () => void {
  let registered = false;
  if (registered) return () => undefined;
  registered = true;

  const handleMessage = (event: MessageEvent) => {
    const data = event.data;
    if (!data || typeof data !== 'object' || typeof data.type !== 'string') {
      return;
    }
    const payload = (data.payload && typeof data.payload === 'object' ? data.payload : {}) as Record<string, unknown>;

    switch (data.type) {
      case 'init': {
        const initPayload = payload as unknown as PluginIframeInitPayload;
        if (typeof initPayload.locale === 'string') {
          locale = initPayload.locale;
          setLocale(initPayload.locale);
        }
        if (typeof initPayload.nodeType === 'string') {
          nodeType = initPayload.nodeType;
        }
        if (Array.isArray(initPayload.nodes)) {
          nodes = initPayload.nodes;
        }
        if (initPayload.config && typeof initPayload.config === 'object') {
          initConfig = initPayload.config as Record<string, unknown>;
          lastReportedJson = JSON.stringify(initPayload.config);
        }
        options.onInit(initPayload);
        postReady();
        break;
      }
      case 'configUpdated': {
        const nextJson = JSON.stringify(payload.config);
        if (nextJson === lastReportedJson) {
          // Echo back of our own change, ignore to avoid interrupting user IME / composition
          break;
        }
        lastReportedJson = nextJson;
        if (options.onConfigUpdated) {
          options.onConfigUpdated(payload.config);
        }
        break;
      }
      case 'actionResult': {
        const requestId = typeof payload.requestId === 'string' ? payload.requestId : '';
        const pending = pendingActions.get(requestId);
        if (!pending) return;
        pendingActions.delete(requestId);
        window.clearTimeout(pending.timer);
        if (typeof payload.error === 'string' && payload.error.length > 0) {
          pending.reject(new Error(payload.error));
        } else {
          pending.resolve(payload.result);
        }
        break;
      }
      case 'stream': {
        if (options.onStream) {
          options.onStream(payload as unknown as PluginStreamMessage);
        }
        break;
      }
    }
  };
  window.addEventListener('message', handleMessage);
  // Inform parent that listener is ready
  post('requestInit', {});
  let lastReportedHeight = -1;
  const updateHeight = () => {
    const root = document.getElementById('root');
    if (root) {
      const height = Math.ceil(root.offsetHeight || root.scrollHeight);
      if (Number.isFinite(height) && height > 0 && Math.abs(height - lastReportedHeight) >= 1) {
        lastReportedHeight = height;
        postResize(height);
      }
    }
  };

  let resizeObserver: ResizeObserver | null = null;
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(updateHeight);
    const root = document.getElementById('root');
    if (root) {
      resizeObserver.observe(root);
    } else if (document.body) {
      resizeObserver.observe(document.body);
    }
  }

  // Immediate & deferred measurements to catch initial render
  setTimeout(updateHeight, 0);
  setTimeout(updateHeight, 100);

  return () => {
    window.removeEventListener('message', handleMessage);
    if (resizeObserver) {
      resizeObserver.disconnect();
    }
  };
}

// --- accessors -------------------------------------------------------------

export function getLocale(): string {
  return locale;
}

export function getNodeType(): string {
  return nodeType;
}

export function getNodes(): PluginIframeNodeSnapshot[] | undefined {
  return nodes;
}

export function getInitConfig(): Record<string, unknown> {
  return initConfig;
}
