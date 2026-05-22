use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlayUrlRequest {
    pub bvid: String,
    pub cid: u64,
    pub qn: u32,
    pub cookie_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlayUrlResponse {
    pub quality: u32,
    pub timelength: u64,
    pub accept_quality: Vec<u32>,
    pub dash: Option<DashStreams>,
    pub durl: Vec<DurlSegment>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DashStreams {
    pub duration: u64,
    pub video: Vec<DashTrack>,
    pub audio: Vec<DashTrack>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DashTrack {
    pub id: u32,
    pub codecs: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<String>,
    pub bandwidth: Option<u64>,
    pub size_bytes: Option<u64>,
    pub mime_type: Option<String>,
    pub base_url: String,
    pub backup_urls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DurlSegment {
    pub order: u32,
    pub length: u64,
    pub size: u64,
    pub url: String,
    pub backup_urls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StartDownloadRequest {
    pub bvid: String,
    pub quality: String,
    pub format: String,
    pub outdir: String,
    pub cookie_path: Option<String>,
    pub skip_merge: bool,
    #[serde(default)]
    pub download_danmaku: bool,
    #[serde(default)]
    pub danmaku_mode: DanmakuMode,
    #[serde(default = "default_threads")]
    pub threads: usize,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DanmakuMode {
    #[default]
    None,
    Ass,
    Burn,
}

impl StartDownloadRequest {
    pub fn effective_danmaku_mode(&self) -> DanmakuMode {
        if self.format != "video" {
            DanmakuMode::None
        } else if self.danmaku_mode != DanmakuMode::None {
            self.danmaku_mode
        } else if self.download_danmaku {
            DanmakuMode::Ass
        } else {
            DanmakuMode::None
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StartDownloadResponse {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgressEvent {
    pub task_id: String,
    pub stage: String,
    pub percent: f32,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub speed_bytes_per_sec: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadDoneEvent {
    pub task_id: String,
    pub status: String,
    pub message: Option<String>,
}

fn default_threads() -> usize {
    8
}
