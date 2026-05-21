use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub default_quality: String,
    pub default_speed: String,
    pub default_outdir: String,
    pub auto_merge: bool,
    pub max_history: usize,
}

impl AppConfig {
    pub fn with_default_outdir(default_outdir: String) -> Self {
        Self {
            default_quality: "1080P".to_string(),
            default_speed: "标准 (8线程)".to_string(),
            default_outdir,
            auto_merge: true,
            max_history: 200,
        }
    }
}
