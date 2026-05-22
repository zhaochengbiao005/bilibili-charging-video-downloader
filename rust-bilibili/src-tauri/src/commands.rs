use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use reqwest::header::{CONTENT_TYPE, REFERER};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::{
    auth::LoginStatus,
    downloader::{emit_progress, FileDownloadSpec, ProgressSender},
    error::{AppError, AppResult},
    models::{
        config::{ConfigResponse, SaveConfigRequest},
        download::{
            DanmakuMode, DashTrack, DownloadDoneEvent, DownloadProgressEvent, DurlSegment,
            PlayUrlRequest, PlayUrlResponse, StartDownloadRequest, StartDownloadResponse,
        },
        history::HistoryItem,
        video::{FetchInfoRequest, FetchInfoResponse},
    },
    state::AppState,
};

#[tauri::command]
pub async fn fetch_info(
    input: FetchInfoRequest,
    state: State<'_, AppState>,
) -> AppResult<FetchInfoResponse> {
    let bvid = input.bvid.trim();
    if !is_valid_bvid(bvid) {
        return Err(AppError::InvalidInput {
            message: "请输入有效的 BVID".to_string(),
        });
    }

    let cookies = load_cookies_for_request(input.cookie_path.as_deref(), &state)?;
    let video = state.client.video_info(bvid, cookies.as_ref()).await?;
    Ok(FetchInfoResponse { video })
}

#[tauri::command]
pub async fn fetch_image_data_url(url: String) -> AppResult<String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::InvalidInput {
            message: "图片地址无效".to_string(),
        });
    }

    let response = reqwest::Client::new()
        .get(url)
        .header(REFERER, "https://www.bilibili.com/")
        .send()
        .await?
        .error_for_status()?;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("image/jpeg")
        .split(';')
        .next()
        .unwrap_or("image/jpeg")
        .to_string();
    if !content_type.starts_with("image/") {
        return Err(AppError::InvalidInput {
            message: "远程地址不是图片".to_string(),
        });
    }

    let bytes = response.bytes().await?;
    Ok(format!(
        "data:{content_type};base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

#[tauri::command]
pub async fn start_download(
    input: StartDownloadRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<StartDownloadResponse> {
    if !is_valid_bvid(&input.bvid) {
        return Err(AppError::InvalidInput {
            message: "请输入有效的 BVID".to_string(),
        });
    }
    if input.outdir.trim().is_empty() {
        return Err(AppError::InvalidInput {
            message: "输出目录不能为空".to_string(),
        });
    }
    if input.threads == 0 {
        return Err(AppError::InvalidInput {
            message: "下载线程数必须大于 0".to_string(),
        });
    }

    let task_id = format!("dl_{}", Uuid::new_v4());
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .cancel_tokens
        .write()
        .await
        .insert(task_id.clone(), cancel.clone());
    let client = state.client.clone();
    let danmaku = state.danmaku.clone();
    let downloader = state.downloader.clone();
    let ffmpeg = state.ffmpeg.clone();
    let app_dir = state.config_store.app_dir().to_path_buf();
    let resource_dir = app.path().resource_dir().ok();
    let history_store = state.history_store.clone();
    let config_store = state.config_store.clone();
    let cancel_tokens = state.cancel_tokens.clone();
    let cookies = load_cookies_for_request(input.cookie_path.as_deref(), &state)?;
    let event_task_id = task_id.clone();
    let app_for_task = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = run_download_task(
            event_task_id.clone(),
            input,
            app_for_task.clone(),
            client,
            danmaku,
            downloader,
            ffmpeg,
            app_dir,
            resource_dir,
            history_store,
            config_store,
            cookies,
            cancel,
        )
        .await;

        cancel_tokens.write().await.remove(&event_task_id);

        match result {
            Ok(output_path) => {
                let _ = app_for_task.emit(
                    "download://completed",
                    DownloadDoneEvent {
                        task_id: event_task_id,
                        status: "completed".to_string(),
                        message: Some(format!("下载完成：{}", output_path.to_string_lossy())),
                    },
                );
            }
            Err(err) => {
                let message = err.to_string();
                let status = if message.contains("任务已取消") {
                    "cancelled"
                } else {
                    "failed"
                };
                let _ = app_for_task.emit(
                    "download://failed",
                    DownloadDoneEvent {
                        task_id: event_task_id,
                        status: status.to_string(),
                        message: Some(message),
                    },
                );
            }
        }
    });

    Ok(StartDownloadResponse { task_id })
}

#[tauri::command]
pub async fn cancel_download(task_id: String, state: State<'_, AppState>) -> AppResult<()> {
    if let Some(cancel) = state.cancel_tokens.read().await.get(&task_id) {
        cancel.store(true, Ordering::SeqCst);
        Ok(())
    } else {
        Err(AppError::InvalidInput {
            message: "下载任务不存在或已结束".to_string(),
        })
    }
}

#[tauri::command]
pub async fn fetch_playurl(
    input: PlayUrlRequest,
    state: State<'_, AppState>,
) -> AppResult<PlayUrlResponse> {
    let bvid = input.bvid.trim();
    if !is_valid_bvid(bvid) {
        return Err(AppError::InvalidInput {
            message: "请输入有效的 BVID".to_string(),
        });
    }
    let cookies = load_cookies_for_request(input.cookie_path.as_deref(), &state)?;
    state
        .client
        .playurl(bvid, input.cid, input.qn, cookies.as_ref())
        .await
}

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> AppResult<Vec<HistoryItem>> {
    state.history_store.load()
}

#[tauri::command]
pub fn delete_history_item(id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.history_store.delete(&id)
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> AppResult<()> {
    state.history_store.clear()
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppResult<ConfigResponse> {
    config_response(&state)
}

#[tauri::command]
pub fn save_config(
    input: SaveConfigRequest,
    state: State<'_, AppState>,
) -> AppResult<ConfigResponse> {
    let mut config = input.config;
    if config.default_outdir.trim().is_empty() {
        config.default_outdir = state.config_store.default_outdir();
    }
    state.config_store.save(&config)?;
    config_response(&state)
}

#[tauri::command]
pub fn get_quality_options() -> Vec<String> {
    ["360P", "480P", "720P", "1080P", "1080P60", "4K", "HDR"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[tauri::command]
pub fn get_default_outdir(state: State<'_, AppState>) -> AppResult<String> {
    Ok(state.config_store.load()?.default_outdir)
}

#[tauri::command]
pub fn get_app_dir(state: State<'_, AppState>) -> String {
    state.config_store.app_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn check_ffmpeg(app: AppHandle, state: State<'_, AppState>) -> crate::ffmpeg::FfmpegStatus {
    let resource_dir = app.path().resource_dir().ok();
    state
        .ffmpeg
        .check(state.config_store.app_dir(), resource_dir.as_deref())
}

#[tauri::command]
pub async fn install_ffmpeg(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<StartDownloadResponse> {
    let task_id = format!("ffmpeg_{}", Uuid::new_v4());
    let event_task_id = task_id.clone();
    let ffmpeg = state.ffmpeg.clone();
    let app_dir = state.config_store.app_dir().to_path_buf();

    tauri::async_runtime::spawn(async move {
        let progress_task_id = event_task_id.clone();
        let progress_app = app.clone();
        let result = ffmpeg
            .install_release(&app_dir, move |downloaded, total| {
                let percent = if total > 0 {
                    (downloaded as f32 / total as f32 * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                let _ = progress_app.emit(
                    "download://progress",
                    DownloadProgressEvent {
                        task_id: progress_task_id.clone(),
                        stage: "downloading_segments".to_string(),
                        percent,
                        bytes_done: downloaded,
                        bytes_total: total,
                        speed_bytes_per_sec: 0,
                        message: Some("正在安装 FFmpeg".to_string()),
                    },
                );
            })
            .await;

        match result {
            Ok(path) => {
                let _ = app.emit(
                    "download://completed",
                    DownloadDoneEvent {
                        task_id: event_task_id,
                        status: "completed".to_string(),
                        message: Some(format!("FFmpeg 已安装：{}", path.to_string_lossy())),
                    },
                );
            }
            Err(err) => {
                let _ = app.emit(
                    "download://failed",
                    DownloadDoneEvent {
                        task_id: event_task_id,
                        status: "failed".to_string(),
                        message: Some(err.to_string()),
                    },
                );
            }
        }
    });

    Ok(StartDownloadResponse { task_id })
}

#[tauri::command]
pub async fn choose_output_dir(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<String>> {
    let start_dir = state.config_store.load()?.default_outdir;
    let mut dialog = app.dialog().file().set_title("选择默认输出目录");
    if Path::new(&start_dir).exists() {
        dialog = dialog.set_directory(start_dir);
    }

    tauri::async_runtime::spawn_blocking(move || {
        dialog
            .blocking_pick_folder()
            .map(|path| {
                path.into_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .map_err(|err| AppError::Io {
                        message: err.to_string(),
                    })
            })
            .transpose()
    })
    .await
    .map_err(|err| AppError::Io {
        message: err.to_string(),
    })?
}

#[tauri::command]
pub fn open_path(path: String, app: AppHandle) -> AppResult<()> {
    if path.trim().is_empty() {
        return Err(AppError::InvalidInput {
            message: "路径不能为空".to_string(),
        });
    }
    let path = PathBuf::from(path.trim());
    if !path.exists() {
        return Err(AppError::Io {
            message: format!("文件已不存在：{}", path.to_string_lossy()),
        });
    }
    let path_to_open = if path.is_file() {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.clone())
    } else {
        path
    };

    app.opener()
        .open_path(path_to_open.to_string_lossy().into_owned(), None::<String>)
        .map_err(|err| AppError::Io {
            message: err.to_string(),
        })?;
    Ok(())
}

#[tauri::command]
pub async fn choose_cookie_file(app: AppHandle) -> AppResult<Option<String>> {
    let dialog = app
        .dialog()
        .file()
        .set_title("选择 Cookie 文件")
        .add_filter("Cookie 文件", &["json", "txt", "cookies"]);

    tauri::async_runtime::spawn_blocking(move || {
        dialog
            .blocking_pick_file()
            .map(|path| {
                path.into_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .map_err(|err| AppError::Io {
                        message: err.to_string(),
                    })
            })
            .transpose()
    })
    .await
    .map_err(|err| AppError::Io {
        message: err.to_string(),
    })?
}

#[tauri::command]
pub async fn check_login(state: State<'_, AppState>) -> AppResult<LoginStatus> {
    let Some(cookies) = state.cookie_store.load()? else {
        return Ok(LoginStatus::guest("尚未导入 Cookie"));
    };
    let status = state.client.check_login(&cookies).await?;
    Ok(status.with_cookie_path(Some(state.cookie_store.path())))
}

#[tauri::command]
pub async fn check_cookie(
    cookie_path: String,
    state: State<'_, AppState>,
) -> AppResult<LoginStatus> {
    let path = PathBuf::from(cookie_path.trim());
    let cookies = state.cookie_store.import_from_path(&path)?;
    let status = state.client.check_login(&cookies).await?;
    Ok(status.with_cookie_path(Some(state.cookie_store.path())))
}

#[tauri::command]
pub async fn clear_cookie(state: State<'_, AppState>) -> AppResult<LoginStatus> {
    state.cookie_store.clear()?;
    Ok(LoginStatus::guest("已退出登录"))
}

#[tauri::command]
pub async fn start_qr_login(
    state: State<'_, AppState>,
) -> AppResult<crate::auth::QrLoginStartResponse> {
    state.client.start_qr_login().await
}

#[tauri::command]
pub async fn poll_qr_login(
    qrcode_key: String,
    state: State<'_, AppState>,
) -> AppResult<crate::auth::QrLoginPollResponse> {
    let outcome = state.client.poll_qr_login(qrcode_key.trim()).await?;
    if let Some(cookies) = outcome.cookies {
        state.cookie_store.save(&cookies)?;
    }
    Ok(outcome.response)
}

fn is_valid_bvid(input: &str) -> bool {
    input.starts_with("BV") && input.len() >= 12 && input.chars().all(|c| c.is_ascii_alphanumeric())
}

fn config_response(state: &AppState) -> AppResult<ConfigResponse> {
    let config = state.config_store.load()?;
    Ok(ConfigResponse {
        default_outdir: state.config_store.default_outdir(),
        app_dir: state.config_store.app_dir().to_string_lossy().into_owned(),
        config,
    })
}

fn load_cookies_for_request(
    cookie_path: Option<&str>,
    state: &AppState,
) -> AppResult<Option<crate::auth::CookieSet>> {
    if let Some(path) = cookie_path.map(str::trim).filter(|path| !path.is_empty()) {
        return state
            .cookie_store
            .import_from_path(Path::new(path))
            .map(Some);
    }
    state.cookie_store.load()
}

async fn run_download_task(
    task_id: String,
    input: StartDownloadRequest,
    app: AppHandle,
    client: crate::api::BilibiliClient,
    danmaku: crate::danmaku::DanmakuClient,
    downloader: crate::downloader::DownloadClient,
    ffmpeg: crate::ffmpeg::FfmpegManager,
    app_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    history_store: crate::storage::HistoryStore,
    config_store: crate::storage::ConfigStore,
    cookies: Option<crate::auth::CookieSet>,
    cancel: Arc<AtomicBool>,
) -> AppResult<PathBuf> {
    let progress = progress_sender(app.clone());
    let started = Instant::now();
    progress(DownloadProgressEvent {
        task_id: task_id.clone(),
        stage: "resolving".to_string(),
        percent: 0.0,
        bytes_done: 0,
        bytes_total: 0,
        speed_bytes_per_sec: 0,
        message: Some(format!("正在解析播放地址{}", danmaku_mode_suffix(&input))),
    });

    let video = client.video_info(&input.bvid, cookies.as_ref()).await?;
    let page = video.pages.first().ok_or_else(|| AppError::InvalidInput {
        message: "视频没有可下载分P".to_string(),
    })?;
    let qn = quality_to_qn(&input.quality);
    let max_workers = input.threads.clamp(1, 32);
    let playurl = client
        .playurl(&input.bvid, page.cid, qn, cookies.as_ref())
        .await?;
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
        let result = downloader
            .download_file(
                FileDownloadSpec {
                    task_id: task_id.clone(),
                    stage: "downloading_audio".to_string(),
                    url: first_url(&track.base_url, &track.backup_urls)?,
                    output_path: raw_audio_path.clone(),
                    referer,
                    max_workers,
                    cookie_header: cookie_header.clone(),
                },
                cancel,
                progress.clone(),
            )
            .await?;
        progress(DownloadProgressEvent {
            task_id: task_id.clone(),
            stage: "converting_audio".to_string(),
            percent: 98.0,
            bytes_done: result.bytes_written,
            bytes_total: result.bytes_written,
            speed_bytes_per_sec: 0,
            message: Some("正在转换 MP3".to_string()),
        });
        let mp3_path = output_dir.join(format!("{safe_title}.mp3"));
        ffmpeg
            .convert_audio_to_mp3(
                &app_dir,
                resource_dir.as_deref(),
                &raw_audio_path,
                &mp3_path,
            )
            .await?;
        let _ = tokio::fs::remove_file(&raw_audio_path).await;
        write_history_record(
            &history_store,
            &config_store,
            &input,
            &video.title,
            &mp3_path,
            "completed",
        )?;
        return Ok(mp3_path);
    }

    if let Some(dash) = playurl.dash {
        let video_track =
            select_video_track(&dash.video, qn).ok_or_else(|| AppError::Download {
                task_id: Some(task_id.clone()),
                message: "没有可下载视频流".to_string(),
            })?;
        let video_path = output_dir.join(format!("{safe_title}-video-{}.m4s", video_track.id));
        let video_result = downloader
            .download_file(
                FileDownloadSpec {
                    task_id: task_id.clone(),
                    stage: "downloading_video".to_string(),
                    url: first_url(&video_track.base_url, &video_track.backup_urls)?,
                    output_path: video_path,
                    referer: referer.clone(),
                    max_workers,
                    cookie_header: cookie_header.clone(),
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
        let _ = downloader
            .download_file(
                FileDownloadSpec {
                    task_id: task_id.clone(),
                    stage: "downloading_audio".to_string(),
                    url: first_url(&audio_track.base_url, &audio_track.backup_urls)?,
                    output_path: audio_path.clone(),
                    referer,
                    max_workers,
                    cookie_header: cookie_header.clone(),
                },
                cancel,
                progress.clone(),
            )
            .await?;

        if !input.skip_merge {
            progress(DownloadProgressEvent {
                task_id: task_id.clone(),
                stage: "merging".to_string(),
                percent: 98.0,
                bytes_done: video_result.bytes_written,
                bytes_total: video_result.bytes_written,
                speed_bytes_per_sec: 0,
                message: Some("正在合并音视频".to_string()),
            });
            let merged_path = output_dir.join(format!("{safe_title}.mp4"));
            ffmpeg
                .merge_video_audio(
                    &app_dir,
                    resource_dir.as_deref(),
                    &video_result.output_path,
                    &audio_path,
                    &merged_path,
                )
                .await?;
            let danmaku_result = process_danmaku_if_requested(
                &danmaku,
                &ffmpeg,
                &input,
                page.cid,
                &merged_path,
                &app_dir,
                resource_dir.as_deref(),
                cookies.as_ref(),
                &progress,
                &task_id,
            )
            .await;
            let completion_message = completion_message("MP4 下载完成", danmaku_result);
            let _ = tokio::fs::remove_file(&video_result.output_path).await;
            let _ = tokio::fs::remove_file(&audio_path).await;
            write_history_record(
                &history_store,
                &config_store,
                &input,
                &video.title,
                &merged_path,
                "completed",
            )?;
            progress(DownloadProgressEvent {
                task_id,
                stage: "completed".to_string(),
                percent: 100.0,
                bytes_done: video_result.bytes_written,
                bytes_total: video_result.bytes_written,
                speed_bytes_per_sec: 0,
                message: Some(completion_message),
            });
            return Ok(merged_path);
        }

        let danmaku_result = process_danmaku_if_requested(
            &danmaku,
            &ffmpeg,
            &input,
            page.cid,
            &video_result.output_path,
            &app_dir,
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
            stage: "completed".to_string(),
            percent: 100.0,
            bytes_done: video_result.bytes_written,
            bytes_total: video_result.bytes_written,
            speed_bytes_per_sec: 0,
            message: Some(completion_message),
        });
        write_history_record(
            &history_store,
            &config_store,
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
            &downloader,
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
            &danmaku,
            &ffmpeg,
            &input,
            page.cid,
            &output_path,
            &app_dir,
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
            "completed",
            1,
            1,
            started,
            Some(completion_message),
        );
        write_history_record(
            &history_store,
            &config_store,
            &input,
            &video.title,
            &output_path,
            "completed",
        )?;
        return Ok(output_path);
    }

    Err(AppError::Download {
        task_id: Some(task_id),
        message: "没有找到可下载的视频流".to_string(),
    })
}

fn write_history_record(
    history_store: &crate::storage::HistoryStore,
    config_store: &crate::storage::ConfigStore,
    input: &StartDownloadRequest,
    title: &str,
    output_path: &Path,
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
            format: input.format.clone(),
            output_path: output_path.to_string_lossy().into_owned(),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            status: status.to_string(),
        },
    );
    items.truncate(max_history);
    history_store.save(&items)
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
    danmaku: &crate::danmaku::DanmakuClient,
    ffmpeg: &crate::ffmpeg::FfmpegManager,
    input: &StartDownloadRequest,
    cid: u64,
    output_path: &Path,
    app_dir: &Path,
    resource_dir: Option<&Path>,
    cookies: Option<&crate::auth::CookieSet>,
    progress: &ProgressSender,
    task_id: &str,
) -> Option<Result<DanmakuProcessResult, String>> {
    let mode = input.effective_danmaku_mode();
    if mode == DanmakuMode::None {
        return None;
    }

    progress(DownloadProgressEvent {
        task_id: task_id.to_string(),
        stage: "downloading_danmaku".to_string(),
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
                stage: "burning_danmaku".to_string(),
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
            stage: "burning_danmaku".to_string(),
            percent: 99.9,
            bytes_done: 1,
            bytes_total: 1,
            speed_bytes_per_sec: 0,
            message: Some("弹幕已烧录到视频".to_string()),
        }),
        Ok(result) => progress(DownloadProgressEvent {
            task_id: task_id.to_string(),
            stage: "downloading_danmaku".to_string(),
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
            stage: "downloading_danmaku".to_string(),
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
    downloader: &crate::downloader::DownloadClient,
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
                    stage: "downloading_segments".to_string(),
                    url: first_url(&segment.url, &segment.backup_urls)?,
                    output_path: part_path.clone(),
                    referer: referer.to_string(),
                    max_workers,
                    cookie_header: cookie_header.clone(),
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
            "downloading_segments",
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

fn progress_sender(app: AppHandle) -> ProgressSender {
    Arc::new(move |event| {
        let _ = app.emit("download://progress", event);
    })
}

fn quality_to_qn(quality: &str) -> u32 {
    if quality.contains("HDR") {
        125
    } else if quality.contains("4K") {
        120
    } else if quality.contains("1080P60") {
        116
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
    tracks
        .iter()
        .filter(|track| track.id <= qn)
        .max_by_key(|track| (track.id, track.bandwidth.unwrap_or_default()))
        .or_else(|| {
            tracks
                .iter()
                .max_by_key(|track| track.bandwidth.unwrap_or_default())
        })
        .cloned()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dash_audio_track(id: u32, bandwidth: u64) -> DashTrack {
        DashTrack {
            id,
            codecs: "mp4a".to_string(),
            width: None,
            height: None,
            frame_rate: None,
            bandwidth: Some(bandwidth),
            size_bytes: Some(bandwidth),
            mime_type: Some("audio/mp4".to_string()),
            base_url: format!("https://example.test/{id}.m4s"),
            backup_urls: vec![],
        }
    }

    #[test]
    fn select_audio_track_prefers_matching_bilibili_audio_id() {
        let tracks = vec![
            dash_audio_track(30280, 320_000),
            dash_audio_track(30232, 192_000),
            dash_audio_track(30216, 128_000),
        ];

        assert_eq!(
            select_audio_track(&tracks, "320kbps 高品质").unwrap().id,
            30280
        );
        assert_eq!(
            select_audio_track(&tracks, "192kbps 标准").unwrap().id,
            30232
        );
        assert_eq!(
            select_audio_track(&tracks, "128kbps 基础").unwrap().id,
            30216
        );
    }
}
