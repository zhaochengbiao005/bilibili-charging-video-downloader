use std::process::Command;

use serde::Serialize;

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
        let output = Command::new("ffmpeg").arg("-version").output();
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
}
