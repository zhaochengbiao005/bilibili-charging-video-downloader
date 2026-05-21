use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FetchInfoRequest {
    pub bvid: String,
    pub cookie_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoPage {
    pub cid: u64,
    pub page: u32,
    pub part: String,
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
    pub is_charging: Option<bool>,
    pub is_vip: Option<bool>,
    pub vip_type: Option<u32>,
    pub is_login: Option<bool>,
    pub login_name: Option<String>,
    pub login_level: Option<u32>,
    pub desc: Option<String>,
    pub error: Option<String>,
}
