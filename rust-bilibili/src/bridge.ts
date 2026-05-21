/**
 * Bridge - React 前端唯一后端入口。
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AppConfig, HistoryItem, VideoData } from './types';

export type ProgressHandler = (taskId: string, percent: number, speed: string) => void;
export type LogHandler = (msg: string) => void;
export type TaskDoneHandler = (taskId: string, result: any) => void;

let onProgress: ProgressHandler | null = null;
let onLog: LogHandler | null = null;
let onTaskDone: TaskDoneHandler | null = null;

export function setOnProgress(h: ProgressHandler | null) { onProgress = h; }
export function setOnLog(h: LogHandler | null) { onLog = h; }
export function setOnTaskDone(h: TaskDoneHandler | null) { onTaskDone = h; }

interface DownloadProgressEvent {
  task_id: string;
  percent: number;
  speed_bytes_per_sec: number;
  message?: string;
}

interface DownloadDoneEvent {
  task_id: string;
  status: 'completed' | 'failed' | 'cancelled';
  message?: string;
}

interface StartDownloadResponse {
  task_id: string;
}

interface FfmpegStatus {
  available: boolean;
  path?: string;
  version?: string;
}

let eventListeners: Promise<UnlistenFn[]> | null = null;

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
  return invoke<T>(command, args);
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
      onProgress?.(
        payload.task_id,
        payload.percent,
        formatSpeed(payload.speed_bytes_per_sec),
      );
      if (payload.message) onLog?.(payload.message);
    }),
    listen<string>('download://log', (event) => onLog?.(event.payload)),
    listen<DownloadDoneEvent>('download://completed', (event) => {
      onTaskDone?.(event.payload.task_id, { ...event.payload, status: 'completed' });
    }),
    listen<DownloadDoneEvent>('download://failed', (event) => {
      onTaskDone?.(event.payload.task_id, { ...event.payload, status: 'failed' });
    }),
  ]);

  return eventListeners;
}

export async function fetchInfo(bvid: string, cookiePath = ''): Promise<VideoData> {
  return callCommand<VideoData>('fetch_info', {
    input: { bvid, cookie_path: cookiePath || null },
  });
}

export async function startDownload(
  bvid: string, quality: string, fmt: string,
  outdir: string, cookiePath = '', skipMerge = false
): Promise<string> {
  if (!isTauriRuntime()) throw missingRuntimeError();
  await ensureEventListeners();
  const res = await callCommand<StartDownloadResponse>('start_download', {
    input: {
      bvid,
      quality,
      format: fmt,
      outdir,
      cookie_path: cookiePath || null,
      skip_merge: skipMerge,
    },
  });
  return res.task_id;
}

export async function getHistory(): Promise<HistoryItem[]> {
  if (!isTauriRuntime()) return [];
  return callCommand<HistoryItem[]>('get_history');
}

export async function clearHistory(): Promise<void> {
  await callCommand<void>('clear_history');
}

export async function deleteHistoryItem(id: string): Promise<boolean> {
  await callCommand<void>('delete_history_item', { id });
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
  return callCommand<AppConfig>('get_config');
}

export async function saveConfig(cfg: AppConfig): Promise<boolean> {
  await callCommand<void>('save_config', { config: cfg });
  return true;
}

export async function checkFfmpeg(): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  const res = await callCommand<FfmpegStatus>('check_ffmpeg');
  return res.available;
}

export async function installFfmpeg(): Promise<string> {
  if (!isTauriRuntime()) throw missingRuntimeError();
  await ensureEventListeners();
  const res = await callCommand<StartDownloadResponse>('install_ffmpeg');
  return res.task_id;
}

export async function getQualityOptions(): Promise<string[]> {
  if (!isTauriRuntime()) return ['360P', '480P', '720P', '1080P', '1080P60', '4K', 'HDR'];
  return callCommand<string[]>('get_quality_options');
}

export async function getDefaultOutdir(): Promise<string> {
  if (!isTauriRuntime()) return 'downloads';
  return callCommand<string>('get_default_outdir');
}

export async function getAppDir(): Promise<string> {
  if (!isTauriRuntime()) return 'Tauri 桌面应用运行时';
  return callCommand<string>('get_app_dir');
}

export async function qrLogin(): Promise<any> {
  return callCommand('start_qr_login');
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

export async function checkCookie(cookiePath: string): Promise<any> {
  return callCommand('check_cookie', { cookiePath });
}

export const minimizeWindow = () => {};
export const maximizeWindow = () => {};
export const closeWindow = () => {};
export const moveWindow = (_dx: number, _dy: number) => {};
