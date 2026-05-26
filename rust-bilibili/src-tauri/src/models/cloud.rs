use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudProvider {
    BaiduNetdisk,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudUploadMode {
    RawDash,
    Mp4PipeExperimental,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudSaveMode {
    Local,
    BaiduNetdisk,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudConfig {
    pub default_provider: CloudProvider,
    pub default_remote_dir: String,
    pub default_save_mode: CloudSaveMode,
    pub part_size_mb: u64,
    pub baidu: BaiduCloudConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BaiduCloudConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SaveCloudConfigRequest {
    pub config: CloudConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BaiduAuthStartResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BaiduAuthFinishRequest {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudAuthStatus {
    pub provider: CloudProvider,
    pub is_authorized: bool,
    pub account_name: Option<String>,
    pub expires_at: Option<String>,
    pub message: Option<String>,
}

impl CloudAuthStatus {
    pub fn guest(provider: CloudProvider, message: impl Into<String>) -> Self {
        Self {
            provider,
            is_authorized: false,
            account_name: None,
            expires_at: None,
            message: Some(message.into()),
        }
    }
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            default_provider: CloudProvider::BaiduNetdisk,
            default_remote_dir: "/apps/B站充电视频下载器".to_string(),
            default_save_mode: CloudSaveMode::Local,
            part_size_mb: 4,
            baidu: BaiduCloudConfig::default(),
        }
    }
}

impl Default for BaiduCloudConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: "http://localhost:1421/baidu/callback".to_string(),
            scope: "basic,netdisk".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudFilePlan {
    pub provider: CloudProvider,
    pub mode: CloudUploadMode,
    pub remote_path: String,
    pub size_bytes: u64,
    pub part_size: u64,
    pub block_md5: Vec<String>,
    pub content_type: Option<String>,
}

impl CloudFilePlan {
    pub fn expected_part_count(&self) -> usize {
        if self.size_bytes == 0 || self.part_size == 0 {
            return 0;
        }
        self.size_bytes.div_ceil(self.part_size) as usize
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudUploadSession {
    pub provider: CloudProvider,
    pub upload_id: String,
    pub remote_path: String,
    pub size_bytes: u64,
    pub part_size: u64,
    pub block_md5: Vec<String>,
    pub uploaded_parts: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudPartResult {
    pub part_index: usize,
    pub bytes_uploaded: u64,
    pub md5: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudFileResult {
    pub provider: CloudProvider,
    pub remote_path: String,
    pub file_id: Option<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CloudUploadProgressEvent {
    pub task_id: String,
    pub provider: CloudProvider,
    pub stage: String,
    pub remote_path: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub percent: f32,
    pub message: Option<String>,
}
