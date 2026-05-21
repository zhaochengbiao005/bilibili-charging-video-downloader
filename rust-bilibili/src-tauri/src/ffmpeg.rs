use std::{path::Path, process::Command as SyncCommand};

use serde::Serialize;
use tokio::process::Command;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct FfmpegManager;

#[derive(Debug, Clone, Serialize)]
pub struct FfmpegStatus {
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

impl FfmpegManager {
    pub fn new() -> Self {
        Self
    }

    pub fn check(&self) -> FfmpegStatus {
        let output = SyncCommand::new("ffmpeg").arg("-version").output();
        match output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .map(str::to_string);
                FfmpegStatus {
                    available: true,
                    path: Some("ffmpeg".to_string()),
                    version,
                }
            }
            _ => FfmpegStatus {
                available: false,
                path: None,
                version: None,
            },
        }
    }

    pub async fn merge_video_audio(
        &self,
        video_path: &Path,
        audio_path: &Path,
        output_path: &Path,
    ) -> AppResult<()> {
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let output = Command::new("ffmpeg")
            .arg("-y")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(video_path)
            .arg("-i")
            .arg(audio_path)
            .arg("-map")
            .arg("0:v:0")
            .arg("-map")
            .arg("1:a:0")
            .arg("-c")
            .arg("copy")
            .arg("-shortest")
            .arg(output_path)
            .output()
            .await
            .map_err(|err| {
                if err.kind() == std::io::ErrorKind::NotFound {
                    AppError::FfmpegNotFound
                } else {
                    AppError::Io {
                        message: err.to_string(),
                    }
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.is_empty() {
                format!("FFmpeg 退出码 {}", output.status)
            } else {
                stderr
            };
            return Err(AppError::Merge { message });
        }

        Ok(())
    }
}
