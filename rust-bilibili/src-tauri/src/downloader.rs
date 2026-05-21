use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use reqwest::{
    header::{CONTENT_LENGTH, RANGE, REFERER},
    StatusCode,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

use crate::{
    error::{AppError, AppResult},
    models::download::DownloadProgressEvent,
};

const CHUNK_SIZE: u64 = 4 * 1024 * 1024;
const STREAM_BUFFER_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct DownloadClient {
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct FileDownloadSpec {
    pub task_id: String,
    pub stage: String,
    pub url: String,
    pub output_path: PathBuf,
    pub referer: String,
}

#[derive(Debug, Clone)]
pub struct FileDownloadResult {
    pub output_path: PathBuf,
    pub bytes_written: u64,
}

pub type ProgressSender = Arc<dyn Fn(DownloadProgressEvent) + Send + Sync + 'static>;

impl DownloadClient {
    pub fn new() -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(32)
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
            )
            .build()?;
        Ok(Self { client })
    }

    pub async fn download_file(
        &self,
        spec: FileDownloadSpec,
        cancel: Arc<AtomicBool>,
        progress: ProgressSender,
    ) -> AppResult<FileDownloadResult> {
        if let Some(parent) = spec.output_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let total_size = self
            .content_length(&spec.url, &spec.referer)
            .await
            .unwrap_or(0);
        if total_size > CHUNK_SIZE {
            match self
                .download_range(&spec, total_size, cancel.clone(), progress.clone())
                .await
            {
                Ok(result) => return Ok(result),
                Err(err) if is_range_fallback(&err) => {
                    progress(DownloadProgressEvent {
                        task_id: spec.task_id.clone(),
                        stage: spec.stage.clone(),
                        percent: 0.0,
                        bytes_done: 0,
                        bytes_total: total_size,
                        speed_bytes_per_sec: 0,
                        message: Some("服务器不支持 Range，已回退为普通流式下载".to_string()),
                    });
                }
                Err(err) => return Err(err),
            }
        }

        self.download_stream(&spec, total_size, cancel, progress)
            .await
    }

    async fn content_length(&self, url: &str, referer: &str) -> Option<u64> {
        let response = self
            .client
            .head(url)
            .header(REFERER, referer)
            .send()
            .await
            .ok()?;
        response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
    }

    async fn download_range(
        &self,
        spec: &FileDownloadSpec,
        total_size: u64,
        cancel: Arc<AtomicBool>,
        progress: ProgressSender,
    ) -> AppResult<FileDownloadResult> {
        let temp_dir = spec.output_path.with_extension("parts");
        fs::create_dir_all(&temp_dir).await?;

        let mut ranges = Vec::new();
        let mut start = 0_u64;
        while start < total_size {
            let end = (start + CHUNK_SIZE - 1).min(total_size - 1);
            ranges.push((start, end));
            start = end + 1;
        }

        let mut bytes_done = 0_u64;
        let started = Instant::now();
        let mut part_paths = Vec::with_capacity(ranges.len());

        for (index, (start, end)) in ranges.into_iter().enumerate() {
            ensure_not_cancelled(&cancel, &spec.task_id)?;
            let part_path = temp_dir.join(format!("{index:04}.part"));
            let bytes = self
                .download_range_part(spec, start, end, &part_path, &cancel)
                .await?;
            bytes_done += bytes;
            part_paths.push(part_path);
            emit_progress(
                &progress,
                &spec.task_id,
                &spec.stage,
                bytes_done,
                total_size,
                started,
                None,
            );
        }

        let temp_path = spec.output_path.with_extension("tmp");
        let mut output = File::create(&temp_path).await?;
        for part_path in &part_paths {
            ensure_not_cancelled(&cancel, &spec.task_id)?;
            let mut part = File::open(part_path).await?;
            tokio::io::copy(&mut part, &mut output).await?;
        }
        output.flush().await?;
        replace_file(&temp_path, &spec.output_path).await?;
        let _ = fs::remove_dir_all(temp_dir).await;

        Ok(FileDownloadResult {
            output_path: spec.output_path.clone(),
            bytes_written: bytes_done,
        })
    }

    async fn download_range_part(
        &self,
        spec: &FileDownloadSpec,
        start: u64,
        end: u64,
        part_path: &Path,
        cancel: &Arc<AtomicBool>,
    ) -> AppResult<u64> {
        let mut response = self
            .client
            .get(&spec.url)
            .header(REFERER, &spec.referer)
            .header(RANGE, format!("bytes={start}-{end}"))
            .send()
            .await?;

        if response.status() == StatusCode::OK {
            return Err(AppError::Download {
                task_id: Some(spec.task_id.clone()),
                message: "range_not_supported".to_string(),
            });
        }
        if response.status() != StatusCode::PARTIAL_CONTENT {
            return Err(AppError::Download {
                task_id: Some(spec.task_id.clone()),
                message: format!("分块下载失败，HTTP {}", response.status()),
            });
        }

        let mut file = File::create(part_path).await?;
        let mut bytes = 0_u64;
        while let Some(chunk) = response.chunk().await? {
            ensure_not_cancelled(cancel, &spec.task_id)?;
            bytes += chunk.len() as u64;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(bytes)
    }

    async fn download_stream(
        &self,
        spec: &FileDownloadSpec,
        total_size: u64,
        cancel: Arc<AtomicBool>,
        progress: ProgressSender,
    ) -> AppResult<FileDownloadResult> {
        let mut response = self
            .client
            .get(&spec.url)
            .header(REFERER, &spec.referer)
            .send()
            .await?
            .error_for_status()?;
        let total_size = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(total_size);

        let temp_path = spec.output_path.with_extension("tmp");
        let mut output = File::create(&temp_path).await?;
        let mut bytes_done = 0_u64;
        let started = Instant::now();
        let mut buffered = Vec::with_capacity(STREAM_BUFFER_BYTES);

        while let Some(chunk) = response.chunk().await? {
            ensure_not_cancelled(&cancel, &spec.task_id)?;
            bytes_done += chunk.len() as u64;
            buffered.extend_from_slice(&chunk);
            if buffered.len() >= STREAM_BUFFER_BYTES {
                output.write_all(&buffered).await?;
                buffered.clear();
            }
            emit_progress(
                &progress,
                &spec.task_id,
                &spec.stage,
                bytes_done,
                total_size,
                started,
                None,
            );
        }
        if !buffered.is_empty() {
            output.write_all(&buffered).await?;
        }
        output.flush().await?;
        replace_file(&temp_path, &spec.output_path).await?;

        Ok(FileDownloadResult {
            output_path: spec.output_path.clone(),
            bytes_written: bytes_done,
        })
    }
}

pub fn emit_progress(
    progress: &ProgressSender,
    task_id: &str,
    stage: &str,
    bytes_done: u64,
    bytes_total: u64,
    started: Instant,
    message: Option<String>,
) {
    let percent = if bytes_total > 0 {
        (bytes_done as f32 / bytes_total as f32 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let elapsed = started.elapsed().as_secs_f64();
    let speed = if elapsed > 0.0 {
        (bytes_done as f64 / elapsed) as u64
    } else {
        0
    };
    progress(DownloadProgressEvent {
        task_id: task_id.to_string(),
        stage: stage.to_string(),
        percent,
        bytes_done,
        bytes_total,
        speed_bytes_per_sec: speed,
        message,
    });
}

fn ensure_not_cancelled(cancel: &Arc<AtomicBool>, task_id: &str) -> AppResult<()> {
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::Download {
            task_id: Some(task_id.to_string()),
            message: "任务已取消".to_string(),
        });
    }
    Ok(())
}

async fn replace_file(temp_path: &Path, output_path: &Path) -> AppResult<()> {
    if output_path.exists() {
        fs::remove_file(output_path).await?;
    }
    fs::rename(temp_path, output_path).await?;
    Ok(())
}

fn is_range_fallback(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Download { message, .. } if message == "range_not_supported"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_percent_for_known_totals() {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let sender: ProgressSender = Arc::new(move |event| {
            events_clone.lock().unwrap().push(event);
        });

        emit_progress(
            &sender,
            "task",
            "downloading_video",
            50,
            200,
            Instant::now(),
            None,
        );

        let events = events.lock().unwrap();
        assert_eq!(events[0].percent, 25.0);
        assert_eq!(events[0].bytes_done, 50);
        assert_eq!(events[0].bytes_total, 200);
    }
}
