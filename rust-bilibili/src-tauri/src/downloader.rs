use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::AtomicBool,
        Arc,
    },
    time::Instant,
};

use reqwest::{
    header::{CONTENT_LENGTH, COOKIE, RANGE, REFERER},
    StatusCode,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    sync::Semaphore,
    task::JoinSet,
};

use crate::{
    download::{
        bili_http::{probe_content_size, BILIBILI_UA},
        ensure_not_cancelled, DownloadStage,
    },
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
    pub stage: DownloadStage,
    pub url: String,
    /// 备用 CDN 节点地址。主地址被节点拒绝（403/404/429/5xx/连接失败）时按序重试。
    pub backup_urls: Vec<String>,
    pub output_path: PathBuf,
    pub referer: String,
    pub max_workers: usize,
    pub cookie_header: Option<String>,
    /// playurl 已给出的精确字节数。优先于 HEAD/Range 探测，避免 CDN 对 HEAD 返回 404 导致体积为 0。
    pub content_length_hint: Option<u64>,
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
            .user_agent(BILIBILI_UA)
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

        let urls = candidate_urls(&spec);
        let mut last_error: Option<AppError> = None;
        for (index, url) in urls.iter().enumerate() {
            let mut attempt = spec.clone();
            attempt.url = url.clone();
            match self
                .download_one_url(&attempt, cancel.clone(), progress.clone())
                .await
            {
                Ok(result) => return Ok(result),
                Err(err) if index + 1 < urls.len() && is_url_retryable(&err) => {
                    last_error = Some(err);
                    progress(DownloadProgressEvent {
                        task_id: spec.task_id.clone(),
                        stage: spec.stage,
                        percent: 0.0,
                        bytes_done: 0,
                        bytes_total: 0,
                        speed_bytes_per_sec: 0,
                        message: Some(format!(
                            "下载节点 {}/{} 拒绝，切换备用节点重试",
                            index + 1,
                            urls.len()
                        )),
                    });
                }
                Err(err) => return Err(err),
            }
        }

        Err(last_error.unwrap_or_else(|| AppError::Download {
            task_id: Some(spec.task_id.clone()),
            message: "所有下载节点均不可用".to_string(),
        }))
    }

    /// 下载单个 URL：体积已知且超阈值走 Range 分块，否则流式。
    async fn download_one_url(
        &self,
        spec: &FileDownloadSpec,
        cancel: Arc<AtomicBool>,
        progress: ProgressSender,
    ) -> AppResult<FileDownloadResult> {
        let total_size = match spec.content_length_hint {
            Some(size) if size > 0 => size,
            _ => {
                probe_content_size(
                    &self.client,
                    &spec.url,
                    &spec.referer,
                    spec.cookie_header.as_deref(),
                )
                .await
                .unwrap_or(0)
            }
        };
        if total_size > CHUNK_SIZE {
            match self
                .download_range(spec, total_size, cancel.clone(), progress.clone())
                .await
            {
                Ok(result) => return Ok(result),
                Err(err) if is_range_fallback(&err) => {
                    progress(DownloadProgressEvent {
                        task_id: spec.task_id.clone(),
                        stage: spec.stage,
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

        self.download_stream(spec, total_size, cancel, progress)
            .await
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
        let _cleanup = TempDirCleanup::new(temp_dir.clone());

        let mut ranges = Vec::new();
        let mut start = 0_u64;
        while start < total_size {
            let end = (start + CHUNK_SIZE - 1).min(total_size - 1);
            ranges.push((start, end));
            start = end + 1;
        }

        let started = Instant::now();
        let max_workers = spec.max_workers.clamp(1, 32).min(ranges.len().max(1));
        let semaphore = Arc::new(Semaphore::new(max_workers));
        let mut tasks = JoinSet::new();
        let range_count = ranges.len();

        for (index, (start, end)) in ranges.into_iter().enumerate() {
            ensure_not_cancelled(&cancel, &spec.task_id)?;
            let part_path = temp_dir.join(format!("{index:04}.part"));
            let worker = self.clone();
            let spec = spec.clone();
            let cancel = cancel.clone();
            let semaphore = semaphore.clone();
            tasks.spawn(async move {
                let _permit =
                    semaphore
                        .acquire_owned()
                        .await
                        .map_err(|err| AppError::Download {
                            task_id: Some(spec.task_id.clone()),
                            message: err.to_string(),
                        })?;
                let bytes = worker
                    .download_range_part(&spec, start, end, &part_path, &cancel)
                    .await?;
                Ok::<_, AppError>((index, part_path, bytes))
            });
        }

        let mut bytes_done = 0_u64;
        let mut part_paths: Vec<Option<PathBuf>> = vec![None; range_count];
        while let Some(result) = tasks.join_next().await {
            ensure_not_cancelled(&cancel, &spec.task_id)?;
            match result {
                Ok(Ok((index, part_path, bytes))) => {
                    bytes_done += bytes;
                    part_paths[index] = Some(part_path);
                    emit_progress(
                        &progress,
                        &spec.task_id,
                        spec.stage,
                        bytes_done,
                        total_size,
                        started,
                        None,
                    );
                }
                Ok(Err(err)) => {
                    tasks.abort_all();
                    return Err(err);
                }
                Err(err) => {
                    tasks.abort_all();
                    return Err(AppError::Download {
                        task_id: Some(spec.task_id.clone()),
                        message: format!("下载线程异常: {err}"),
                    });
                }
            }
        }

        let temp_path = spec.output_path.with_extension("tmp");
        let mut output = File::create(&temp_path).await?;
        for part_path in part_paths {
            ensure_not_cancelled(&cancel, &spec.task_id)?;
            let part_path = part_path.ok_or_else(|| AppError::Download {
                task_id: Some(spec.task_id.clone()),
                message: "下载分块缺失".to_string(),
            })?;
            let mut part = File::open(part_path).await?;
            tokio::io::copy(&mut part, &mut output).await?;
        }
        output.flush().await?;
        replace_file(&temp_path, &spec.output_path).await?;

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
        let mut request = self
            .client
            .get(&spec.url)
            .header(REFERER, &spec.referer)
            .header(RANGE, format!("bytes={start}-{end}"));
        if let Some(cookie_header) = &spec.cookie_header {
            request = request.header(COOKIE, cookie_header);
        }
        let mut response = request.send().await?;

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
        let mut request = self.client.get(&spec.url).header(REFERER, &spec.referer);
        if let Some(cookie_header) = &spec.cookie_header {
            request = request.header(COOKIE, cookie_header);
        }
        let mut response = request.send().await?.error_for_status()?;
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
                spec.stage,
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

struct TempDirCleanup {
    path: PathBuf,
}

impl TempDirCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempDirCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub fn emit_progress(
    progress: &ProgressSender,
    task_id: &str,
    stage: DownloadStage,
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
        stage,
        percent,
        bytes_done,
        bytes_total,
        speed_bytes_per_sec: speed,
        message,
    });
}

/// 构造一个把进度事件推送到前端的 ProgressSender。
pub fn progress_sender<R, E>(app: E) -> ProgressSender
where
    R: tauri::Runtime,
    E: tauri::Emitter<R> + Send + Sync + 'static,
{
    Arc::new(move |event| {
        let _ = app.emit("download://progress", event);
    })
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

/// 按序收集可用下载地址：主地址 + 备用节点，去重、去空。
fn candidate_urls(spec: &FileDownloadSpec) -> Vec<String> {
    let mut urls = Vec::new();
    let push = |urls: &mut Vec<String>, url: &str| {
        let trimmed = url.trim();
        if !trimmed.is_empty() && !urls.iter().any(|existing| existing == trimmed) {
            urls.push(trimmed.to_string());
        }
    };
    push(&mut urls, &spec.url);
    for backup in &spec.backup_urls {
        push(&mut urls, backup);
    }
    urls
}

/// 判定失败是否属于“该节点拒绝此地址”、换备用节点可能成功的类型。
/// 命中：403/404/429/5xx 或连接层失败（超时、重置、DNS、不可达）。
fn is_url_retryable(err: &AppError) -> bool {
    match err {
        AppError::Download { message, .. } => {
            message != "range_not_supported" && mentions_http_rejection(message)
        }
        AppError::Network { message } => {
            mentions_http_rejection(message) || mentions_connection_failure(message)
        }
        _ => false,
    }
}

fn mentions_http_rejection(message: &str) -> bool {
    let upper = message.to_ascii_uppercase();
    [
        "HTTP 401",
        "HTTP 403",
        "HTTP 404",
        "HTTP 429",
        "HTTP 500",
        "HTTP 502",
        "HTTP 503",
        "HTTP 504",
        "FORBIDDEN",
        "NOT FOUND",
        "TOO MANY REQUESTS",
        "INTERNAL SERVER ERROR",
        "BAD GATEWAY",
        "SERVICE UNAVAILABLE",
        "GATEWAY TIMEOUT",
    ]
    .iter()
    .any(|token| upper.contains(token))
}

fn mentions_connection_failure(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    [
        "connection",
        "timed out",
        "timeout",
        "reset",
        "unreachable",
        "dns error",
        "resolve",
    ]
    .iter()
    .any(|token| lower.contains(token))
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
            DownloadStage::DownloadingVideo,
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

    fn make_spec(url: &str, backups: &[&str]) -> FileDownloadSpec {
        FileDownloadSpec {
            task_id: "t".to_string(),
            stage: DownloadStage::Queued,
            url: url.to_string(),
            backup_urls: backups.iter().map(|s| s.to_string()).collect(),
            output_path: PathBuf::from("out"),
            referer: "r".to_string(),
            max_workers: 1,
            cookie_header: None,
            content_length_hint: None,
        }
    }

    #[test]
    fn candidate_urls_orders_primary_then_backups_deduped() {
        let spec = make_spec("https://a", &["https://b", "https://a", "   ", "https://c"]);
        assert_eq!(
            candidate_urls(&spec),
            vec!["https://a", "https://b", "https://c"]
        );
    }

    #[test]
    fn candidate_urls_empty_primary_falls_back_to_backups() {
        let spec = make_spec("  ", &["https://b"]);
        assert_eq!(candidate_urls(&spec), vec!["https://b"]);
    }

    #[test]
    fn retryable_when_cdn_rejects_or_connection_fails() {
        assert!(is_url_retryable(&AppError::Download {
            task_id: None,
            message: "分块下载失败，HTTP 403".to_string(),
        }));
        assert!(is_url_retryable(&AppError::Network {
            message: "error sending request for url (https://x): HTTP status client error \
                       (403 Forbidden) for url (https://x)"
                .to_string(),
        }));
        assert!(is_url_retryable(&AppError::Network {
            message: "error sending request: connection timed out".to_string(),
        }));
        assert!(!is_url_retryable(&AppError::Download {
            task_id: None,
            message: "range_not_supported".to_string(),
        }));
        assert!(!is_url_retryable(&AppError::Download {
            task_id: None,
            message: "任务已取消".to_string(),
        }));
        assert!(!is_url_retryable(&AppError::FfmpegNotFound));
    }
}
