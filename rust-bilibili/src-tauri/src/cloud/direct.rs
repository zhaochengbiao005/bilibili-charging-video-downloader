use std::{
    io,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use tauri::{AppHandle, Emitter};
use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStdout, Command},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    api::BilibiliClient,
    auth::CookieSet,
    cloud::{
        baidu::{BaiduNetdiskUploader, BaiduTokenStore},
        session::CloudUploadSessionStore,
        uploader::CloudUploader,
    },
    danmaku::DanmakuClient,
    error::{AppError, AppResult},
    ffmpeg::FfmpegManager,
    models::{
        cloud::{CloudFilePlan, CloudFileResult, CloudProvider, CloudUploadMode},
        download::{DanmakuMode, DashTrack, DownloadProgressEvent, StartDownloadRequest},
        history::HistoryItem,
        video::VideoData,
    },
    storage::{ConfigStore, HistoryStore},
    streaming::{
        bili_stream::{BiliStreamClient, BiliStreamSpec},
        md5_plan::{build_cloud_file_plan, Md5PlanInput},
    },
};

pub struct CloudUploadTask {
    pub task_id: String,
    pub input: StartDownloadRequest,
    pub app: AppHandle,
    pub client: BilibiliClient,
    pub danmaku: DanmakuClient,
    pub token_store: BaiduTokenStore,
    pub history_store: HistoryStore,
    pub config_store: ConfigStore,
    pub ffmpeg: FfmpegManager,
    pub app_dir: PathBuf,
    pub resource_dir: Option<PathBuf>,
    pub cookies: Option<CookieSet>,
    pub cancel: Arc<AtomicBool>,
}

struct UploadTrackSpec {
    label: String,
    stage: &'static str,
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
    remote_path: String,
    part_size: u64,
}

struct FfmpegMp4Process {
    child: Child,
    stdout: ChildStdout,
    stderr_task: JoinHandle<io::Result<Vec<u8>>>,
}

pub async fn run_cloud_upload_task(task: CloudUploadTask) -> AppResult<Vec<CloudFileResult>> {
    ensure_not_cancelled(&task.cancel, &task.task_id)?;
    let cloud_config = task.token_store.load_config()?;
    let part_size = cloud_config.part_size_mb.clamp(1, 64) * 1024 * 1024;
    let remote_dir_source = task.input.outdir.trim();
    let remote_dir = normalize_remote_dir(if remote_dir_source.is_empty() {
        &cloud_config.default_remote_dir
    } else {
        remote_dir_source
    });
    let uploader = BaiduNetdiskUploader::new(task.token_store.clone());
    let session_store = CloudUploadSessionStore::new(task.config_store.app_dir());
    let stream_client = BiliStreamClient::new()?;
    let progress = CloudProgress::new(task.app.clone(), task.task_id.clone());

    progress.emit("resolving", 0.0, 0, 0, "正在解析云盘直传任务".to_string());
    let video = task
        .client
        .video_info(&task.input.bvid, task.cookies.as_ref())
        .await?;
    let page = selected_page(&video, &task.input)?;
    let qn = quality_to_qn(&task.input.quality);
    let playurl = task
        .client
        .playurl(&task.input.bvid, page.cid, qn, task.cookies.as_ref())
        .await?;
    let dash = playurl.dash.ok_or_else(|| AppError::Download {
        task_id: Some(task.task_id.clone()),
        message: "云盘直传暂只支持 DASH 音视频流".to_string(),
    })?;
    let referer = format!("https://www.bilibili.com/video/{}", task.input.bvid);
    let cookie_header = task.cookies.as_ref().map(CookieSet::to_header);
    let safe_title = sanitize_filename::sanitize(format!("{}-{}", video.title, page.part));

    let mut outputs = Vec::new();
    if task.input.format == "audio" {
        let audio = select_audio_track(&dash.audio, &task.input.quality).ok_or_else(|| {
            AppError::Download {
                task_id: Some(task.task_id.clone()),
                message: "没有可上传的音频流".to_string(),
            }
        })?;
        let spec = track_spec(
            &task.task_id,
            "音频",
            "cloud_uploading_audio",
            &audio,
            &referer,
            cookie_header.clone(),
            format!("{remote_dir}/{safe_title}.m4a"),
            part_size,
        )?;
        outputs.push(
            upload_bili_track(
                &stream_client,
                &uploader,
                &session_store,
                &progress,
                &task.cancel,
                spec,
            )
            .await?,
        );
    } else {
        let video_track =
            select_video_track(&dash.video, qn).ok_or_else(|| AppError::Download {
                task_id: Some(task.task_id.clone()),
                message: "没有可上传的视频流".to_string(),
            })?;
        let audio_track =
            select_audio_track(&dash.audio, &task.input.quality).ok_or_else(|| {
                AppError::Download {
                    task_id: Some(task.task_id.clone()),
                    message: "没有可上传的音频流".to_string(),
                }
            })?;
        if !task.input.skip_merge {
            let ffmpeg_path = task
                .ffmpeg
                .resolve_executable(&task.app_dir, task.resource_dir.as_deref())?;
            let spec = Mp4PipeSpec {
                task_id: task.task_id.clone(),
                ffmpeg_path,
                video_url: first_url(&video_track.base_url, &video_track.backup_urls)?,
                audio_url: first_url(&audio_track.base_url, &audio_track.backup_urls)?,
                referer: referer.clone(),
                cookie_header: cookie_header.clone(),
                remote_path: format!("{remote_dir}/{safe_title}.mp4"),
                part_size,
            };
            outputs.push(
                upload_ffmpeg_mp4(&uploader, &session_store, &progress, &task.cancel, spec).await?,
            );
        } else {
            let video_spec = track_spec(
                &task.task_id,
                "视频",
                "cloud_uploading_video",
                &video_track,
                &referer,
                cookie_header.clone(),
                format!("{remote_dir}/{safe_title}-video-{}.m4s", video_track.id),
                part_size,
            )?;
            outputs.push(
                upload_bili_track(
                    &stream_client,
                    &uploader,
                    &session_store,
                    &progress,
                    &task.cancel,
                    video_spec,
                )
                .await?,
            );

            let audio_spec = track_spec(
                &task.task_id,
                "音频",
                "cloud_uploading_audio",
                &audio_track,
                &referer,
                cookie_header.clone(),
                format!("{remote_dir}/{safe_title}-audio.m4a"),
                part_size,
            )?;
            outputs.push(
                upload_bili_track(
                    &stream_client,
                    &uploader,
                    &session_store,
                    &progress,
                    &task.cancel,
                    audio_spec,
                )
                .await?,
            );
        }
    }

    if task.input.effective_danmaku_mode() != DanmakuMode::None && task.input.format == "video" {
        progress.emit(
            "cloud_uploading_danmaku",
            94.0,
            0,
            1,
            "正在生成并上传弹幕文件".to_string(),
        );
        match task
            .danmaku
            .render_ass_text(page.cid, &task.input.bvid, task.cookies.as_ref())
            .await
        {
            Ok(ass) => {
                let result = upload_memory_file(
                    &uploader,
                    &session_store,
                    format!("{remote_dir}/{safe_title}.ass"),
                    ass.into_bytes(),
                    Some("text/plain; charset=utf-8".to_string()),
                    &task.cancel,
                )
                .await?;
                outputs.push(result);
            }
            Err(err) => {
                progress.emit(
                    "cloud_uploading_danmaku",
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
    progress.emit("completed", 100.0, 1, 1, message);
    write_cloud_history(
        &task.history_store,
        &task.config_store,
        &task.input,
        &video.title,
        &outputs,
        "completed",
    )?;
    Ok(outputs)
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

async fn upload_bili_track(
    stream_client: &BiliStreamClient,
    uploader: &impl CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: &CloudProgress,
    cancel: &Arc<AtomicBool>,
    spec: UploadTrackSpec,
) -> AppResult<CloudFileResult> {
    ensure_not_cancelled(cancel, &spec.stream.task_id)?;
    progress.emit(
        "cloud_calculating_md5",
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

async fn upload_planned_stream(
    stream_client: &BiliStreamClient,
    uploader: &impl CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: &CloudProgress,
    cancel: &Arc<AtomicBool>,
    stream: &BiliStreamSpec,
    plan: CloudFilePlan,
    label: &str,
    stage: &str,
) -> AppResult<CloudFileResult> {
    ensure_not_cancelled(cancel, &stream.task_id)?;
    progress.emit(
        "cloud_precreating",
        20.0,
        0,
        plan.size_bytes,
        format!("正在创建{}上传任务", label),
    );
    let mut session = uploader.precreate(plan.clone()).await?;
    session_store.upsert(session.clone())?;

    let part_count = plan.block_md5.len();
    for part_index in 0..part_count {
        ensure_not_cancelled(cancel, &stream.task_id)?;
        let start = part_index as u64 * plan.part_size;
        let end = (start + plan.part_size - 1).min(plan.size_bytes.saturating_sub(1));
        let bytes = stream_client.read_range(stream, start, end, cancel).await?;
        let _ = uploader.upload_part(&session, part_index, bytes).await?;
        if !session.uploaded_parts.contains(&part_index) {
            session.uploaded_parts.push(part_index);
        }
        session_store.upsert(session.clone())?;
        let bytes_done = ((part_index + 1) as u64 * plan.part_size).min(plan.size_bytes);
        let percent = 20.0 + (bytes_done as f32 / plan.size_bytes.max(1) as f32 * 70.0);
        progress.emit(
            stage,
            percent,
            bytes_done,
            plan.size_bytes,
            format!("正在上传{} {}/{}", label, part_index + 1, part_count),
        );
    }

    let result = uploader.finish(session.clone()).await?;
    session_store.remove(&session.upload_id)?;
    Ok(result)
}

async fn upload_ffmpeg_mp4(
    uploader: &impl CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: &CloudProgress,
    cancel: &Arc<AtomicBool>,
    spec: Mp4PipeSpec,
) -> AppResult<CloudFileResult> {
    let plan = build_ffmpeg_mp4_plan(&spec, progress, cancel).await?;
    progress.emit(
        "cloud_precreating",
        40.0,
        0,
        plan.size_bytes,
        "正在创建 MP4 上传任务".to_string(),
    );
    let mut session = uploader.precreate(plan.clone()).await?;
    session_store.upsert(session.clone())?;

    progress.emit(
        "cloud_uploading_video",
        42.0,
        0,
        plan.size_bytes,
        "正在上传 MP4 到百度网盘".to_string(),
    );
    let mut process = spawn_ffmpeg_mp4(&spec)?;
    let mut read_buffer = vec![0_u8; 256 * 1024];
    let mut part_buffer = Vec::with_capacity(spec.part_size as usize);
    let mut part_index = 0_usize;
    let mut bytes_done = 0_u64;

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
    uploader: &impl CloudUploader,
    session_store: &CloudUploadSessionStore,
    progress: &CloudProgress,
    session: &mut crate::models::cloud::CloudUploadSession,
    part_index: usize,
    bytes: Vec<u8>,
    bytes_done: &mut u64,
    bytes_total: u64,
) -> AppResult<()> {
    *bytes_done = bytes_done.saturating_add(bytes.len() as u64);
    let _ = uploader.upload_part(session, part_index, bytes).await?;
    if !session.uploaded_parts.contains(&part_index) {
        session.uploaded_parts.push(part_index);
    }
    session_store.upsert(session.clone())?;
    let percent = 42.0 + (*bytes_done as f32 / bytes_total.max(1) as f32 * 53.0);
    progress.emit(
        "cloud_uploading_video",
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
        "cloud_muxing_mp4",
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
            "cloud_muxing_mp4",
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
    uploader: &impl CloudUploader,
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
    let md5 = format!("{:x}", md5::compute(&bytes));
    let plan = CloudFilePlan {
        provider: CloudProvider::BaiduNetdisk,
        mode: CloudUploadMode::RawDash,
        remote_path,
        size_bytes: bytes.len() as u64,
        part_size: bytes.len().max(1) as u64,
        block_md5: vec![md5],
        content_type,
    };
    let mut session = uploader.precreate(plan).await?;
    session_store.upsert(session.clone())?;
    let _ = uploader.upload_part(&session, 0, bytes).await?;
    session.uploaded_parts.push(0);
    session_store.upsert(session.clone())?;
    let result = uploader.finish(session.clone()).await?;
    session_store.remove(&session.upload_id)?;
    Ok(result)
}

fn spawn_ffmpeg_mp4(spec: &Mp4PipeSpec) -> AppResult<FfmpegMp4Process> {
    let headers = ffmpeg_input_headers(&spec.referer, spec.cookie_header.as_deref());
    let mut child = Command::new(&spec.ffmpeg_path)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-fflags")
        .arg("+bitexact")
        .arg("-headers")
        .arg(&headers)
        .arg("-i")
        .arg(&spec.video_url)
        .arg("-headers")
        .arg(&headers)
        .arg("-i")
        .arg(&spec.audio_url)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("1:a:0")
        .arg("-map_metadata")
        .arg("-1")
        .arg("-map_chapters")
        .arg("-1")
        .arg("-c")
        .arg("copy")
        .arg("-shortest")
        .arg("-movflags")
        .arg("frag_keyframe+empty_moov+default_base_moof")
        .arg("-f")
        .arg("mp4")
        .arg("pipe:1")
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
        return Err(AppError::Download {
            task_id: Some(task_id.to_string()),
            message: "任务已取消".to_string(),
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
        "Referer: {referer}\r\nOrigin: https://www.bilibili.com\r\nUser-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36\r\n"
    );
    if let Some(cookie_header) = cookie_header.filter(|value| !value.trim().is_empty()) {
        headers.push_str("Cookie: ");
        headers.push_str(cookie_header);
        headers.push_str("\r\n");
    }
    headers
}

fn md5_hex(bytes: &[u8]) -> String {
    format!("{:x}", md5::compute(bytes))
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
    stage: &'static str,
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

fn selected_page<'a>(
    video: &'a VideoData,
    input: &StartDownloadRequest,
) -> AppResult<&'a crate::models::video::VideoPage> {
    input
        .cid
        .and_then(|cid| video.pages.iter().find(|page| page.cid == cid))
        .or_else(|| {
            input
                .page
                .and_then(|page_no| video.pages.iter().find(|page| page.page == page_no))
        })
        .or_else(|| video.pages.first())
        .ok_or_else(|| AppError::InvalidInput {
            message: "视频没有可下载分P".to_string(),
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

fn ensure_not_cancelled(cancel: &Arc<AtomicBool>, task_id: &str) -> AppResult<()> {
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::Download {
            task_id: Some(task_id.to_string()),
            message: "任务已取消".to_string(),
        });
    }
    Ok(())
}

fn quality_to_qn(quality: &str) -> u32 {
    if quality.contains("8K") {
        127
    } else if quality.contains("杜比") || quality.to_ascii_lowercase().contains("dolby") {
        126
    } else if quality.contains("HDR") {
        125
    } else if quality.contains("4K") {
        120
    } else if quality.contains("1080P60") {
        116
    } else if quality.contains("1080P+") {
        112
    } else if quality.contains("720P60") {
        74
    } else if quality.contains("1080P") {
        80
    } else if quality.contains("720P") {
        64
    } else if quality.contains("480P") {
        32
    } else {
        16
    }
}

fn select_video_track(tracks: &[DashTrack], qn: u32) -> Option<DashTrack> {
    let wants_dolby = qn == 126;
    let wants_hdr = qn == 125;
    tracks
        .iter()
        .filter(|track| track.id <= qn)
        .max_by_key(|track| {
            (
                track.id,
                u8::from(wants_dolby && is_dolby_track(track)),
                u8::from(wants_hdr && is_hdr_track(track)),
                track.bandwidth.unwrap_or_default(),
            )
        })
        .or_else(|| {
            tracks
                .iter()
                .max_by_key(|track| track.bandwidth.unwrap_or_default())
        })
        .cloned()
}

fn is_dolby_track(track: &DashTrack) -> bool {
    let codecs = track.codecs.to_ascii_lowercase();
    codecs.contains("dvh")
        || codecs.contains("dvhe")
        || codecs.contains("dolby")
        || track
            .mime_type
            .as_deref()
            .is_some_and(|mime| mime.to_ascii_lowercase().contains("dolby"))
}

fn is_hdr_track(track: &DashTrack) -> bool {
    let codecs = track.codecs.to_ascii_lowercase();
    codecs.contains("hev1") || codecs.contains("hvc1") || track.id == 125
}

fn select_audio_track(tracks: &[DashTrack], quality: &str) -> Option<DashTrack> {
    let target_id = if quality.contains("128kbps") {
        30216
    } else if quality.contains("192kbps") {
        30232
    } else {
        30280
    };
    tracks
        .iter()
        .find(|track| track.id == target_id)
        .or_else(|| {
            tracks
                .iter()
                .max_by_key(|track| track.bandwidth.unwrap_or_default())
        })
        .cloned()
}

fn first_url(primary: &str, backups: &[String]) -> AppResult<String> {
    if !primary.trim().is_empty() {
        return Ok(primary.to_string());
    }
    backups
        .iter()
        .find(|url| !url.trim().is_empty())
        .cloned()
        .ok_or_else(|| AppError::Download {
            task_id: None,
            message: "播放地址为空".to_string(),
        })
}

struct CloudProgress {
    app: AppHandle,
    task_id: String,
}

impl CloudProgress {
    fn new(app: AppHandle, task_id: String) -> Self {
        Self { app, task_id }
    }

    fn emit(&self, stage: &str, percent: f32, bytes_done: u64, bytes_total: u64, message: String) {
        let _ = self.app.emit(
            "download://progress",
            DownloadProgressEvent {
                task_id: self.task_id.clone(),
                stage: stage.to_string(),
                percent: percent.clamp(0.0, 100.0),
                bytes_done,
                bytes_total,
                speed_bytes_per_sec: 0,
                message: Some(message),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_remote_dir_adds_leading_slash_and_removes_trailing_slash() {
        assert_eq!(normalize_remote_dir("apps/demo/"), "/apps/demo");
        assert_eq!(normalize_remote_dir("/apps/demo/"), "/apps/demo");
        assert_eq!(normalize_remote_dir(""), "/apps/B站充电视频下载器");
    }

    #[test]
    fn first_url_prefers_primary_and_falls_back_to_backup() -> AppResult<()> {
        assert_eq!(first_url("https://a", &[])?, "https://a");
        assert_eq!(
            first_url("", &["".to_string(), "https://b".to_string()])?,
            "https://b"
        );
        Ok(())
    }

    fn dash_video_track(id: u32, codecs: &str, bandwidth: u64) -> DashTrack {
        DashTrack {
            id,
            codecs: codecs.to_string(),
            width: Some(if id == 127 { 7680 } else { 3840 }),
            height: Some(if id == 127 { 4320 } else { 2160 }),
            frame_rate: Some("60".to_string()),
            bandwidth: Some(bandwidth),
            size_bytes: Some(bandwidth),
            mime_type: Some("video/mp4".to_string()),
            base_url: format!("https://example.test/video-{id}-{bandwidth}.m4s"),
            backup_urls: vec![],
        }
    }

    #[test]
    fn select_video_track_prefers_requested_dolby_variant() {
        let tracks = vec![
            dash_video_track(126, "hev1.2.4.L153", 8_000_000),
            dash_video_track(126, "dvh1.08.07", 7_000_000),
            dash_video_track(120, "avc1.640034", 10_000_000),
        ];

        let selected = select_video_track(&tracks, 126).expect("track should be selected");

        assert_eq!(selected.id, 126);
        assert!(selected.codecs.contains("dvh"));
    }
}
