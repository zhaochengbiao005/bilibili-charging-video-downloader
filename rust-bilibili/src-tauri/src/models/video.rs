use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FetchInfoRequest {
    pub bvid: String,
    pub cookie_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FetchInfoResponse {
    pub video: VideoData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoPage {
    pub cid: u64,
    pub page: u32,
    pub part: String,
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
    pub requires_login: bool,
    pub requires_vip: bool,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoData {
    pub id: String,
    pub title: String,
    pub author: String,
    pub thumbnail: String,
    pub views: String,
    pub duration: String,
    pub duration_sec: u64,
    pub pages: Vec<VideoPage>,
    pub qualities: Vec<String>,
    pub streams: Vec<StreamOption>,
    pub is_charging: Option<bool>,
    pub is_vip: Option<bool>,
    pub vip_type: Option<u32>,
    pub is_login: Option<bool>,
    pub login_name: Option<String>,
    pub login_level: Option<u32>,
    pub desc: Option<String>,
    pub error: Option<String>,
}
