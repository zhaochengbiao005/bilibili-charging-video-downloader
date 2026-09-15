use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FetchInfoRequest {
    pub bvid: String,
    pub cookie_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FetchInfoResponse {
    pub video: VideoData,
    pub videos: Vec<VideoData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnrichVideoRequest {
    pub video: VideoData,
    pub cid: Option<u64>,
    pub cookie_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoPage {
    pub cid: u64,
    pub page: u32,
    pub part: String,
    pub duration: String,
    pub duration_sec: u64,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamOption {
    pub id: String,
    pub qn: u32,
    pub label: String,
    pub codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<String>,
    pub bandwidth: Option<u64>,
    pub size_bytes: Option<u64>,
    pub requires_login: bool,
    pub requires_vip: bool,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioStreamOption {
    pub id: String,
    pub label: String,
    pub bandwidth: Option<u64>,
    pub size_bytes: Option<u64>,
    pub available: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoData {
    pub id: String,
    pub title: String,
    pub author: String,
    pub author_avatar: String,
    pub thumbnail: String,
    pub views: String,
    pub duration: String,
    pub duration_sec: u64,
    pub pages: Vec<VideoPage>,
    pub qualities: Vec<String>,
    pub streams: Vec<StreamOption>,
    pub audio_streams: Vec<AudioStreamOption>,
    pub is_charging: Option<bool>,
    /// 当前账号是否已具备充电专属完整播放权（is_upower_play）
    pub is_upower_play: Option<bool>,
    /// playurl 实际只返回了试看流
    pub is_preview: Option<bool>,
    pub is_vip: Option<bool>,
    pub vip_type: Option<u32>,
    pub is_login: Option<bool>,
    pub login_name: Option<String>,
    pub login_level: Option<u32>,
    pub desc: Option<String>,
    /// 访问受限/试看等提示文案
    pub access_message: Option<String>,
    /// 是否为互动视频（stein gate）
    pub is_stein: Option<bool>,
    /// 互动视频剧情图（解析后填充）
    pub stein_graph: Option<crate::models::stein::SteinGraph>,
    pub error: Option<String>,
}
