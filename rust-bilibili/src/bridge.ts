/**
 * Bridge - React 前端唯一后端入口。
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppConfig,
  ConfigResponse,
  DanmakuMode,
  DownloadDoneEvent,
  DownloadProgressEvent,
  FetchInfoResponse,
  FfmpegStatus,
  HistoryItem,
  LoginStatus,
  PlayUrlResponse,
  QrLoginPollResponse,
  QrLoginStartResponse,
  SaveConfigRequest,
  StartDownloadRequest,
  StartDownloadResponse,
  VideoData,
  AppErrorPayload,
} from './types';

export type ProgressHandler = (taskId: string, percent: number, speed: string, event: DownloadProgressEvent) => void;
export type LogHandler = (msg: string) => void;
export type TaskDoneHandler = (taskId: string, result: any) => void;

const progressHandlers = new Set<ProgressHandler>();
const logHandlers = new Set<LogHandler>();
const taskDoneHandlers = new Set<TaskDoneHandler>();

export function setOnProgress(h: ProgressHandler | null) {
  progressHandlers.clear();
  if (h) progressHandlers.add(h);
}
export function setOnLog(h: LogHandler | null) {
  logHandlers.clear();
  if (h) logHandlers.add(h);
}
export function setOnTaskDone(h: TaskDoneHandler | null) {
  taskDoneHandlers.clear();
  if (h) taskDoneHandlers.add(h);
}
export function addProgressListener(h: ProgressHandler): () => void {
  progressHandlers.add(h);
  return () => progressHandlers.delete(h);
}
export function addTaskDoneListener(h: TaskDoneHandler): () => void {
  taskDoneHandlers.add(h);
  return () => taskDoneHandlers.delete(h);
}

let eventListeners: Promise<UnlistenFn[]> | null = null;
let cachedConfigResponse: ConfigResponse | null = null;
let cachedFfmpegStatus: FfmpegStatus | null = null;
let cachedHistory: HistoryItem[] | null = null;

function isTauriRuntime(): boolean {
  return Boolean((window as any).__TAURI_INTERNALS__?.invoke);
}

function missingRuntimeError(): Error {
  return new Error('此功能需要在 Tauri 桌面应用中运行');
}

async function callCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    throw missingRuntimeError();
  }
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw normalizeCommandError(err);
  }
}

function normalizeCommandError(err: unknown): Error {
  if (err instanceof Error) return err;
  if (typeof err === 'string') return new Error(err);
  if (err && typeof err === 'object') {
    const payload = err as AppErrorPayload;
    const message =
      payload.message ||
      payload.reason ||
      (payload.code !== undefined ? `B站 API 错误 ${payload.code}` : undefined) ||
      JSON.stringify(payload);
    return new Error(message);
  }
  return new Error('后端命令执行失败');
}

function formatSpeed(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return '';
  const units = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
  let value = bytesPerSec;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function ensureEventListeners(): Promise<UnlistenFn[]> {
  if (eventListeners) return eventListeners;
  if (!isTauriRuntime()) return Promise.resolve([]);

  eventListeners = Promise.all([
    listen<DownloadProgressEvent>('download://progress', (event) => {
      const payload = event.payload;
      progressHandlers.forEach((handler) => handler(
        payload.task_id,
        payload.percent,
        formatSpeed(payload.speed_bytes_per_sec),
        payload,
      ));
      if (payload.message) logHandlers.forEach((handler) => handler(payload.message!));
    }),
    listen<string>('download://log', (event) => {
      logHandlers.forEach((handler) => handler(event.payload));
    }),
    listen<DownloadDoneEvent>('download://completed', (event) => {
      taskDoneHandlers.forEach((handler) => handler(
        event.payload.task_id,
        { ...event.payload, status: 'completed' },
      ));
    }),
    listen<DownloadDoneEvent>('download://failed', (event) => {
      taskDoneHandlers.forEach((handler) => handler(
        event.payload.task_id,
        { ...event.payload, status: 'failed' },
      ));
    }),
  ]);

  return eventListeners;
}

export async function fetchInfo(bvid: string, cookiePath = ''): Promise<VideoData> {
  const res = await callCommand<FetchInfoResponse>('fetch_info', {
    input: { bvid, cookie_path: cookiePath || null },
  });
  return res.video;
}

export async function fetchImageDataUrl(url: string): Promise<string> {
  if (!url.trim()) return '';
  if (url.startsWith('data:')) return url;
  if (!isTauriRuntime()) return url;
  return callCommand<string>('fetch_image_data_url', { url });
}

export async function startDownload(
  bvid: string, quality: string, fmt: string,
  outdir: string, cookiePath = '', skipMerge = false, threads = 8, danmakuMode: DanmakuMode = 'none'
): Promise<string> {
  if (!isTauriRuntime()) throw missingRuntimeError();
  await ensureEventListeners();
  const isVideo = fmt !== 'audio';
  const effectiveDanmakuMode = isVideo ? danmakuMode : 'none';
  const input: StartDownloadRequest = {
    bvid,
    quality,
    format: isVideo ? 'video' : 'audio',
    outdir,
    cookie_path: cookiePath || null,
    skip_merge: skipMerge,
    download_danmaku: effectiveDanmakuMode !== 'none',
    danmaku_mode: effectiveDanmakuMode,
    threads,
  };
  const res = await callCommand<StartDownloadResponse>('start_download', {
    input,
  });
  cachedHistory = null;
  return res.task_id;
}

export async function fetchPlayurl(
  bvid: string,
  cid: number,
  qn: number,
  cookiePath = '',
): Promise<PlayUrlResponse> {
  return callCommand<PlayUrlResponse>('fetch_playurl', {
    input: { bvid, cid, qn, cookie_path: cookiePath || null },
  });
}

export async function getHistory(): Promise<HistoryItem[]> {
  if (!isTauriRuntime()) return [];
  if (cachedHistory) return cachedHistory;
  const items = await callCommand<HistoryItem[]>('get_history');
  cachedHistory = items;
  return items;
}

export async function clearHistory(): Promise<void> {
  await callCommand<void>('clear_history');
  cachedHistory = [];
}

export async function deleteHistoryItem(id: string): Promise<boolean> {
  await callCommand<void>('delete_history_item', { id });
  if (cachedHistory) cachedHistory = cachedHistory.filter(item => item.id !== id);
  return true;
}

export async function getConfig(): Promise<AppConfig> {
  if (!isTauriRuntime()) {
    return {
      default_quality: '1080P',
      default_speed: '标准 (8线程)',
      default_outdir: 'downloads',
      auto_merge: true,
      max_history: 200,
    };
  }
  const res = await callCommand<ConfigResponse>('get_config');
  cachedConfigResponse = res;
  return res.config;
}

export async function saveConfig(cfg: AppConfig): Promise<boolean> {
  const input: SaveConfigRequest = { config: cfg };
  const res = await callCommand<ConfigResponse>('save_config', { input });
  cachedConfigResponse = res;
  return true;
}

export async function checkFfmpeg(): Promise<FfmpegStatus> {
  if (!isTauriRuntime()) return { available: false };
  if (cachedFfmpegStatus) return cachedFfmpegStatus;
  const status = await callCommand<FfmpegStatus>('check_ffmpeg');
  cachedFfmpegStatus = status;
  return status;
}

export async function installFfmpeg(): Promise<string> {
  if (!isTauriRuntime()) throw missingRuntimeError();
  await ensureEventListeners();
  cachedFfmpegStatus = null;
  const res = await callCommand<StartDownloadResponse>('install_ffmpeg');
  return res.task_id;
}

export async function getQualityOptions(): Promise<string[]> {
  if (!isTauriRuntime()) return ['360P', '480P', '720P', '1080P', '1080P60', '4K', 'HDR'];
  return callCommand<string[]>('get_quality_options');
}

export async function getDefaultOutdir(): Promise<string> {
  if (!isTauriRuntime()) return 'downloads';
  if (cachedConfigResponse) return cachedConfigResponse.config.default_outdir;
  return callCommand<string>('get_default_outdir');
}

export async function getAppDir(): Promise<string> {
  if (!isTauriRuntime()) return 'Tauri 桌面应用运行时';
  if (cachedConfigResponse) return cachedConfigResponse.app_dir;
  return callCommand<string>('get_app_dir');
}

export async function startQrLogin(): Promise<QrLoginStartResponse> {
  return callCommand<QrLoginStartResponse>('start_qr_login');
}

export async function pollQrLogin(qrcodeKey: string): Promise<QrLoginPollResponse> {
  return callCommand<QrLoginPollResponse>('poll_qr_login', { qrcodeKey });
}

export async function qrLogin(): Promise<QrLoginStartResponse> {
  return startQrLogin();
}

export async function cancelDownload(taskId: string): Promise<boolean> {
  await callCommand<void>('cancel_download', { taskId });
  return true;
}

export async function chooseOutputDir(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  return callCommand<string | null>('choose_output_dir');
}

export async function openPath(path: string): Promise<void> {
  await callCommand<void>('open_path', { path });
}

export async function checkLogin(): Promise<LoginStatus> {
  if (!isTauriRuntime()) {
    return { is_login: false, avatar: null, message: '此功能需要在 Tauri 桌面应用中运行' };
  }
  return callCommand<LoginStatus>('check_login');
}

export async function chooseCookieFile(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  return callCommand<string | null>('choose_cookie_file');
}

export async function checkCookie(cookiePath: string): Promise<LoginStatus> {
  return callCommand<LoginStatus>('check_cookie', { cookiePath });
}

export async function clearCookie(): Promise<LoginStatus> {
  return callCommand<LoginStatus>('clear_cookie');
}

export const minimizeWindow = () => {};
export const maximizeWindow = () => {};
export const closeWindow = () => {};
export const moveWindow = (_dx: number, _dy: number) => {};
