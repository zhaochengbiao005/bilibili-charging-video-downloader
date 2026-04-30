/**
 * Bridge — 连接 React 前端与 Python 后端（HTTP + SSE）
 */

const getBase = () => {
  const apiBase = (window as any).electronAPI?.getApiBase();
  return apiBase || 'http://127.0.0.1:5000';
};

function apiBase(): string {
  // 优先从 Electron preload 获取
  if ((window as any).electronAPI?.getApiBase) {
    return (window as any).electronAPI.getApiBase();
  }
  // 降级：URL query
  const m = window.location.search.match(/[?&]apiBase=([^&]+)/);
  if (m) return decodeURIComponent(m[1]);
  // 降级：全局变量
  if ((window as any).__API_BASE__) return (window as any).__API_BASE__;
  return 'http://127.0.0.1:5000';
}

async function api<T>(url: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${apiBase()}${url}`, {
    ...options,
    headers: { 'Content-Type': 'application/json', ...options?.headers },
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(body.error || res.statusText);
  }
  return res.json();
}

// ── Callbacks (via SSE) ──
export type ProgressHandler = (taskId: string, percent: number, speed: string) => void;
export type LogHandler = (msg: string) => void;
export type TaskDoneHandler = (taskId: string, result: any) => void;

let onProgress: ProgressHandler | null = null;
let onLog: LogHandler | null = null;
let onTaskDone: TaskDoneHandler | null = null;

export function setOnProgress(h: ProgressHandler | null) { onProgress = h; }
export function setOnLog(h: LogHandler | null) { onLog = h; }
export function setOnTaskDone(h: TaskDoneHandler | null) { onTaskDone = h; }

let sseConnected = false;
function connectSSE() {
  if (sseConnected) return;
  sseConnected = true;
  const evtSource = new EventSource(`${apiBase()}/api/events`);

  evtSource.addEventListener('log', (e) => {
    try { const d = JSON.parse(e.data); onLog?.(d.message); }
    catch { /* ignore */ }
  });

  evtSource.addEventListener('progress', (e) => {
    try {
      const d = JSON.parse(e.data);
      onProgress?.(d.taskId, d.percent, d.speed || '');
    } catch { /* ignore */ }
  });

  evtSource.addEventListener('taskDone', (e) => {
    try {
      const d = JSON.parse(e.data);
      onTaskDone?.(d.taskId, d);
    } catch { /* ignore */ }
  });

  evtSource.onerror = () => {
    // 断线重连是 EventSource 内置行为
    console.warn('[SSE] 连接断开，尝试重连...');
  };
}

// ── API 调用 ──
export async function fetchInfo(bvid: string, cookiePath = ''): Promise<any> {
  const params = new URLSearchParams({ bvid });
  if (cookiePath) params.set('cookie', cookiePath);
  return api(`/api/info?${params}`);
}

export async function startDownload(
  bvid: string, quality: string, fmt: string,
  outdir: string, cookiePath = '', skipMerge = false
): Promise<string> {
  connectSSE();
  const res = await api<{ taskId: string }>('/api/download', {
    method: 'POST',
    body: JSON.stringify({ bvid, quality, format: fmt, outdir, cookie: cookiePath, skipMerge }),
  });
  return res.taskId;
}

export async function getHistory(): Promise<any[]> {
  return api('/api/history');
}

export async function clearHistory(): Promise<void> {
  await api('/api/history', { method: 'DELETE' });
}

export async function deleteHistoryItem(id: string): Promise<boolean> {
  await api(`/api/history/${id}`, { method: 'DELETE' });
  return true;
}

export async function getConfig(): Promise<Record<string, any>> {
  return api('/api/config');
}

export async function saveConfig(cfg: Record<string, any>): Promise<boolean> {
  await api('/api/config', { method: 'POST', body: JSON.stringify(cfg) });
  return true;
}

export async function checkFfmpeg(): Promise<boolean> {
  const res = await api<{ available: boolean }>('/api/ffmpeg/check');
  return res.available;
}

export async function installFfmpeg(): Promise<string> {
  connectSSE();
  const res = await api<{ taskId: string }>('/api/ffmpeg/install', { method: 'POST' });
  return res.taskId;
}

export async function getQualityOptions(): Promise<string[]> {
  return api('/api/quality-options');
}

export async function getDefaultOutdir(): Promise<string> {
  const res = await api<{ default_outdir: string }>('/api/paths');
  return res.default_outdir;
}

export async function getAppDir(): Promise<string> {
  const res = await api<{ app_dir: string }>('/api/paths');
  return res.app_dir;
}

export async function qrLogin(): Promise<any> {
  return api('/api/login/qr', { method: 'POST' });
}

export async function cancelDownload(_taskId: string): Promise<boolean> {
  // TODO: 后端取消端点
  return false;
}

export async function checkCookie(_cookiePath: string): Promise<any> {
  return { is_login: false };
}

// ── 窗口控制（Electron 通过 preload 暴露） ──
export const minimizeWindow = () => {};
export const maximizeWindow = () => {};
export const closeWindow = () => {};
export const moveWindow = (_dx: number, _dy: number) => {};
