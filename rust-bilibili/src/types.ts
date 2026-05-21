export interface VideoPage {
  cid: number;
  page: number;
  part: string;
}

export interface StreamOption {
  id: string;
  qn: number;
  label: string;
  codec?: string;
  width?: number;
  height?: number;
  frame_rate?: string;
  bandwidth?: number;
  requires_login: boolean;
  requires_vip: boolean;
  available: boolean;
  unavailable_reason?: string;
}

export interface VideoData {
  id: string;            // BVID
  title: string;
  author: string;
  thumbnail: string;
  views: string;
  duration: string;      // "12:45" format
  duration_sec: number;
  pages: VideoPage[];
  qualities: string[];
  streams: StreamOption[];
  is_charging?: boolean;
  is_vip?: boolean;
  vip_type?: number;     // 0=none, 1=monthly, 2=annual
  is_login?: boolean;
  login_name?: string;
  login_level?: number;
  desc?: string;
  error?: string;
}

export interface FetchInfoRequest {
  bvid: string;
  cookie_path?: string | null;
}

export interface FetchInfoResponse {
  video: VideoData;
}

export type DownloadStage =
  | 'queued'
  | 'resolving'
  | 'downloading_video'
  | 'downloading_audio'
  | 'downloading_segments'
  | 'merging'
  | 'converting_audio'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'paused';

export interface StartDownloadRequest {
  bvid: string;
  quality: string;
  format: 'video' | 'audio';
  outdir: string;
  cookie_path?: string | null;
  skip_merge: boolean;
  threads: number;
}

export interface PlayUrlRequest {
  bvid: string;
  cid: number;
  qn: number;
  cookie_path?: string | null;
}

export interface PlayUrlResponse {
  quality: number;
  timelength: number;
  accept_quality: number[];
  dash?: DashStreams | null;
  durl: DurlSegment[];
}

export interface DashStreams {
  duration: number;
  video: DashTrack[];
  audio: DashTrack[];
}

export interface DashTrack {
  id: number;
  codecs: string;
  width?: number | null;
  height?: number | null;
  frame_rate?: string | null;
  bandwidth?: number | null;
  mime_type?: string | null;
  base_url: string;
  backup_urls: string[];
}

export interface DurlSegment {
  order: number;
  length: number;
  size: number;
  url: string;
  backup_urls: string[];
}

export interface StartDownloadResponse {
  task_id: string;
}

export interface DownloadProgressEvent {
  task_id: string;
  stage: DownloadStage;
  percent: number;
  bytes_done: number;
  bytes_total: number;
  speed_bytes_per_sec: number;
  message?: string;
}

export interface DownloadDoneEvent {
  task_id: string;
  status: 'completed' | 'failed' | 'cancelled';
  message?: string;
}

export interface DownloadTask {
  id: string;
  bvid?: string;
  title: string;
  quality: string;
  format: 'video' | 'audio';
  progress: number;
  status: 'downloading' | 'completed' | 'error' | 'cancelled';
  speed?: string;
  output_dir?: string;
  error_message?: string;
}

export interface HistoryItem {
  id: string;
  bvid: string;
  title: string;
  quality: string;
  format: string;
  output_path: string;
  timestamp: string;
  status: string;
}

export interface AppConfig {
  default_quality: string;
  default_speed: string;
  default_outdir: string;
  auto_merge: boolean;
  max_history: number;
}

export interface ConfigResponse {
  config: AppConfig;
  app_dir: string;
  default_outdir: string;
}

export interface SaveConfigRequest {
  config: AppConfig;
}

export interface FfmpegStatus {
  available: boolean;
  path?: string;
  version?: string;
}

export interface AppErrorPayload {
  kind: string;
  message?: string;
  code?: number;
  reason?: string;
  task_id?: string | null;
}

export interface LoginStatus {
  is_login: boolean;
  username?: string | null;
  uid?: number | null;
  level?: number | null;
  vip_type?: number | null;
  message?: string | null;
  cookie_path?: string | null;
}

export interface QrLoginStartResponse {
  url: string;
  qrcode_key: string;
  qrcode_svg: string;
  expires_in_sec: number;
}

export interface QrLoginPollResponse {
  status: 'waiting' | 'scanned' | 'confirmed' | 'expired' | 'unknown';
  message: string;
  login?: LoginStatus | null;
}
