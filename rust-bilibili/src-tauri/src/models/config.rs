use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub default_quality: String,
    pub default_speed: String,
    pub default_outdir: String,
    pub auto_merge: bool,
    pub max_history: usize,
    /// 同时执行的下载/直传任务数（1-16），旧配置文件缺省为 2
    #[serde(default = "default_concurrent_downloads")]
    pub concurrent_downloads: usize,
}

fn default_concurrent_downloads() -> usize {
    2
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigResponse {
    pub config: AppConfig,
    pub app_dir: String,
    pub default_outdir: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SaveConfigRequest {
    pub config: AppConfig,
}

impl AppConfig {
    pub fn with_default_outdir(default_outdir: String) -> Self {
        Self {
            default_quality: "1080P".to_string(),
            default_speed: "标准 (8线程)".to_string(),
            default_outdir,
            auto_merge: true,
            max_history: 200,
            concurrent_downloads: default_concurrent_downloads(),
        }
    }
}
