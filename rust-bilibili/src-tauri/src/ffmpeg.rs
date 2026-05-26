use std::{
    fs::File as StdFile,
    io,
    path::{Path, PathBuf},
};

use serde::Serialize;
use tokio::{fs, io::AsyncWriteExt, process::Command};

use crate::error::{AppError, AppResult};

const FFMPEG_DOWNLOAD_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";

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

    pub fn check(&self, app_dir: &Path, resource_dir: Option<&Path>) -> FfmpegStatus {
        if let Some(bundled) = bundled_ffmpeg_path(resource_dir) {
            if bundled.exists() {
                return available_status(bundled);
            }
        }

        let cached = cached_ffmpeg_path(app_dir);
        if cached.exists() {
            return available_status(cached);
        }

        unavailable_status()
    }

    pub async fn install_release<F>(&self, app_dir: &Path, progress: F) -> AppResult<PathBuf>
    where
        F: Fn(u64, u64) + Send + Sync,
    {
        let install_dir = app_dir.join("ffmpeg");
        fs::create_dir_all(&install_dir).await?;
        let zip_path = install_dir.join("ffmpeg-release-essentials.zip.tmp");
        let response = reqwest::Client::new()
            .get(FFMPEG_DOWNLOAD_URL)
            .send()
            .await?
            .error_for_status()?;
        let total = response.content_length().unwrap_or(0);
        let mut downloaded = 0_u64;
        let mut file = fs::File::create(&zip_path).await?;
        let mut response = response;

        while let Some(chunk) = response.chunk().await? {
            downloaded += chunk.len() as u64;
            file.write_all(&chunk).await?;
            progress(downloaded, total);
        }
        file.flush().await?;

        let install_dir_for_extract = install_dir.clone();
        let zip_path_for_extract = zip_path.clone();
        let ffmpeg_path = tokio::task::spawn_blocking(move || {
            extract_ffmpeg_exe(&zip_path_for_extract, &install_dir_for_extract)
        })
        .await
        .map_err(|err| AppError::Io {
            message: err.to_string(),
        })??;

        let _ = fs::remove_file(zip_path).await;
        Ok(ffmpeg_path)
    }

    pub async fn merge_video_audio(
        &self,
        app_dir: &Path,
        resource_dir: Option<&Path>,
        video_path: &Path,
        audio_path: &Path,
        output_path: &Path,
    ) -> AppResult<()> {
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let executable = self.resolve_executable(app_dir, resource_dir)?;
        let temp_path = temp_output_path(output_path);

        let mut command = Command::new(executable);
        hide_child_window(&mut command);
        let output = command
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
            .arg(&temp_path)
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
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AppError::Merge { message });
        }

        replace_file(&temp_path, output_path).await?;
        Ok(())
    }

    pub async fn convert_audio_to_mp3(
        &self,
        app_dir: &Path,
        resource_dir: Option<&Path>,
        input_path: &Path,
        output_path: &Path,
    ) -> AppResult<()> {
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let executable = self.resolve_executable(app_dir, resource_dir)?;
        let temp_path = temp_output_path(output_path);

        let mut command = Command::new(executable);
        hide_child_window(&mut command);
        let output = command
            .arg("-y")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(input_path)
            .arg("-vn")
            .arg("-codec:a")
            .arg("libmp3lame")
            .arg("-q:a")
            .arg("2")
            .arg(&temp_path)
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
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AppError::Merge { message });
        }

        replace_file(&temp_path, output_path).await?;
        Ok(())
    }

    pub async fn burn_ass_subtitles(
        &self,
        app_dir: &Path,
        resource_dir: Option<&Path>,
        input_path: &Path,
        ass_path: &Path,
        output_path: &Path,
    ) -> AppResult<()> {
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let executable = self.resolve_executable(app_dir, resource_dir)?;
        let temp_path = temp_output_path(output_path);
        let filter = format!("subtitles={}", ffmpeg_filter_path(ass_path));

        let mut command = Command::new(executable);
        hide_child_window(&mut command);
        let output = command
            .arg("-y")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(input_path)
            .arg("-vf")
            .arg(filter)
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("veryfast")
            .arg("-crf")
            .arg("20")
            .arg("-c:a")
            .arg("copy")
            .arg("-movflags")
            .arg("+faststart")
            .arg(&temp_path)
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
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AppError::Merge { message });
        }

        replace_file(&temp_path, output_path).await?;
        Ok(())
    }

    pub fn resolve_executable(
        &self,
        app_dir: &Path,
        resource_dir: Option<&Path>,
    ) -> AppResult<PathBuf> {
        let status = self.check(app_dir, resource_dir);
        status
            .available
            .then(|| status.path.map(PathBuf::from))
            .flatten()
            .ok_or(AppError::FfmpegNotFound)
    }
}

fn cached_ffmpeg_path(app_dir: &Path) -> PathBuf {
    app_dir.join("ffmpeg").join("ffmpeg.exe")
}

fn bundled_ffmpeg_path(resource_dir: Option<&Path>) -> Option<PathBuf> {
    resource_dir.map(|dir| dir.join("ffmpeg").join("ffmpeg.exe"))
}

fn available_status(path: PathBuf) -> FfmpegStatus {
    FfmpegStatus {
        available: true,
        path: Some(path.to_string_lossy().into_owned()),
        version: None,
    }
}

fn unavailable_status() -> FfmpegStatus {
    FfmpegStatus {
        available: false,
        path: None,
        version: None,
    }
}

pub(crate) fn hide_child_window(command: &mut Command) {
    hide_child_window_inner(command);
}

#[cfg(windows)]
fn hide_child_window_inner(command: &mut Command) {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_child_window_inner(_command: &mut Command) {}

fn extract_ffmpeg_exe(zip_path: &Path, install_dir: &Path) -> AppResult<PathBuf> {
    let zip_file = StdFile::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file).map_err(zip_error)?;
    let mut ffmpeg_index = None;

    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(zip_error)?;
        if is_ffmpeg_exe_entry(file.name()) {
            ffmpeg_index = Some(index);
            break;
        }
    }

    let Some(index) = ffmpeg_index else {
        return Err(AppError::InvalidInput {
            message: "FFmpeg 压缩包中没有找到 bin/ffmpeg.exe".to_string(),
        });
    };

    std::fs::create_dir_all(install_dir)?;
    let ffmpeg_path = cached_ffmpeg_path(install_dir.parent().unwrap_or(install_dir));
    let mut source = archive.by_index(index).map_err(zip_error)?;
    let mut output = StdFile::create(&ffmpeg_path)?;
    io::copy(&mut source, &mut output)?;
    Ok(ffmpeg_path)
}

fn is_ffmpeg_exe_entry(name: &str) -> bool {
    let normalized = name.replace('\\', "/").to_ascii_lowercase();
    if normalized.contains("..") || normalized.contains(':') || normalized.starts_with('/') {
        return false;
    }
    normalized == "ffmpeg.exe" || normalized.ends_with("/bin/ffmpeg.exe")
}

fn zip_error(err: zip::result::ZipError) -> AppError {
    AppError::Io {
        message: err.to_string(),
    }
}

fn temp_output_path(output_path: &Path) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    let extension = output_path.extension().and_then(|ext| ext.to_str());

    match extension {
        Some(ext) if !ext.is_empty() => output_path.with_file_name(format!("{stem}.tmp.{ext}")),
        _ => output_path.with_file_name(format!("{stem}.tmp")),
    }
}

fn ffmpeg_filter_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let escaped = normalized
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "\\'");
    format!("'{escaped}'")
}

async fn replace_file(temp_path: &Path, output_path: &Path) -> AppResult<()> {
    if output_path.exists() {
        tokio::fs::remove_file(output_path).await?;
    }
    tokio::fs::rename(temp_path, output_path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_expected_ffmpeg_zip_entry() {
        assert!(is_ffmpeg_exe_entry(
            "ffmpeg-8.1-essentials_build/bin/ffmpeg.exe"
        ));
        assert!(is_ffmpeg_exe_entry("ffmpeg.exe"));
    }

    #[test]
    fn rejects_unsafe_ffmpeg_zip_entry() {
        assert!(!is_ffmpeg_exe_entry("../bin/ffmpeg.exe"));
        assert!(!is_ffmpeg_exe_entry("C:/tmp/bin/ffmpeg.exe"));
        assert!(!is_ffmpeg_exe_entry("/tmp/bin/ffmpeg.exe"));
    }

    #[test]
    fn keeps_media_extension_on_temp_output_path() {
        assert_eq!(
            temp_output_path(Path::new("C:/Videos/demo.mp4")),
            PathBuf::from("C:/Videos/demo.tmp.mp4")
        );
        assert_eq!(
            temp_output_path(Path::new("C:/Videos/demo.mp3")),
            PathBuf::from("C:/Videos/demo.tmp.mp3")
        );
    }

    #[test]
    fn escapes_windows_path_for_subtitles_filter() {
        assert_eq!(
            ffmpeg_filter_path(Path::new("C:/Videos/demo subtitle.ass")),
            "'C\\:/Videos/demo subtitle.ass'"
        );
    }
}
