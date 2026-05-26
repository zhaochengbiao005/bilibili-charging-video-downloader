export interface VideoPage {
  cid: number;
  page: number;
  part: string;
  duration: string;
  duration_sec: number;
  thumbnail?: string | null;
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
  size_bytes?: number | null;
  requires_login: boolean;
  requires_vip: boolean;
  available: boolean;
  unavailable_reason?: string;
}

export interface AudioStreamOption {
  id: string;
  label: string;
  bandwidth?: number | null;
  size_bytes?: number | null;
  available: boolean;
}

export interface VideoData {
  id: string;            // BVID
  title: string;
  author: string;
  author_avatar: string;
  thumbnail: string;
  views: string;
  duration: string;      // "12:45" format
  duration_sec: number;
  pages: VideoPage[];
  qualities: string[];
  streams: StreamOption[];
  audio_streams: AudioStreamOption[];
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
  videos?: VideoData[];
}

export interface EnrichVideoRequest {
  video: VideoData;
  cid?: number | null;
  cookie_path?: string | null;
}

export type DownloadStage =
  | 'queued'
  | 'resolving'
  | 'cloud_calculating_md5'
  | 'cloud_muxing_mp4'
  | 'cloud_precreating'
  | 'cloud_uploading_video'
  | 'cloud_uploading_audio'
  | 'cloud_uploading_danmaku'
  | 'downloading_video'
  | 'downloading_audio'
  | 'downloading_danmaku'
  | 'burning_danmaku'
  | 'downloading_segments'
  | 'merging'
  | 'converting_audio'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'paused';

export interface StartDownloadRequest {
  bvid: string;
  cid?: number | null;
  page?: number | null;
  part?: string | null;
  quality: string;
  format: 'video' | 'audio';
  outdir: string;
  cookie_path?: string | null;
  skip_merge: boolean;
  download_danmaku: boolean;
  danmaku_mode: DanmakuMode;
  threads: number;
}

export type DanmakuMode = 'none' | 'ass' | 'burn';

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
  size_bytes?: number | null;
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
  storage?: CloudSaveMode;
  error_message?: string;
  message?: string;
  download_danmaku?: boolean;
  danmaku_mode?: DanmakuMode;
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

export type CloudProvider = 'baidu_netdisk';
export type CloudSaveMode = 'local' | 'baidu_netdisk';

export interface BaiduCloudConfig {
  client_id: string;
  client_secret: string;
  redirect_uri: string;
  scope: string;
}

export interface CloudConfig {
  default_provider: CloudProvider;
  default_remote_dir: string;
  default_save_mode: CloudSaveMode;
  part_size_mb: number;
  baidu: BaiduCloudConfig;
}

export interface SaveCloudConfigRequest {
  config: CloudConfig;
}

export interface CloudAuthStatus {
  provider: CloudProvider;
  is_authorized: boolean;
  account_name?: string | null;
  expires_at?: string | null;
  message?: string | null;
}

export interface CloudFileResult {
  provider: CloudProvider;
  remote_path: string;
  file_id?: string | null;
  size_bytes: number;
}

export interface BaiduAuthStartResponse {
  auth_url: string;
  state: string;
}

export interface BaiduAuthFinishRequest {
  code: string;
  state?: string | null;
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
  avatar?: string | null;
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
