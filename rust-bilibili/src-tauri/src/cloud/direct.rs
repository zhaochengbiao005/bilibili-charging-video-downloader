use std::{
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use tauri::AppHandle;
use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStdout, Command},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    auth::CookieSet,
    cloud::{
        baidu::{BaiduNetdiskUploader, BaiduTokenStore},
        session::CloudUploadSessionStore,
        uploader::CloudUploader,
    },
    danmaku::DanmakuClient,
    download::{
        bili_http::{md5_hex, BILIBILI_UA},
        ensure_not_cancelled, first_url, quality_to_qn, select_audio_track, select_video_track,
        selected_page, DownloadStage, TaskOrchestrator,
    },
    downloader::{progress_sender, ProgressSender},
    error::{AppError, AppResult},
    ffmpeg::{ffmpeg_filter_path, hide_child_window},
    models::{
        cloud::{
            CloudFilePlan, CloudFileResult, CloudProvider, CloudUploadMode, CloudUploadSession,
        },
        download::{DanmakuMode, DashTrack, DownloadProgressEvent, StartDownloadRequest},
        history::HistoryItem,
    },
    storage::{ConfigStore, HistoryStore},
    streaming::{
        bili_stream::{BiliStreamClient, BiliStreamSpec},
        md5_plan::{build_cloud_file_plan, Md5PlanInput},
    },
};

struct UploadTrackSpec {
    label: String,
    stage: DownloadStage,
    stream: BiliStreamSpec,
    remote_path: String,
    part_size: u64,
    content_type: Option<String>,
}

struct Mp4PipeSpec {
    task_id: String,
    ffmpeg_path: PathBuf,
    video_url: String,
    audio_url: String,
    referer: String,
    cookie_header: Option<String>,
    subtitle_path: Option<PathBuf>,
    remote_path: String,
    part_size: u64,
}

struct FfmpegMp4Process {
    child: Child,
    stdout: ChildStdout,
    stderr_task: JoinHandle<io::Result<Vec<u8>>>,
}

impl TaskOrchestrator {
    /// 云盘直传任务：上传器可注入（None = 百度网盘默认），便于测试时使用 mock。
    #[allow(clippy::too_many_arguments)]
    pub async fn run_cloud_upload_task(
        &self,
        task_id: String,
        input: StartDownloadRequest,
        app: AppHandle,
        resource_dir: Option<PathBuf>,
        cookies: Option<CookieSet>,
        cancel: Arc<AtomicBool>,
        uploader: Option<Arc<dyn CloudUploader>>,
    ) -> AppResult<Vec<CloudFileResult>> {
        ensure_not_cancelled(&cancel, &task_id)?;
        let cloud_config = self.baidu_token_store.load_config()?;
        let part_size = cloud_config.part_size_mb.clamp(1, 64) * 1024 * 1024;
        let remote_dir_source = input.outdir.trim();
        let remote_dir = normalize_remote_dir(if remote_dir_source.is_empty() {
            &cloud_config.default_remote_dir
        } else {
            remote_dir_source
        });
        let uploader: Arc<dyn CloudUploader> = uploader.unwrap_or_else(|| {
            Arc::new(BaiduNetdiskUploader::new(self.baidu_token_store.clone()))
        });
        let session_store = CloudUploadSessionStore::new(self.config_store.app_dir());
        let stream_client = BiliStreamClient::new()?;
        let progress = CloudProgress::new(progress_sender(app), task_id.clone());

        progress.emit(DownloadStage::Resolving, 0.0, 0, 0, "正在解析云盘直传任务".to_string());
        let video = self
            .client
            .video_info(&input.bvid, cookies.as_ref())
            .await?;
        let page = selected_page(&video, &input)?;
        let qn = quality_to_qn(&input.quality);
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
                task_id: Some(task_id.clone()),
                message: playurl.preview_block_message(meta_duration_sec),
            });
        }
        let dash = playurl.dash.ok_or_else(|| AppError::Download {
            task_id: Some(task_id.clone()),
            message: "云盘直传暂只支持 DASH 音视频流".to_string(),
        })?;
        let referer = format!("https://www.bilibili.com/video/{}", input.bvid);
        let cookie_header = cookies.as_ref().map(CookieSet::to_header);
        let safe_title = sanitize_filename::sanitize(format!("{}-{}", video.title, page.part));

        let mut outputs = Vec::new();
        if input.format == "audio" {
            let audio = select_audio_track(&dash.audio, &input.quality).ok_or_else(|| {
                AppError::Download {
                    task_id: Some(task_id.clone()),
                    message: "没有可上传的音频流".to_string(),
                }
            })?;
            let spec = track_spec(
                &task_id,
                "音频",
                DownloadStage::CloudUploadingAudio,
                &audio,
                &referer,
                cookie_header.clone(),
                format!("{remote_dir}/{safe_title}.m4a"),
                part_size,
            )?;
            outputs.push(
                upload_bili_track(
                    &stream_client,
                    uploader.as_ref(),
                    &session_store,
                    &progress,
                    &cancel,
                    spec,
                )
                .await?,
            );
        } else {
            let video_track =
                select_video_track(&dash.video, qn).ok_or_else(|| AppError::Download {
                    task_id: Some(task_id.clone()),
                    message: "没有可上传的视频流".to_string(),
                })?;
            let audio_track =
                select_audio_track(&dash.audio, &input.quality).ok_or_else(|| {
                    AppError::Download {
                        task_id: Some(task_id.clone()),
                        message: "没有可上传的音频流".to_string(),
                    }
                })?;
            if !input.skip_merge {
                let ffmpeg_path = self
                    .ffmpeg
                    .resolve_executable(self.app_dir(), resource_dir.as_deref())?;
                let subtitle_path = if input.effective_danmaku_mode() == DanmakuMode::Burn {
                    Some(
                        prepare_cloud_burn_subtitles(
                            &self.danmaku,
                            self.app_dir(),
                            page.cid,
                            &input.bvid,
                            cookies.as_ref(),
                            &progress,
                            &cancel,
                            &task_id,
                        )
                        .await?,
                    )
                } else {
                    None
                };
                let spec = Mp4PipeSpec {
                    task_id: task_id.clone(),
                    ffmpeg_path,
                    video_url: first_url(&video_track.base_url, &video_track.backup_urls)?,
                    audio_url: first_url(&audio_track.base_url, &audio_track.backup_urls)?,
                    referer: referer.clone(),
                    cookie_header: cookie_header.clone(),
                    subtitle_path: subtitle_path.clone(),
                    remote_path: format!("{remote_dir}/{safe_title}.mp4"),
                    part_size,
                };
                let result = upload_ffmpeg_mp4(
                    uploader.as_ref(),
                    &session_store,
                    &progress,
                    &cancel,
                    spec,
                )
                .await;
                if let Some(path) = subtitle_path {
                    let _ = tokio::fs::remove_file(path).await;
                }
                outputs.push(result?);
            } else {
                let video_spec = track_spec(
                    &task_id,
                    "视频",
                    DownloadStage::CloudUploadingVideo,
                    &video_track,
                    &referer,
                    cookie_header.clone(),
                    format!("{remote_dir}/{safe_title}-video-{}.m4s", video_track.id),
                    part_size,
                )?;
                outputs.push(
                    upload_bili_track(
                        &stream_client,
                        uploader.as_ref(),
                        &session_store,
                        &progress,
                        &cancel,
                        video_spec,
                    )
                    .await?,
                );

                let audio_spec = track_spec(
                    &task_id,
                    "音频",
                    DownloadStage::CloudUploadingAudio,
                    &audio_track,
                    &referer,
                    cookie_header.clone(),
                    format!("{remote_dir}/{safe_title}-audio.m4a"),
                    part_size,
                )?;
                outputs.push(
                    upload_bili_track(
                        &stream_client,
                        uploader.as_ref(),
                        &session_store,
                        &progress,
                        &cancel,
                        audio_spec,
                    )
                    .await?,
                );
            }
        }

        if input.effective_danmaku_mode() == DanmakuMode::Ass && input.format == "video" {
            progress.emit(
                DownloadStage::CloudUploadingDanmaku,
                94.0,
                0,
                1,
                "正在生成并上传弹幕文件".to_string(),
            );
            match self
                .danmaku
                .render_ass_text(page.cid, &input.bvid, cookies.as_ref())
                .await
            {
                Ok(ass) => {
                    let result = upload_memory_file(
                        uploader.as_ref(),
                        &session_store,
                        format!("{remote_dir}/{safe_title}.ass"),
                        ass.into_bytes(),
                        Some("text/plain; charset=utf-8".to_string()),
                        &cancel,
                    )
                    .await?;
                    outputs.push(result);
                }
                Err(err) => {
                    progress.emit(
                        DownloadStage::CloudUploadingDanmaku,
                        94.0,
                        0,
                        1,
                        format!("弹幕文件上传失败：{err}"),
                    );
                }
            }
        }

        let message = format!(
            "已保存到百度网盘：{}",
            outputs
                .iter()
                .map(|item| item.remote_path.as_str())
                .collect::<Vec<_>>()
                .join("；")
        );
        progress.emit(DownloadStage::Completed, 100.0, 1, 1, message);
        write_cloud_history(
            &self.history_store,
            &self.config_store,
            &input,
            &video.title,
            &outputs,
            "completed",
        )?;
        Ok(outputs)
    }
}

pub async fn upload_baidu_test_file(
    token_store: BaiduTokenStore,
    app_dir: PathBuf,
) -> AppResult<CloudFileResult> {
    let config = token_store.load_config()?;
    let remote_dir = normalize_remote_dir(&config.default_remote_dir);
    let uploader = BaiduNetdiskUploader::new(token_store);
    let session_store = CloudUploadSessionStore::new(app_dir);
    let bytes = format!(
        "B站充电视频下载器百度网盘上传测试\n{}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
    .into_bytes();
    upload_memory_file(
        &uploader,
        &session_store,
        format!(
            "{remote_dir}/cloud-upload-test-{}.txt",
            chrono::Local::now().format("%Y%m%d%H%M%S")
        ),
        bytes,
        Some("text/plain; charset=utf-8".to_string()),
        &Arc::new(AtomicBool::new(false)),
    )
    .await
}

async fn prepare_cloud_burn_subtitles(
    danmaku: &DanmakuClient,
    app_dir: &Path,
    cid: u64,
    bvid: &str,
    cookies: Option<&CookieSet>,
    progress: &CloudProgress,
    cancel: &Arc<AtomicBool>,
    task_id: &str,
) -> AppResult<PathBuf> {
    ensure_not_cancelled(cancel, task_id)?;
    progress.emit(
        DownloadStage::BurningDanmaku,
        8.0,
        0,
        0,
        "正在生成烧录弹幕文件".to_string(),
    );
    let ass = danmaku.render_ass_text(cid, bvid, cookies).await?;
    ensure_not_cancelled(cancel, task_id)?;
    let dir = app_dir.join("cloud-danmaku");
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{}.ass", Uuid::new_v4()));
    tokio::fs::write(&path, ass).await?;
    Ok(path)
}

async fn upload_bili_track(
    stream_client: &BiliStreamClient,
    uploader: &dyn CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: &CloudProgress,
    cancel: &Arc<AtomicBool>,
    spec: UploadTrackSpec,
) -> AppResult<CloudFileResult> {
    ensure_not_cancelled(cancel, &spec.stream.task_id)?;
    progress.emit(
        DownloadStage::CloudCalculatingMd5,
        12.0,
        0,
        0,
        format!("正在计算{}分片", spec.label),
    );
    let plan = build_cloud_file_plan(
        stream_client,
        &spec.stream,
        Md5PlanInput {
            provider: CloudProvider::BaiduNetdisk,
            mode: CloudUploadMode::RawDash,
            remote_path: spec.remote_path,
            part_size: spec.part_size,
            content_type: spec.content_type,
        },
        cancel,
    )
    .await?;

    upload_planned_stream(
        stream_client,
        uploader,
        session_store,
        progress,
        cancel,
        &spec.stream,
        plan,
        &spec.label,
        spec.stage,
    )
    .await
}

async fn get_or_create_session(
    uploader: &dyn CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: Option<&CloudProgress>,
    plan: &CloudFilePlan,
    label: &str,
) -> AppResult<CloudUploadSession> {
    if let Some(session) = find_reusable_session(session_store, plan)? {
        if let Some(progress) = progress {
            progress.emit(
                DownloadStage::CloudPrecreating,
                20.0,
                uploaded_bytes_for_plan(&session, plan),
                plan.size_bytes,
                format!("发现未完成{}上传，继续使用已保存会话", label),
            );
        }
        session_store.upsert(session.clone())?;
        return Ok(session);
    }

    let session = uploader.precreate(plan.clone()).await?;
    session_store.upsert(session.clone())?;
    Ok(session)
}

fn find_reusable_session(
    session_store: &CloudUploadSessionStore,
    plan: &CloudFilePlan,
) -> AppResult<Option<CloudUploadSession>> {
    let mut sessions = session_store.load()?;
    let Some(mut session) = sessions
        .drain(..)
        .find(|session| session_matches_plan(session, plan))
    else {
        return Ok(None);
    };
    normalize_uploaded_parts(&mut session, plan.expected_part_count());
    Ok(Some(session))
}

fn session_matches_plan(session: &CloudUploadSession, plan: &CloudFilePlan) -> bool {
    session.provider == plan.provider
        && session.remote_path == plan.remote_path
        && session.size_bytes == plan.size_bytes
        && session.part_size == plan.part_size
        && session.block_md5 == plan.block_md5
}

fn normalize_uploaded_parts(session: &mut CloudUploadSession, expected_part_count: usize) {
    session
        .uploaded_parts
        .retain(|part_index| *part_index < expected_part_count);
    session.uploaded_parts.sort_unstable();
    session.uploaded_parts.dedup();
}

fn uploaded_bytes_for_plan(session: &CloudUploadSession, plan: &CloudFilePlan) -> u64 {
    session
        .uploaded_parts
        .iter()
        .map(|part_index| part_size_at(plan, *part_index))
        .sum()
}

fn part_size_at(plan: &CloudFilePlan, part_index: usize) -> u64 {
    let start = part_index as u64 * plan.part_size;
    if start >= plan.size_bytes {
        return 0;
    }
    plan.part_size.min(plan.size_bytes - start)
}

fn progress_percent(base: f32, span: f32, bytes_done: u64, bytes_total: u64) -> f32 {
    base + (bytes_done as f32 / bytes_total.max(1) as f32 * span)
}

async fn upload_planned_stream(
    stream_client: &BiliStreamClient,
    uploader: &dyn CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: &CloudProgress,
    cancel: &Arc<AtomicBool>,
    stream: &BiliStreamSpec,
    plan: CloudFilePlan,
    label: &str,
    stage: DownloadStage,
) -> AppResult<CloudFileResult> {
    ensure_not_cancelled(cancel, &stream.task_id)?;
    progress.emit(
        DownloadStage::CloudPrecreating,
        20.0,
        0,
        plan.size_bytes,
        format!("正在创建{}上传任务", label),
    );
    let part_count = plan.block_md5.len();
    let mut session =
        get_or_create_session(uploader, session_store, Some(progress), &plan, label).await?;
    let mut bytes_done = uploaded_bytes_for_plan(&session, &plan);
    if bytes_done > 0 {
        progress.emit(
            stage,
            progress_percent(20.0, 70.0, bytes_done, plan.size_bytes),
            bytes_done,
            plan.size_bytes,
            format!(
                "发现{}已上传分片，继续上传 {}/{}",
                label,
                session.uploaded_parts.len(),
                part_count
            ),
        );
    }

    for part_index in 0..part_count {
        ensure_not_cancelled(cancel, &stream.task_id)?;
        if session.uploaded_parts.contains(&part_index) {
            progress.emit(
                stage,
                progress_percent(20.0, 70.0, bytes_done, plan.size_bytes),
                bytes_done,
                plan.size_bytes,
                format!("已跳过{}分片 {}/{}", label, part_index + 1, part_count),
            );
            continue;
        }

        let start = part_index as u64 * plan.part_size;
        let end = (start + plan.part_size - 1).min(plan.size_bytes.saturating_sub(1));
        let bytes = stream_client.read_range(stream, start, end, cancel).await?;
        let _ = uploader.upload_part(&session, part_index, bytes).await?;
        if !session.uploaded_parts.contains(&part_index) {
            session.uploaded_parts.push(part_index);
        }
        session_store.upsert(session.clone())?;
        bytes_done = bytes_done.saturating_add(part_size_at(&plan, part_index));
        let percent = 20.0 + (bytes_done as f32 / plan.size_bytes.max(1) as f32 * 70.0);
        progress.emit(
            stage,
            percent,
            bytes_done,
            plan.size_bytes,
            format!("正在上传{} {}/{}", label, part_index + 1, part_count),
        );
    }

    ensure_not_cancelled(cancel, &stream.task_id)?;
    let result = uploader.finish(session.clone()).await?;
    session_store.remove(&session.upload_id)?;
    Ok(result)
}

async fn upload_ffmpeg_mp4(
    uploader: &dyn CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: &CloudProgress,
    cancel: &Arc<AtomicBool>,
    spec: Mp4PipeSpec,
) -> AppResult<CloudFileResult> {
    let plan = build_ffmpeg_mp4_plan(&spec, progress, cancel).await?;
    progress.emit(
        DownloadStage::CloudPrecreating,
        40.0,
        0,
        plan.size_bytes,
        "正在创建 MP4 上传任务".to_string(),
    );
    let mut session =
        get_or_create_session(uploader, session_store, Some(progress), &plan, "MP4").await?;

    progress.emit(
        DownloadStage::CloudUploadingVideo,
        42.0,
        0,
        plan.size_bytes,
        "正在上传 MP4 到百度网盘".to_string(),
    );
    let mut process = spawn_ffmpeg_mp4(&spec)?;
    let mut read_buffer = vec![0_u8; 256 * 1024];
    let mut part_buffer = Vec::with_capacity(spec.part_size as usize);
    let mut part_index = 0_usize;
    let mut bytes_done = uploaded_bytes_for_plan(&session, &plan);
    if bytes_done > 0 {
        progress.emit(
            DownloadStage::CloudUploadingVideo,
            progress_percent(42.0, 53.0, bytes_done, plan.size_bytes),
            bytes_done,
            plan.size_bytes,
            format!(
                "发现 MP4 已上传分片，继续上传 {}/{}",
                session.uploaded_parts.len(),
                plan.block_md5.len()
            ),
        );
    }

    loop {
        ensure_ffmpeg_not_cancelled(cancel, &spec.task_id, &mut process).await?;
        let read = process.stdout.read(&mut read_buffer).await?;
        if read == 0 {
            break;
        }

        let mut offset = 0_usize;
        while offset < read {
            let capacity = spec.part_size as usize - part_buffer.len();
            let take = capacity.min(read - offset);
            part_buffer.extend_from_slice(&read_buffer[offset..offset + take]);
            offset += take;

            if part_buffer.len() == spec.part_size as usize {
                let part_bytes = std::mem::replace(
                    &mut part_buffer,
                    Vec::with_capacity(spec.part_size as usize),
                );
                upload_mp4_part(
                    uploader,
                    session_store,
                    progress,
                    &mut session,
                    part_index,
                    part_bytes,
                    plan.block_md5.get(part_index).map(String::as_str),
                    &mut bytes_done,
                    plan.size_bytes,
                )
                .await?;
                part_index += 1;
            }
        }
    }

    if !part_buffer.is_empty() {
        let part_bytes = std::mem::take(&mut part_buffer);
        upload_mp4_part(
            uploader,
            session_store,
            progress,
            &mut session,
            part_index,
            part_bytes,
            plan.block_md5.get(part_index).map(String::as_str),
            &mut bytes_done,
            plan.size_bytes,
        )
        .await?;
        part_index += 1;
    }

    finish_ffmpeg_mp4(process).await?;
    if part_index != plan.block_md5.len() {
        return Err(AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: "MP4 上传分片数量与上传计划不一致，请重试".to_string(),
        });
    }

    let result = uploader.finish(session.clone()).await?;
    session_store.remove(&session.upload_id)?;
    Ok(result)
}

async fn upload_mp4_part(
    uploader: &dyn CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: &CloudProgress,
    session: &mut crate::models::cloud::CloudUploadSession,
    part_index: usize,
    bytes: Vec<u8>,
    expected_md5: Option<&str>,
    bytes_done: &mut u64,
    bytes_total: u64,
) -> AppResult<()> {
    if session.uploaded_parts.contains(&part_index) {
        progress.emit(
            DownloadStage::CloudUploadingVideo,
            progress_percent(42.0, 53.0, *bytes_done, bytes_total),
            *bytes_done,
            bytes_total,
            format!("已跳过 MP4 分片 {}", part_index + 1),
        );
        return Ok(());
    }

    let Some(expected_md5) = expected_md5 else {
        return Err(AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: "MP4 第二遍输出超出上传计划，请重新发起任务".to_string(),
        });
    };
    let actual_md5 = md5_hex(&bytes);
    if actual_md5 != expected_md5 {
        return Err(AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: "MP4 第二遍输出与上传计划不一致，请重新发起任务".to_string(),
        });
    }

    *bytes_done = bytes_done.saturating_add(bytes.len() as u64);
    let _ = uploader.upload_part(session, part_index, bytes).await?;
    if !session.uploaded_parts.contains(&part_index) {
        session.uploaded_parts.push(part_index);
    }
    session_store.upsert(session.clone())?;
    let percent = 42.0 + (*bytes_done as f32 / bytes_total.max(1) as f32 * 53.0);
    progress.emit(
        DownloadStage::CloudUploadingVideo,
        percent,
        *bytes_done,
        bytes_total,
        format!("正在上传 MP4 分片 {}", part_index + 1),
    );
    Ok(())
}

async fn build_ffmpeg_mp4_plan(
    spec: &Mp4PipeSpec,
    progress: &CloudProgress,
    cancel: &Arc<AtomicBool>,
) -> AppResult<CloudFilePlan> {
    ensure_not_cancelled(cancel, &spec.task_id)?;
    progress.emit(
        DownloadStage::CloudMuxingMp4,
        10.0,
        0,
        0,
        "正在生成 MP4 上传计划".to_string(),
    );

    let mut process = spawn_ffmpeg_mp4(spec)?;
    let mut read_buffer = vec![0_u8; 256 * 1024];
    let mut part_buffer = Vec::with_capacity(spec.part_size as usize);
    let mut block_md5 = Vec::new();
    let mut size_bytes = 0_u64;

    loop {
        ensure_ffmpeg_not_cancelled(cancel, &spec.task_id, &mut process).await?;
        let read = process.stdout.read(&mut read_buffer).await?;
        if read == 0 {
            break;
        }
        size_bytes = size_bytes.saturating_add(read as u64);
        push_part_md5(
            &mut part_buffer,
            &read_buffer[..read],
            spec.part_size as usize,
            &mut block_md5,
        );
        progress.emit(
            DownloadStage::CloudMuxingMp4,
            10.0,
            size_bytes,
            0,
            format!("正在计算 MP4 分片，已处理 {}", format_bytes(size_bytes)),
        );
    }

    if !part_buffer.is_empty() {
        block_md5.push(md5_hex(&part_buffer));
    }
    finish_ffmpeg_mp4(process).await?;

    if size_bytes == 0 || block_md5.is_empty() {
        return Err(AppError::Merge {
            message: "FFmpeg 没有生成可上传的 MP4 数据".to_string(),
        });
    }

    Ok(CloudFilePlan {
        provider: CloudProvider::BaiduNetdisk,
        mode: CloudUploadMode::Mp4PipeExperimental,
        remote_path: spec.remote_path.clone(),
        size_bytes,
        part_size: spec.part_size,
        block_md5,
        content_type: Some("video/mp4".to_string()),
    })
}

async fn upload_memory_file(
    uploader: &dyn CloudUploader,
    session_store: &CloudUploadSessionStore,
    remote_path: String,
    bytes: Vec<u8>,
    content_type: Option<String>,
    cancel: &Arc<AtomicBool>,
) -> AppResult<CloudFileResult> {
    ensure_not_cancelled(cancel, "cloud_memory_upload")?;
    let bytes = if bytes.is_empty() {
        b"\n".to_vec()
    } else {
        bytes
    };
    let md5 = md5_hex(&bytes);
    let plan = CloudFilePlan {
        provider: CloudProvider::BaiduNetdisk,
        mode: CloudUploadMode::RawDash,
        remote_path,
        size_bytes: bytes.len() as u64,
        part_size: bytes.len().max(1) as u64,
        block_md5: vec![md5],
        content_type,
    };
    let mut session = get_or_create_session(uploader, session_store, None, &plan, "小文件").await?;
    if !session.uploaded_parts.contains(&0) {
        let _ = uploader.upload_part(&session, 0, bytes).await?;
        session.uploaded_parts.push(0);
        session_store.upsert(session.clone())?;
    }
    let result = uploader.finish(session.clone()).await?;
    session_store.remove(&session.upload_id)?;
    Ok(result)
}

fn spawn_ffmpeg_mp4(spec: &Mp4PipeSpec) -> AppResult<FfmpegMp4Process> {
    let headers = ffmpeg_input_headers(&spec.referer, spec.cookie_header.as_deref());
    let mut command = Command::new(&spec.ffmpeg_path);
    hide_child_window(&mut command);
    for arg in ffmpeg_mp4_args(spec, &headers) {
        command.arg(arg);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                AppError::FfmpegNotFound
            } else {
                AppError::Io {
                    message: err.to_string(),
                }
            }
        })?;

    let stdout = child.stdout.take().ok_or_else(|| AppError::Merge {
        message: "无法读取 FFmpeg MP4 输出".to_string(),
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| AppError::Merge {
        message: "无法读取 FFmpeg 错误输出".to_string(),
    })?;
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await?;
        Ok(bytes)
    });

    Ok(FfmpegMp4Process {
        child,
        stdout,
        stderr_task,
    })
}

fn ffmpeg_mp4_args(spec: &Mp4PipeSpec, headers: &str) -> Vec<String> {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-fflags".to_string(),
        "+bitexact".to_string(),
        "-headers".to_string(),
        headers.to_string(),
        "-i".to_string(),
        spec.video_url.clone(),
        "-headers".to_string(),
        headers.to_string(),
        "-i".to_string(),
        spec.audio_url.clone(),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "1:a:0".to_string(),
        "-map_metadata".to_string(),
        "-1".to_string(),
        "-map_chapters".to_string(),
        "-1".to_string(),
        "-flags".to_string(),
        "+bitexact".to_string(),
    ];

    if let Some(subtitle_path) = &spec.subtitle_path {
        args.extend([
            "-vf".to_string(),
            format!("subtitles={}", ffmpeg_filter_path(subtitle_path)),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "veryfast".to_string(),
            "-crf".to_string(),
            "20".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-c:a".to_string(),
            "copy".to_string(),
        ]);
    } else {
        args.extend(["-c".to_string(), "copy".to_string()]);
    }

    args.extend([
        "-shortest".to_string(),
        "-movflags".to_string(),
        "frag_keyframe+empty_moov+default_base_moof".to_string(),
        "-f".to_string(),
        "mp4".to_string(),
        "pipe:1".to_string(),
    ]);
    args
}

async fn finish_ffmpeg_mp4(mut process: FfmpegMp4Process) -> AppResult<()> {
    let status = process.child.wait().await.map_err(|err| AppError::Io {
        message: err.to_string(),
    })?;
    let stderr = process
        .stderr_task
        .await
        .map_err(|err| AppError::Io {
            message: err.to_string(),
        })?
        .map_err(|err| AppError::Io {
            message: err.to_string(),
        })?;
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
        return Err(AppError::Merge {
            message: if stderr.is_empty() {
                format!("FFmpeg 退出码 {status}")
            } else {
                stderr
            },
        });
    }
    Ok(())
}

async fn ensure_ffmpeg_not_cancelled(
    cancel: &Arc<AtomicBool>,
    task_id: &str,
    process: &mut FfmpegMp4Process,
) -> AppResult<()> {
    if cancel.load(Ordering::SeqCst) {
        let _ = process.child.kill().await;
        return Err(AppError::Cancelled {
            task_id: Some(task_id.to_string()),
        });
    }
    Ok(())
}

fn push_part_md5(
    part_buffer: &mut Vec<u8>,
    bytes: &[u8],
    part_size: usize,
    block_md5: &mut Vec<String>,
) {
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let capacity = part_size - part_buffer.len();
        let take = capacity.min(bytes.len() - offset);
        part_buffer.extend_from_slice(&bytes[offset..offset + take]);
        offset += take;

        if part_buffer.len() == part_size {
            block_md5.push(md5_hex(part_buffer));
            part_buffer.clear();
        }
    }
}

fn ffmpeg_input_headers(referer: &str, cookie_header: Option<&str>) -> String {
    let mut headers = format!(
        "Referer: {referer}\r\nOrigin: https://www.bilibili.com\r\nUser-Agent: {BILIBILI_UA}\r\n"
    );
    if let Some(cookie_header) = cookie_header.filter(|value| !value.trim().is_empty()) {
        headers.push_str("Cookie: ");
        headers.push_str(cookie_header);
        headers.push_str("\r\n");
    }
    headers
}

fn format_bytes(bytes: u64) -> String {
    let mib = bytes as f64 / 1024.0 / 1024.0;
    if mib < 100.0 {
        format!("{mib:.1} MB")
    } else {
        format!("{mib:.0} MB")
    }
}

fn track_spec(
    task_id: &str,
    label: &str,
    stage: DownloadStage,
    track: &DashTrack,
    referer: &str,
    cookie_header: Option<String>,
    remote_path: String,
    part_size: u64,
) -> AppResult<UploadTrackSpec> {
    Ok(UploadTrackSpec {
        label: label.to_string(),
        stage,
        stream: BiliStreamSpec {
            task_id: task_id.to_string(),
            url: first_url(&track.base_url, &track.backup_urls)?,
            referer: referer.to_string(),
            cookie_header,
        },
        remote_path,
        part_size,
        content_type: track.mime_type.clone(),
    })
}

fn write_cloud_history(
    history_store: &HistoryStore,
    config_store: &ConfigStore,
    input: &StartDownloadRequest,
    title: &str,
    outputs: &[CloudFileResult],
    status: &str,
) -> AppResult<()> {
    let max_history = config_store.load()?.max_history.max(1);
    let mut items = history_store.load()?;
    items.insert(
        0,
        HistoryItem {
            id: format!("hist_{}", Uuid::new_v4()),
            bvid: input.bvid.clone(),
            title: title.to_string(),
            quality: input.quality.clone(),
            format: format!("cloud_{}", input.format),
            output_path: outputs
                .iter()
                .map(|item| item.remote_path.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            status: status.to_string(),
        },
    );
    items.truncate(max_history);
    history_store.save(&items)
}

fn normalize_remote_dir(remote_dir: &str) -> String {
    let trimmed = remote_dir.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "/apps/B站充电视频下载器".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

struct CloudProgress {
    sender: ProgressSender,
    task_id: String,
}

impl CloudProgress {
    fn new(sender: ProgressSender, task_id: String) -> Self {
        Self { sender, task_id }
    }

    fn emit(
        &self,
        stage: DownloadStage,
        percent: f32,
        bytes_done: u64,
        bytes_total: u64,
        message: String,
    ) {
        (self.sender)(DownloadProgressEvent {
            task_id: self.task_id.clone(),
            stage,
            percent: percent.clamp(0.0, 100.0),
            bytes_done,
            bytes_total,
            speed_bytes_per_sec: 0,
            message: Some(message),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn normalize_remote_dir_adds_leading_slash_and_removes_trailing_slash() {
        assert_eq!(normalize_remote_dir("apps/demo/"), "/apps/demo");
        assert_eq!(normalize_remote_dir("/apps/demo/"), "/apps/demo");
        assert_eq!(normalize_remote_dir(""), "/apps/B站充电视频下载器");
    }

    fn cloud_plan() -> CloudFilePlan {
        CloudFilePlan {
            provider: CloudProvider::BaiduNetdisk,
            mode: CloudUploadMode::RawDash,
            remote_path: "/apps/demo/video.m4s".to_string(),
            size_bytes: 10,
            part_size: 4,
            block_md5: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            content_type: Some("video/mp4".to_string()),
        }
    }

    fn mp4_pipe_spec(subtitle_path: Option<PathBuf>) -> Mp4PipeSpec {
        Mp4PipeSpec {
            task_id: "task-1".to_string(),
            ffmpeg_path: PathBuf::from("ffmpeg.exe"),
            video_url: "https://example.test/video.m4s".to_string(),
            audio_url: "https://example.test/audio.m4a".to_string(),
            referer: "https://www.bilibili.com/video/BV1".to_string(),
            cookie_header: None,
            subtitle_path,
            remote_path: "/apps/demo/video.mp4".to_string(),
            part_size: 4 * 1024 * 1024,
        }
    }

    #[test]
    fn find_reusable_session_normalizes_uploaded_parts() -> AppResult<()> {
        let root = std::env::temp_dir().join(format!("bili_cloud_resume_{}", Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let store = CloudUploadSessionStore::new(&root);
        let plan = cloud_plan();
        store.upsert(CloudUploadSession {
            provider: CloudProvider::BaiduNetdisk,
            upload_id: "upload-1".to_string(),
            remote_path: plan.remote_path.clone(),
            size_bytes: plan.size_bytes,
            part_size: plan.part_size,
            block_md5: plan.block_md5.clone(),
            uploaded_parts: vec![2, 0, 2, 9],
        })?;

        let session = find_reusable_session(&store, &plan)?.expect("session should match");

        assert_eq!(session.uploaded_parts, vec![0, 2]);
        assert_eq!(uploaded_bytes_for_plan(&session, &plan), 6);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn session_matches_plan_rejects_different_block_list() {
        let plan = cloud_plan();
        let mut session = CloudUploadSession {
            provider: CloudProvider::BaiduNetdisk,
            upload_id: "upload-1".to_string(),
            remote_path: plan.remote_path.clone(),
            size_bytes: plan.size_bytes,
            part_size: plan.part_size,
            block_md5: plan.block_md5.clone(),
            uploaded_parts: vec![],
        };

        assert!(session_matches_plan(&session, &plan));
        session.block_md5 = vec!["different".to_string()];
        assert!(!session_matches_plan(&session, &plan));
    }

    #[test]
    fn ffmpeg_mp4_args_copy_streams_without_subtitles() {
        let args = ffmpeg_mp4_args(&mp4_pipe_spec(None), "Referer: demo\r\n");

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-c" && pair[1] == "copy"));
        assert!(!args.iter().any(|arg| arg == "-vf"));
        assert!(args.iter().any(|arg| arg == "pipe:1"));
    }

    #[test]
    fn ffmpeg_mp4_args_burns_subtitles_when_requested() {
        let args = ffmpeg_mp4_args(
            &mp4_pipe_spec(Some(PathBuf::from("C:/Videos/demo subtitle.ass"))),
            "Referer: demo\r\n",
        );

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-c:v" && pair[1] == "libx264"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-c:a" && pair[1] == "copy"));
        assert!(args
            .iter()
            .any(|arg| arg == "subtitles='C\\:/Videos/demo subtitle.ass'"));
    }

    #[tokio::test]
    async fn upload_bili_track_round_trips_stream_through_mock_uploader() -> AppResult<()> {
        use crate::cloud::mock::MockCloudUploader;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // 假 CDN：HEAD 一律 404（逼出客户端的 Range 探测），GET Range 回 206 + 对应字节。
        let body: &[u8] = b"abcdefgh";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                loop {
                    let mut buffer = vec![0_u8; 8192];
                    let read = socket.read(&mut buffer).await.unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    let request = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
                    if let Some(position) = request.find("range: bytes=") {
                        let digits = &request[position + "range: bytes=".len()..];
                        let digits: String = digits
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || *c == '-')
                            .collect();
                        let (start, end) = digits.split_once('-').unwrap();
                        let (start, end) =
                            (start.parse::<u64>().unwrap(), end.parse::<u64>().unwrap());
                        let slice = &body[start as usize..=(end as usize)];
                        let header = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nConnection: close\r\n\r\n",
                            slice.len(),
                            body.len()
                        );
                        let _ = socket.write_all(header.as_bytes()).await;
                        let _ = socket.write_all(slice).await;
                    } else {
                        let _ = socket
                            .write_all(
                                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                            .await;
                    }
                }
            }
        });

        let uploader = Arc::new(MockCloudUploader::authorized("测试账号"));
        let root = std::env::temp_dir().join(format!("bili_cloud_track_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await?;
        let session_store = CloudUploadSessionStore::new(&root);
        let stream_client = BiliStreamClient::new()?;

        let events: Arc<std::sync::Mutex<Vec<DownloadProgressEvent>>> = Arc::default();
        let events_for_sender = events.clone();
        let progress = CloudProgress::new(
            Arc::new(move |event| {
                events_for_sender.lock().unwrap().push(event);
            }),
            "task-1".to_string(),
        );

        let spec = UploadTrackSpec {
            label: "视频".to_string(),
            stage: DownloadStage::CloudUploadingVideo,
            stream: BiliStreamSpec {
                task_id: "task-1".to_string(),
                url: format!("http://{addr}/video.m4s"),
                referer: "https://www.bilibili.com/video/BVtest".to_string(),
                cookie_header: None,
            },
            remote_path: "/apps/demo/video.m4s".to_string(),
            part_size: 3,
            content_type: Some("video/mp4".to_string()),
        };

        let result = upload_bili_track(
            &stream_client,
            uploader.as_ref(),
            &session_store,
            &progress,
            &Arc::new(AtomicBool::new(false)),
            spec,
        )
        .await?;

        assert_eq!(result.remote_path, "/apps/demo/video.m4s");
        assert_eq!(result.size_bytes, 8);
        assert_eq!(uploader.completed_files()?.len(), 1);
        assert!(session_store.load()?.is_empty());
        assert!(events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.stage == DownloadStage::CloudUploadingVideo && event.bytes_done == 8));

        server.abort();
        let _ = tokio::fs::remove_dir_all(&root).await;
        Ok(())
    }
}
