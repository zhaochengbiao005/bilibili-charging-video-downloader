use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use reqwest::{
    header::{CONTENT_LENGTH, CONTENT_RANGE, COOKIE, RANGE, REFERER},
    StatusCode,
};

use crate::{
    error::{AppError, AppResult},
    models::download::DownloadProgressEvent,
};

#[derive(Debug, Clone)]
pub struct BiliStreamClient {
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct BiliStreamSpec {
    pub task_id: String,
    pub url: String,
    pub referer: String,
    pub cookie_header: Option<String>,
}

pub type StreamProgressHandler = Arc<dyn Fn(DownloadProgressEvent) + Send + Sync + 'static>;

impl BiliStreamClient {
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

    pub async fn probe_size(&self, spec: &BiliStreamSpec) -> AppResult<Option<u64>> {
        let response = self
            .request_with_bili_headers(self.client.head(&spec.url), spec)
            .send()
            .await;
        if let Ok(response) = response {
            if response.status().is_success() {
                if let Some(size) = header_content_length(response.headers()) {
                    return Ok(Some(size));
                }
            }
        }

        let response = self
            .request_with_bili_headers(self.client.get(&spec.url), spec)
            .header(RANGE, "bytes=0-0")
            .send()
            .await?;
        Ok(header_content_range_total(response.headers())
            .or_else(|| header_content_length(response.headers())))
    }

    pub async fn read_range(
        &self,
        spec: &BiliStreamSpec,
        start: u64,
        end: u64,
        cancel: &Arc<AtomicBool>,
    ) -> AppResult<Vec<u8>> {
        ensure_not_cancelled(cancel, &spec.task_id)?;
        if start > end {
            return Err(AppError::InvalidInput {
                message: "读取区间无效".to_string(),
            });
        }

        let mut response = self
            .request_with_bili_headers(self.client.get(&spec.url), spec)
            .header(RANGE, format!("bytes={start}-{end}"))
            .send()
            .await?;

        match response.status() {
            StatusCode::PARTIAL_CONTENT => {}
            StatusCode::OK => {
                return Err(AppError::Download {
                    task_id: Some(spec.task_id.clone()),
                    message: "服务器不支持 Range 读取".to_string(),
                });
            }
            status => {
                return Err(AppError::Download {
                    task_id: Some(spec.task_id.clone()),
                    message: format!("B站流读取失败，HTTP {status}"),
                });
            }
        }

        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure_not_cancelled(cancel, &spec.task_id)?;
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    pub async fn stream_chunks<F, Fut>(
        &self,
        spec: &BiliStreamSpec,
        cancel: &Arc<AtomicBool>,
        progress: Option<StreamProgressHandler>,
        mut on_chunk: F,
    ) -> AppResult<u64>
    where
        F: FnMut(Vec<u8>, u64, u64) -> Fut,
        Fut: Future<Output = AppResult<()>>,
    {
        ensure_not_cancelled(cancel, &spec.task_id)?;
        let total_size = self.probe_size(spec).await?.unwrap_or(0);
        let mut response = self
            .request_with_bili_headers(self.client.get(&spec.url), spec)
            .send()
            .await?
            .error_for_status()?;

        let total_size = header_content_length(response.headers()).unwrap_or(total_size);
        let mut bytes_done = 0_u64;
        while let Some(chunk) = response.chunk().await? {
            ensure_not_cancelled(cancel, &spec.task_id)?;
            let chunk = chunk.to_vec();
            bytes_done = bytes_done.saturating_add(chunk.len() as u64);
            on_chunk(chunk, bytes_done, total_size).await?;
            if let Some(progress) = &progress {
                progress(stream_progress_event(spec, bytes_done, total_size));
            }
        }
        Ok(bytes_done)
    }

    fn request_with_bili_headers(
        &self,
        request: reqwest::RequestBuilder,
        spec: &BiliStreamSpec,
    ) -> reqwest::RequestBuilder {
        let request = request.header(REFERER, &spec.referer);
        if let Some(cookie_header) = &spec.cookie_header {
            request.header(COOKIE, cookie_header)
        } else {
            request
        }
    }
}

fn stream_progress_event(
    spec: &BiliStreamSpec,
    bytes_done: u64,
    bytes_total: u64,
) -> DownloadProgressEvent {
    let percent = if bytes_total > 0 {
        (bytes_done as f32 / bytes_total as f32 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    DownloadProgressEvent {
        task_id: spec.task_id.clone(),
        stage: "downloading_segments".to_string(),
        percent,
        bytes_done,
        bytes_total,
        speed_bytes_per_sec: 0,
        message: Some("正在读取 B站视频流".to_string()),
    }
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

fn header_content_length(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
}

fn header_content_range_total(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_RANGE)?
        .to_str()
        .ok()?
        .rsplit('/')
        .next()?
        .parse::<u64>()
        .ok()
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    #[test]
    fn parses_content_range_total() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(CONTENT_RANGE, "bytes 0-0/12345".parse().unwrap());

        assert_eq!(header_content_range_total(&headers), Some(12345));
    }

    #[tokio::test]
    async fn read_range_uses_range_referer_and_cookie_headers() -> AppResult<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0_u8; 4096];
            let read = socket.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]);
            let request = request.to_ascii_lowercase();
            assert!(request.contains("range: bytes=1-3"));
            assert!(request.contains("referer: https://www.bilibili.com/video/bvtest"));
            assert!(request.contains("cookie: sessdata=demo"));
            socket
                .write_all(
                    b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 1-3/5\r\n\r\nbcd",
                )
                .await
                .unwrap();
        });

        let client = BiliStreamClient::new()?;
        let bytes = client
            .read_range(
                &BiliStreamSpec {
                    task_id: "task".to_string(),
                    url: format!("http://{addr}/video.m4s"),
                    referer: "https://www.bilibili.com/video/BVtest".to_string(),
                    cookie_header: Some("SESSDATA=demo".to_string()),
                },
                1,
                3,
                &Arc::new(AtomicBool::new(false)),
            )
            .await?;

        server.await.unwrap();
        assert_eq!(bytes, b"bcd");
        Ok(())
    }

    #[tokio::test]
    async fn read_range_checks_cancellation_before_request() {
        let client = BiliStreamClient::new().expect("client should build");
        let cancel = Arc::new(AtomicBool::new(true));
        let result = client
            .read_range(
                &BiliStreamSpec {
                    task_id: "task".to_string(),
                    url: "http://127.0.0.1:9/video.m4s".to_string(),
                    referer: "https://www.bilibili.com/video/BVtest".to_string(),
                    cookie_header: None,
                },
                0,
                1,
                &cancel,
            )
            .await;

        assert!(matches!(result, Err(AppError::Download { .. })));
    }
}
