export interface VideoPage {
  cid: number;
  page: number;
  part: string;
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
  is_charging?: boolean;
  is_vip?: boolean;
  vip_type?: number;     // 0=none, 1=monthly, 2=annual
  is_login?: boolean;
  login_name?: string;
  login_level?: number;
  desc?: string;
  error?: string;
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
