use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    #[error("B站 API 错误 {code}: {message}")]
    Api { code: i32, message: String },

    #[error("网络请求失败: {message}")]
    Network { message: String },

    #[error("登录状态无效，请重新登录")]
    AuthRequired,

    #[error("当前账号无权访问该视频: {reason}")]
    PermissionDenied { reason: String },

    #[error("下载失败: {message}")]
    Download {
        task_id: Option<String>,
        message: String,
    },

    #[error("任务已取消")]
    Cancelled { task_id: Option<String> },

    #[error("FFmpeg 未安装")]
    FfmpegNotFound,

    #[error("合并失败: {message}")]
    Merge { message: String },

    #[error("云盘操作失败: {message}")]
    Cloud {
        provider: Option<crate::models::cloud::CloudProvider>,
        message: String,
    },

    #[error("本地文件错误: {message}")]
    Io { message: String },

    #[error("输入无效: {message}")]
    InvalidInput { message: String },
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        Self::Network {
            message: value.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io {
            message: value.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Io {
            message: value.to_string(),
        }
    }
}
