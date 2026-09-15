//! 任务编排模块：下载 / 云盘直传任务的队列、取消、事件与生命周期收尾。
//!
//! command 层只做校验与委托；这里持有全部任务依赖，任务身体通过
//! `spawn_queued` 统一获得进度发射、并发槽与完成/取消/失败事件语义。

use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use tauri::{AppHandle, Emitter};
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;

use crate::{
    api::BilibiliClient,
    auth::CookieSet,
    cloud::baidu::BaiduTokenStore,
    danmaku::DanmakuClient,
    download::{first_url, quality_to_qn, select_audio_track, select_video_track, selected_page},
    downloader::{
        emit_progress, progress_sender, DownloadClient, FileDownloadSpec, ProgressSender,
    },
    error::{AppError, AppResult},
    ffmpeg::FfmpegManager,
    models::{
        download::{
            DanmakuMode, DownloadDoneEvent, DownloadProgressEvent, DownloadStage, DurlSegment,
            StartDownloadRequest, TaskStatus,
        },
        history::HistoryItem,
        video::VideoData,
    },
    models::stein::SteinDownloadMode,
    storage::{ConfigStore, HistoryStore},
};

/// 全部下载/上传任务共享的编排器：依赖注入点 + 队列 + 取消表 + 事件收尾。
#[derive(Debug)]
pub struct TaskOrchestrator {
    pub client: BilibiliClient,
    pub downloader: DownloadClient,
    pub ffmpeg: FfmpegManager,
    pub danmaku: DanmakuClient,
    pub config_store: ConfigStore,
    pub history_store: HistoryStore,
    pub baidu_token_store: BaiduTokenStore,
    app_dir: PathBuf,
    cancel_tokens: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    download_slots: Arc<Semaphore>,
}

impl TaskOrchestrator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client: BilibiliClient,
        downloader: DownloadClient,
        ffmpeg: FfmpegManager,
        danmaku: DanmakuClient,
        config_store: ConfigStore,
        history_store: HistoryStore,
        baidu_token_store: BaiduTokenStore,
        app_dir: PathBuf,
    ) -> Self {
        Self {
            client,
            downloader,
            ffmpeg,
            danmaku,
            config_store,
            history_store,
            baidu_token_store,
            app_dir,
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
            download_slots: Arc::new(Semaphore::new(2)),
        }
    }

    pub fn app_dir(&self) -> &Path {
        &self.app_dir
    }

    /// 注册取消令牌；在向前端返回 task_id 之前调用，保证取消入口立即可达。
    pub async fn register_cancel(&self, task_id: &str) -> Arc<AtomicBool> {
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_tokens
            .write()
            .await
            .insert(task_id.to_string(), cancel.clone());
        cancel
    }

    /// 请求取消一个运行中的任务。返回 false 表示任务不存在或已结束。
    pub async fn cancel(&self, task_id: &str) -> bool {
        match self.cancel_tokens.read().await.get(task_id) {
            Some(cancel) => {
                cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// 统一任务生命周期：排入队列 → 取并发槽 → 执行 → 收尾（移除令牌 + 完成事件）。
    ///
    /// 任务身体返回完成文案；错误在这里统一映射为 cancelled/failed 事件。
    pub fn spawn_queued<F, Fut>(
        self: &Arc<Self>,
        app: AppHandle,
        task_id: String,
        cancel: Arc<AtomicBool>,
        body: F,
    ) where
        F: FnOnce(Arc<TaskOrchestrator>) -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<String>> + Send + 'static,
    {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let _ = app.emit(
                "download://progress",
                DownloadProgressEvent {
                    task_id: task_id.clone(),
                    stage: DownloadStage::Queued,
                    percent: 0.0,
                    bytes_done: 0,
                    bytes_total: 0,
                    speed_bytes_per_sec: 0,
                    message: Some("已加入下载队列，等待空闲任务槽".to_string()),
                },
            );
            let permit = match this.download_slots.clone().acquire_owned().await {
                Ok(permit) => permit,
                Err(err) => {
                    this.cancel_tokens.write().await.remove(&task_id);
                    let _ = app.emit(
                        "download://failed",
                        DownloadDoneEvent {
                            task_id,
                            status: TaskStatus::Failed,
                            message: Some(format!("下载队列已关闭：{err}")),
                        },
                    );
                    return;
                }
            };
            let _ = &cancel; // 令牌由 command 层注册，任务结束时统一移除
            let result = body(this.clone()).await;
            drop(permit);
            this.cancel_tokens.write().await.remove(&task_id);

            match result {
                Ok(message) => {
                    let _ = app.emit(
                        "download://completed",
                        DownloadDoneEvent {
                            task_id,
                            status: TaskStatus::Completed,
                            message: Some(message),
                        },
                    );
                }
                Err(err) => {
                    let status = match err {
                        AppError::Cancelled { .. } => TaskStatus::Cancelled,
                        _ => TaskStatus::Failed,
                    };
                    let message = err.to_string();
                    let _ = app.emit(
                        "download://failed",
                        DownloadDoneEvent {
                            task_id,
                            status,
                            message: Some(message),
                        },
                    );
                }
            }
        });
    }

    pub async fn run_download_task(
        &self,
        task_id: String,
        input: StartDownloadRequest,
        app: AppHandle,
        resource_dir: Option<PathBuf>,
        cookies: Option<CookieSet>,
        cancel: Arc<AtomicBool>,
    ) -> AppResult<PathBuf> {
        let progress = progress_sender(app);
        let started = Instant::now();
        progress(DownloadProgressEvent {
            task_id: task_id.clone(),
            stage: DownloadStage::Resolving,
            percent: 0.0,
            bytes_done: 0,
            bytes_total: 0,
            speed_bytes_per_sec: 0,
            message: Some(format!("正在解析播放地址{}", danmaku_mode_suffix(&input))),
        });

        let video = self.client.video_info(&input.bvid, cookies.as_ref()).await?;
        let page = selected_page(&video, &input)?;
        let qn = quality_to_qn(&input.quality);
        let max_workers = input.threads.clamp(1, 32);

        // 互动视频：整图 / 路径下载
        if video.is_stein == Some(true)
            || matches!(
                input.stein_mode,
                SteinDownloadMode::All | SteinDownloadMode::Path
            )
        {
            return self
                .run_stein_download_task(
                    task_id,
                    input,
                    video,
                    resource_dir,
                    cookies,
                    cancel,
                    progress,
                    started,
                    qn,
                    max_workers,
                )
                .await;
        }

        let playurl = self
            .client
            .playurl(&input.bvid, page.cid, qn, cookies.as_ref())
            .await?;
        let meta_duration_sec = if page.duration_sec > 0 {
            page.duration_sec
        } else {
            video.duration_sec
        };
        if playurl.is_preview_stream(meta_duration_sec) {
            return Err(AppError::Download {
                task_id: Some(task_id),
                message: playurl.preview_block_message(meta_duration_sec),
            });
        }
        let output_dir = PathBuf::from(input.outdir.trim());
        tokio::fs::create_dir_all(&output_dir).await?;
        let safe_title = sanitize_filename::sanitize(format!("{}-{}", video.title, page.part));
        let referer = format!("https://www.bilibili.com/video/{}", input.bvid);
        let cookie_header = cookies.as_ref().map(|cookies| cookies.to_header());

        if input.format == "audio" {
            let dash = playurl.dash.ok_or_else(|| AppError::Download {
                task_id: Some(task_id.clone()),
                message: "没有找到 DASH 音频流".to_string(),
            })?;
            let track =
                select_audio_track(&dash.audio, &input.quality).ok_or_else(|| AppError::Download {
                    task_id: Some(task_id.clone()),
                    message: "没有可下载音频流".to_string(),
                })?;
            let raw_audio_path = output_dir.join(format!("{safe_title}.m4a"));
            let result = self
                .downloader
                .download_file(
                    FileDownloadSpec {
                        task_id: task_id.clone(),
                        stage: DownloadStage::DownloadingAudio,
                        url: first_url(&track.base_url, &track.backup_urls)?,
                        backup_urls: track.backup_urls.clone(),
                        output_path: raw_audio_path.clone(),
                        referer,
                        max_workers,
                        cookie_header: cookie_header.clone(),
                        content_length_hint: track.size_bytes,
                    },
                    cancel,
                    progress.clone(),
                )
                .await?;
            progress(DownloadProgressEvent {
                task_id: task_id.clone(),
                stage: DownloadStage::ConvertingAudio,
                percent: 98.0,
                bytes_done: result.bytes_written,
                bytes_total: result.bytes_written,
                speed_bytes_per_sec: 0,
                message: Some("正在转换 MP3".to_string()),
            });
            let mp3_path = output_dir.join(format!("{safe_title}.mp3"));
            self.ffmpeg
                .convert_audio_to_mp3(
                    self.app_dir(),
                    resource_dir.as_deref(),
                    &raw_audio_path,
                    &mp3_path,
                )
                .await?;
            let _ = tokio::fs::remove_file(&raw_audio_path).await;
            self.write_history_record(&input, &video.title, &mp3_path, "completed")?;
            return Ok(mp3_path);
        }

        if let Some(dash) = playurl.dash {
            let video_track =
                select_video_track(&dash.video, qn).ok_or_else(|| AppError::Download {
                    task_id: Some(task_id.clone()),
                    message: "没有可下载视频流".to_string(),
                })?;
            let video_path = output_dir.join(format!("{safe_title}-video-{}.m4s", video_track.id));
            let video_result = self
                .downloader
                .download_file(
                    FileDownloadSpec {
                        task_id: task_id.clone(),
                        stage: DownloadStage::DownloadingVideo,
                        url: first_url(&video_track.base_url, &video_track.backup_urls)?,
                        backup_urls: video_track.backup_urls.clone(),
                        output_path: video_path,
                        referer: referer.clone(),
                        max_workers,
                        cookie_header: cookie_header.clone(),
                        content_length_hint: video_track.size_bytes,
                    },
                    cancel.clone(),
                    progress.clone(),
                )
                .await?;

            let Some(audio_track) = select_audio_track(&dash.audio, &input.quality) else {
                return Err(AppError::Download {
                    task_id: Some(task_id),
                    message: "没有可用于合并 MP4 的音频流".to_string(),
                });
            };
            let audio_path = output_dir.join(format!("{safe_title}-audio.m4a"));
            let _ = self
                .downloader
                .download_file(
                    FileDownloadSpec {
                        task_id: task_id.clone(),
                        stage: DownloadStage::DownloadingAudio,
                        url: first_url(&audio_track.base_url, &audio_track.backup_urls)?,
                        backup_urls: audio_track.backup_urls.clone(),
                        output_path: audio_path.clone(),
                        referer,
                        max_workers,
                        cookie_header: cookie_header.clone(),
                        content_length_hint: audio_track.size_bytes,
                    },
                    cancel,
                    progress.clone(),
                )
                .await?;

            if !input.skip_merge {
                progress(DownloadProgressEvent {
                    task_id: task_id.clone(),
                    stage: DownloadStage::Merging,
                    percent: 98.0,
                    bytes_done: video_result.bytes_written,
                    bytes_total: video_result.bytes_written,
                    speed_bytes_per_sec: 0,
                    message: Some("正在合并音视频".to_string()),
                });
                let merged_path = output_dir.join(format!("{safe_title}.mp4"));
                self.ffmpeg
                    .merge_video_audio(
                        self.app_dir(),
                        resource_dir.as_deref(),
                        &video_result.output_path,
                        &audio_path,
                        &merged_path,
                    )
                    .await?;
                let danmaku_result = process_danmaku_if_requested(
                    &self.danmaku,
                    &self.ffmpeg,
                    &input,
                    page.cid,
                    &merged_path,
                    self.app_dir(),
                    resource_dir.as_deref(),
                    cookies.as_ref(),
                    &progress,
                    &task_id,
                )
                .await;
                let completion_message = completion_message("MP4 下载完成", danmaku_result);
                let _ = tokio::fs::remove_file(&video_result.output_path).await;
                let _ = tokio::fs::remove_file(&audio_path).await;
                self.write_history_record(&input, &video.title, &merged_path, "completed")?;
                progress(DownloadProgressEvent {
                    task_id,
                    stage: DownloadStage::Completed,
                    percent: 100.0,
                    bytes_done: video_result.bytes_written,
                    bytes_total: video_result.bytes_written,
                    speed_bytes_per_sec: 0,
                    message: Some(completion_message),
                });
                return Ok(merged_path);
            }

            let danmaku_result = process_danmaku_if_requested(
                &self.danmaku,
                &self.ffmpeg,
                &input,
                page.cid,
                &video_result.output_path,
                self.app_dir(),
                resource_dir.as_deref(),
                cookies.as_ref(),
                &progress,
                &task_id,
            )
            .await;
            let completion_message = completion_message(
                "原始 DASH 流下载完成，已按请求跳过 MP4 合并",
                danmaku_result,
            );
            progress(DownloadProgressEvent {
                task_id,
                stage: DownloadStage::Completed,
                percent: 100.0,
                bytes_done: video_result.bytes_written,
                bytes_total: video_result.bytes_written,
                speed_bytes_per_sec: 0,
                message: Some(completion_message),
            });
            self.write_history_record(
                &input,
                &video.title,
                &video_result.output_path,
                "completed",
            )?;
            return Ok(video_result.output_path);
        }

        if !playurl.durl.is_empty() {
            let output_path = output_dir.join(format!("{safe_title}.flv"));
            download_durl_segments(
                &self.downloader,
                &task_id,
                &playurl.durl,
                &output_path,
                &referer,
                cancel,
                progress.clone(),
                max_workers,
                cookie_header.clone(),
            )
            .await?;
            let danmaku_result = process_danmaku_if_requested(
                &self.danmaku,
                &self.ffmpeg,
                &input,
                page.cid,
                &output_path,
                self.app_dir(),
                resource_dir.as_deref(),
                cookies.as_ref(),
                &progress,
                &task_id,
            )
            .await;
            let completion_message = completion_message("DURL 分段下载完成", danmaku_result);
            emit_progress(
                &progress,
                &task_id,
                DownloadStage::Completed,
                1,
                1,
                started,
                Some(completion_message),
            );
            self.write_history_record(&input, &video.title, &output_path, "completed")?;
            return Ok(output_path);
        }

        Err(AppError::Download {
            task_id: Some(task_id),
            message: "没有找到可下载的视频流".to_string(),
        })
    }

    fn write_history_record(
        &self,
        input: &StartDownloadRequest,
        title: &str,
        output_path: &Path,
        status: &str,
    ) -> AppResult<()> {
        let max_history = self.config_store.load()?.max_history;
        self.history_store.record(
            max_history,
            HistoryItem {
                id: format!("hist_{}", Uuid::new_v4()),
                bvid: input.bvid.clone(),
                title: title.to_string(),
                quality: input.quality.clone(),
                format: input.format.clone(),
                output_path: output_path.to_string_lossy().into_owned(),
                timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                status: status.to_string(),
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_stein_download_task(
        &self,
        task_id: String,
        input: StartDownloadRequest,
        video: VideoData,
        resource_dir: Option<PathBuf>,
        cookies: Option<CookieSet>,
        cancel: Arc<AtomicBool>,
        progress: ProgressSender,
        started: Instant,
        qn: u32,
        max_workers: usize,
    ) -> AppResult<PathBuf> {
        if input.format == "audio" {
            return Err(AppError::Download {
                task_id: Some(task_id),
                message: "互动视频暂不支持纯音频导出，请选择视频格式".to_string(),
            });
        }

        progress(DownloadProgressEvent {
            task_id: task_id.clone(),
            stage: DownloadStage::Resolving,
            percent: 1.0,
            bytes_done: 0,
            bytes_total: 0,
            speed_bytes_per_sec: 0,
            message: Some("正在解析互动视频剧情图".to_string()),
        });

        let entry_cid = input
            .cid
            .or_else(|| video.pages.first().map(|p| p.cid))
            .ok_or_else(|| AppError::InvalidInput {
                message: "互动视频缺少入口 cid".to_string(),
            })?;

        let graph = match video.stein_graph.clone() {
            Some(g) if !g.segments.is_empty() => g,
            _ => {
                self.client
                    .crawl_stein_graph(&input.bvid, entry_cid, cookies.as_ref())
                    .await?
            }
        };

        let mode = match input.stein_mode {
            SteinDownloadMode::None | SteinDownloadMode::All => SteinDownloadMode::All,
            SteinDownloadMode::Path => SteinDownloadMode::Path,
        };

        let output_dir = PathBuf::from(input.outdir.trim());
        let safe_title = sanitize_filename::sanitize(&video.title);
        let work_dir = output_dir.join(format!("{}_{safe_title}_stein", input.bvid));
        let segments_dir = work_dir.join("segments");
        tokio::fs::create_dir_all(&segments_dir).await?;

        let graph_path = work_dir.join("graph.json");
        tokio::fs::write(
            &graph_path,
            serde_json::to_vec_pretty(&graph).map_err(|err| AppError::Io {
                message: format!("写入 graph.json 失败: {err}"),
            })?,
        )
        .await?;

        let (jobs, do_concat): (Vec<(u64, String)>, bool) = match mode {
            SteinDownloadMode::Path => {
                if input.stein_path_edges.is_empty() {
                    return Err(AppError::InvalidInput {
                        message: "路径下载请至少选择一个分支选项".to_string(),
                    });
                }
                let cids = graph
                    .resolve_path_cids(&input.stein_path_edges)
                    .map_err(|message| AppError::InvalidInput { message })?;
                let jobs = cids
                    .into_iter()
                    .enumerate()
                    .map(|(idx, cid)| {
                        let title = graph
                            .segments
                            .iter()
                            .find(|s| s.cid == cid)
                            .map(|s| s.title.clone())
                            .unwrap_or_else(|| format!("片段{cid}"));
                        (cid, format!("{:03}_{}", idx + 1, sanitize_filename::sanitize(&title)))
                    })
                    .collect();
                (jobs, true)
            }
            _ => {
                // 整图：固定 {cid}.mp4，便于离线播放器映射
                let jobs = graph
                    .segments
                    .iter()
                    .map(|seg| {
                        let title = sanitize_filename::sanitize(&seg.title);
                        let label = if title.is_empty() {
                            format!("{}", seg.cid)
                        } else {
                            format!("{}_{title}", seg.cid)
                        };
                        (seg.cid, label)
                    })
                    .collect();
                (jobs, false)
            }
        };

        if jobs.is_empty() {
            return Err(AppError::Download {
                task_id: Some(task_id),
                message: "剧情图中没有可下载分片".to_string(),
            });
        }

        let referer = format!("https://www.bilibili.com/video/{}", input.bvid);
        let cookie_header = cookies.as_ref().map(|c| c.to_header());
        let total = jobs.len() as f32;
        let mut output_files: Vec<PathBuf> = Vec::new();
        let mut cid_to_file: std::collections::HashMap<u64, String> =
            std::collections::HashMap::new();

        for (index, (cid, label)) in jobs.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(AppError::Cancelled {
                    task_id: Some(task_id),
                });
            }
            let base_percent = (index as f32 / total) * 95.0;
            progress(DownloadProgressEvent {
                task_id: task_id.clone(),
                stage: DownloadStage::DownloadingVideo,
                percent: base_percent,
                bytes_done: index as u64,
                bytes_total: jobs.len() as u64,
                speed_bytes_per_sec: 0,
                message: Some(format!(
                    "互动分片 {}/{}：{label} (cid={cid})",
                    index + 1,
                    jobs.len()
                )),
            });

            let playurl = self
                .client
                .playurl(&input.bvid, *cid, qn, cookies.as_ref())
                .await?;
            let safe_label = sanitize_filename::sanitize(label);
            let out_mp4 = segments_dir.join(format!("{safe_label}.mp4"));

            if let Some(dash) = playurl.dash {
                let video_track = select_video_track(&dash.video, qn).ok_or_else(|| {
                    AppError::Download {
                        task_id: Some(task_id.clone()),
                        message: format!("分片 {cid} 没有可下载视频流"),
                    }
                })?;
                let audio_track = select_audio_track(&dash.audio, &input.quality).ok_or_else(|| {
                    AppError::Download {
                        task_id: Some(task_id.clone()),
                        message: format!("分片 {cid} 没有可下载音频流"),
                    }
                })?;
                let video_tmp = segments_dir.join(format!("{safe_label}.video.m4s"));
                let audio_tmp = segments_dir.join(format!("{safe_label}.audio.m4a"));
                self.downloader
                    .download_file(
                        FileDownloadSpec {
                            task_id: task_id.clone(),
                            stage: DownloadStage::DownloadingVideo,
                            url: first_url(&video_track.base_url, &video_track.backup_urls)?,
                            backup_urls: video_track.backup_urls.clone(),
                            output_path: video_tmp.clone(),
                            referer: referer.clone(),
                            max_workers,
                            cookie_header: cookie_header.clone(),
                            content_length_hint: video_track.size_bytes,
                        },
                        cancel.clone(),
                        progress.clone(),
                    )
                    .await?;
                self.downloader
                    .download_file(
                        FileDownloadSpec {
                            task_id: task_id.clone(),
                            stage: DownloadStage::DownloadingAudio,
                            url: first_url(&audio_track.base_url, &audio_track.backup_urls)?,
                            backup_urls: audio_track.backup_urls.clone(),
                            output_path: audio_tmp.clone(),
                            referer: referer.clone(),
                            max_workers,
                            cookie_header: cookie_header.clone(),
                            content_length_hint: audio_track.size_bytes,
                        },
                        cancel.clone(),
                        progress.clone(),
                    )
                    .await?;
                if !input.skip_merge {
                    self.ffmpeg
                        .merge_video_audio(
                            self.app_dir(),
                            resource_dir.as_deref(),
                            &video_tmp,
                            &audio_tmp,
                            &out_mp4,
                        )
                        .await?;
                    let _ = tokio::fs::remove_file(&video_tmp).await;
                    let _ = tokio::fs::remove_file(&audio_tmp).await;
                    cid_to_file.insert(
                        *cid,
                        format!("segments/{}", out_mp4.file_name().unwrap().to_string_lossy()),
                    );
                    output_files.push(out_mp4);
                } else {
                    output_files.push(video_tmp);
                }
            } else if !playurl.durl.is_empty() {
                download_durl_segments(
                    &self.downloader,
                    &task_id,
                    &playurl.durl,
                    &out_mp4,
                    &referer,
                    cancel.clone(),
                    progress.clone(),
                    max_workers,
                    cookie_header.clone(),
                )
                .await?;
                cid_to_file.insert(
                    *cid,
                    format!("segments/{}", out_mp4.file_name().unwrap().to_string_lossy()),
                );
                output_files.push(out_mp4);
            } else {
                return Err(AppError::Download {
                    task_id: Some(task_id),
                    message: format!("分片 {cid} 没有可下载流"),
                });
            }
        }

        let final_path = if do_concat && !input.skip_merge && output_files.len() > 1 {
            progress(DownloadProgressEvent {
                task_id: task_id.clone(),
                stage: DownloadStage::Merging,
                percent: 96.0,
                bytes_done: jobs.len() as u64,
                bytes_total: jobs.len() as u64,
                speed_bytes_per_sec: 0,
                message: Some("正在拼接路径成片".to_string()),
            });
            let merged = work_dir.join(format!("{safe_title}_path.mp4"));
            self.ffmpeg
                .concat_videos(
                    self.app_dir(),
                    resource_dir.as_deref(),
                    &output_files,
                    &merged,
                )
                .await?;
            merged
        } else if !do_concat {
            // 整图：生成离线互动播放器
            progress(DownloadProgressEvent {
                task_id: task_id.clone(),
                stage: DownloadStage::Merging,
                percent: 97.0,
                bytes_done: jobs.len() as u64,
                bytes_total: jobs.len() as u64,
                speed_bytes_per_sec: 0,
                message: Some("正在生成离线互动播放器".to_string()),
            });
            // 重写 graph.json（含 play_cid / 选项时机）
            tokio::fs::write(
                &graph_path,
                serde_json::to_vec_pretty(&graph).map_err(|err| AppError::Io {
                    message: format!("写入 graph.json 失败: {err}"),
                })?,
            )
            .await?;
            let manifest = graph.to_player_manifest(&video.title, &input.bvid, &cid_to_file);
            let manifest_path = work_dir.join("player_manifest.json");
            tokio::fs::write(
                &manifest_path,
                serde_json::to_vec_pretty(&manifest).map_err(|err| AppError::Io {
                    message: format!("写入 player_manifest.json 失败: {err}"),
                })?,
            )
            .await?;
            let html = crate::models::stein::render_offline_player_html(&manifest).map_err(|err| {
                AppError::Io {
                    message: format!("生成播放器失败: {err}"),
                }
            })?;
            let play_path = work_dir.join("play.html");
            tokio::fs::write(&play_path, html).await?;
            let readme = format!(
                "【离线互动播放说明】\n\n\
                 1. 用浏览器打开本目录下的 play.html（推荐 Chrome / Edge）\n\
                 2. 不要单独移动 segments 文件夹，需与 play.html 保持相对路径\n\
                 3. 播放到分支点会暂停并弹出选项，点击后跳转下一分片\n\
                 4. 可「重新开始」或「回退一步」\n\n\
                 标题: {}\nBVID: {}\n分片数: {}\n节点数: {}\n",
                video.title,
                input.bvid,
                jobs.len(),
                graph.nodes.len()
            );
            tokio::fs::write(work_dir.join("如何播放.txt"), readme).await?;
            play_path
        } else if output_files.len() == 1 {
            output_files[0].clone()
        } else {
            work_dir.clone()
        };

        self.write_history_record(&input, &video.title, &final_path, "completed")?;

        let message = if do_concat {
            format!(
                "互动路径下载完成：{} 个分片已拼接 → {}",
                jobs.len(),
                final_path.to_string_lossy()
            )
        } else {
            format!(
                "互动整图完成：{} 分片 + 离线播放器 → 请用浏览器打开 {}",
                jobs.len(),
                final_path.to_string_lossy()
            )
        };
        emit_progress(
            &progress,
            &task_id,
            DownloadStage::Completed,
            1,
            1,
            started,
            Some(message),
        );
        Ok(final_path)
    }
}

#[derive(Debug)]
struct DanmakuProcessResult {
    ass_path: PathBuf,
    burned_into_video: bool,
}

fn completion_message(
    base: &str,
    danmaku_result: Option<Result<DanmakuProcessResult, String>>,
) -> String {
    match danmaku_result {
        Some(Ok(result)) if result.burned_into_video => {
            format!("{base}，弹幕已烧录到视频")
        }
        Some(Ok(result)) => format!("{base}，弹幕文件：{}", result.ass_path.to_string_lossy()),
        Some(Err(message)) => format!("{base}，但弹幕文件下载失败：{message}"),
        None => base.to_string(),
    }
}

async fn process_danmaku_if_requested(
    danmaku: &DanmakuClient,
    ffmpeg: &FfmpegManager,
    input: &StartDownloadRequest,
    cid: u64,
    output_path: &Path,
    app_dir: &Path,
    resource_dir: Option<&Path>,
    cookies: Option<&CookieSet>,
    progress: &ProgressSender,
    task_id: &str,
) -> Option<Result<DanmakuProcessResult, String>> {
    let mode = input.effective_danmaku_mode();
    if mode == DanmakuMode::None {
        return None;
    }

    progress(DownloadProgressEvent {
        task_id: task_id.to_string(),
        stage: DownloadStage::DownloadingDanmaku,
        percent: 99.0,
        bytes_done: 0,
        bytes_total: 0,
        speed_bytes_per_sec: 0,
        message: Some(match mode {
            DanmakuMode::Ass => "正在下载弹幕文件".to_string(),
            DanmakuMode::Burn => "正在下载弹幕文件，稍后烧录到视频".to_string(),
            DanmakuMode::None => unreachable!(),
        }),
    });

    let result = match danmaku
        .download_ass(cid, &input.bvid, output_path, cookies)
        .await
    {
        Ok(ass_path)
            if mode == DanmakuMode::Burn
                && output_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4")) =>
        {
            progress(DownloadProgressEvent {
                task_id: task_id.to_string(),
                stage: DownloadStage::BurningDanmaku,
                percent: 99.6,
                bytes_done: 0,
                bytes_total: 1,
                speed_bytes_per_sec: 0,
                message: Some("正在烧录弹幕到视频".to_string()),
            });
            match ffmpeg
                .burn_ass_subtitles(app_dir, resource_dir, output_path, &ass_path, output_path)
                .await
            {
                Ok(()) => {
                    let _ = tokio::fs::remove_file(&ass_path).await;
                    Ok(DanmakuProcessResult {
                        ass_path,
                        burned_into_video: true,
                    })
                }
                Err(err) => Err(format!("弹幕烧录失败：{err}")),
            }
        }
        Ok(ass_path) if mode == DanmakuMode::Burn => Ok(DanmakuProcessResult {
            ass_path,
            burned_into_video: false,
        }),
        Ok(ass_path) => Ok(DanmakuProcessResult {
            ass_path,
            burned_into_video: false,
        }),
        Err(err) => Err(err.to_string()),
    };

    match &result {
        Ok(result) if result.burned_into_video => progress(DownloadProgressEvent {
            task_id: task_id.to_string(),
            stage: DownloadStage::BurningDanmaku,
            percent: 99.9,
            bytes_done: 1,
            bytes_total: 1,
            speed_bytes_per_sec: 0,
            message: Some("弹幕已烧录到视频".to_string()),
        }),
        Ok(result) => progress(DownloadProgressEvent {
            task_id: task_id.to_string(),
            stage: DownloadStage::DownloadingDanmaku,
            percent: 99.5,
            bytes_done: 1,
            bytes_total: 1,
            speed_bytes_per_sec: 0,
            message: Some(format!(
                "弹幕文件已保存：{}",
                result.ass_path.to_string_lossy()
            )),
        }),
        Err(message) => progress(DownloadProgressEvent {
            task_id: task_id.to_string(),
            stage: DownloadStage::DownloadingDanmaku,
            percent: 99.5,
            bytes_done: 0,
            bytes_total: 1,
            speed_bytes_per_sec: 0,
            message: Some(format!("弹幕文件下载失败：{message}")),
        }),
    }
    Some(result)
}

fn danmaku_mode_suffix(input: &StartDownloadRequest) -> &'static str {
    match input.effective_danmaku_mode() {
        DanmakuMode::None => "",
        DanmakuMode::Ass => "，已选择外挂弹幕",
        DanmakuMode::Burn => "，已选择烧录弹幕",
    }
}

async fn download_durl_segments(
    downloader: &DownloadClient,
    task_id: &str,
    segments: &[DurlSegment],
    output_path: &Path,
    referer: &str,
    cancel: Arc<AtomicBool>,
    progress: ProgressSender,
    max_workers: usize,
    cookie_header: Option<String>,
) -> AppResult<()> {
    let temp_dir = output_path.with_extension("segments");
    tokio::fs::create_dir_all(&temp_dir).await?;
    let mut parts = Vec::new();
    let total_size: u64 = segments.iter().map(|segment| segment.size).sum();
    let started = Instant::now();
    let mut done = 0_u64;

    for (index, segment) in segments.iter().enumerate() {
        let part_path = temp_dir.join(format!("{index:04}.flv"));
        let result = downloader
            .download_file(
                FileDownloadSpec {
                    task_id: task_id.to_string(),
                    stage: DownloadStage::DownloadingSegments,
                    url: first_url(&segment.url, &segment.backup_urls)?,
                    backup_urls: segment.backup_urls.clone(),
                    output_path: part_path.clone(),
                    referer: referer.to_string(),
                    max_workers,
                    cookie_header: cookie_header.clone(),
                    content_length_hint: Some(segment.size),
                },
                cancel.clone(),
                progress.clone(),
            )
            .await?;
        done += if total_size > 0 {
            segment.size
        } else {
            result.bytes_written
        };
        emit_progress(
            &progress,
            task_id,
            DownloadStage::DownloadingSegments,
            done,
            total_size.max(done),
            started,
            None,
        );
        parts.push(part_path);
    }

    let temp_path = output_path.with_extension("tmp");
    let mut output = tokio::fs::File::create(&temp_path).await?;
    for part in parts {
        let mut file = tokio::fs::File::open(part).await?;
        tokio::io::copy(&mut file, &mut output).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut output).await?;
    if output_path.exists() {
        tokio::fs::remove_file(output_path).await?;
    }
    tokio::fs::rename(&temp_path, output_path).await?;
    let _ = tokio::fs::remove_dir_all(temp_dir).await;
    Ok(())
}
