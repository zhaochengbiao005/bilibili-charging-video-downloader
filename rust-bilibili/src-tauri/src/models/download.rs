use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StartDownloadRequest {
    pub bvid: String,
    pub quality: String,
    pub format: String,
    pub outdir: String,
    pub cookie_path: Option<String>,
    pub skip_merge: bool,
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
