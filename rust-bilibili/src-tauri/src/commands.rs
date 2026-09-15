use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use reqwest::header::{CONTENT_TYPE, REFERER};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::{
    auth::LoginStatus,
    cloud::{
        direct::upload_baidu_test_file,
        session::CloudUploadSessionStore,
    },
    error::{AppError, AppResult},
    models::{
        cloud::{
            BaiduAuthFinishRequest, BaiduAuthStartResponse, CloudAuthStatus, CloudConfig,
            CloudFileResult, SaveCloudConfigRequest,
        },
        config::{ConfigResponse, SaveConfigRequest},
        download::{
            DownloadDoneEvent, DownloadProgressEvent, DownloadStage, PlayUrlRequest,
            PlayUrlResponse, StartDownloadRequest, StartDownloadResponse, TaskStatus,
        },
        video::{EnrichVideoRequest, FetchInfoRequest, FetchInfoResponse, VideoData},
    },
    models::history::HistoryItem,
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
    let videos = state.client.video_info_list(bvid, cookies.as_ref()).await?;
    let video = videos.first().cloned().ok_or_else(|| AppError::Api {
        code: -1,
        message: "B站响应缺少视频信息".to_string(),
    })?;
    Ok(FetchInfoResponse { video, videos })
}

#[tauri::command]
pub async fn enrich_video_sizes(
    input: EnrichVideoRequest,
    state: State<'_, AppState>,
) -> AppResult<VideoData> {
    if !is_valid_bvid(&input.video.id) {
        return Err(AppError::InvalidInput {
            message: "视频信息缺少有效的 BVID".to_string(),
        });
    }

    let cookies = load_cookies_for_request(input.cookie_path.as_deref(), &state)?;
    state
        .client
        .enrich_video_sizes(input.video, input.cid, cookies.as_ref())
        .await
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
    let cancel = state.tasks.register_cancel(&task_id).await;
    let cookies = load_cookies_for_request(input.cookie_path.as_deref(), &state)?;
    let resource_dir = app.path().resource_dir().ok();
    let task_for_body = task_id.clone();
    let cancel_for_body = cancel.clone();
    let app_for_body = app.clone();

    state.tasks.spawn_queued(app, task_id.clone(), cancel, move |tasks| async move {
        match tasks
            .run_download_task(
                task_for_body,
                input,
                app_for_body,
                resource_dir,
                cookies,
                cancel_for_body,
            )
            .await
        {
            Ok(output_path) => Ok(format!("下载完成：{}", output_path.to_string_lossy())),
            Err(err) => Err(err),
        }
    });

    Ok(StartDownloadResponse { task_id })
}

#[tauri::command]
pub async fn start_cloud_upload(
    input: StartDownloadRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<StartDownloadResponse> {
    if !is_valid_bvid(&input.bvid) {
        return Err(AppError::InvalidInput {
            message: "请输入有效的 BVID".to_string(),
        });
    }
    if input.threads == 0 {
        return Err(AppError::InvalidInput {
            message: "下载线程数必须大于 0".to_string(),
        });
    }

    let status = state.baidu_token_store.auth_status().await?;
    if !status.is_authorized {
        return Err(AppError::AuthRequired);
    }

    // 互动视频暂不支持云盘直传（多分片）
    let cookies_probe = load_cookies_for_request(input.cookie_path.as_deref(), &state)?;
    if let Ok(video) = state
        .client
        .video_info(&input.bvid, cookies_probe.as_ref())
        .await
    {
        if video.is_stein == Some(true) {
            return Err(AppError::InvalidInput {
                message: "互动视频暂仅支持本地下载（整图分片 / 路径拼接）".to_string(),
            });
        }
    }

    let task_id = format!("cloud_{}", Uuid::new_v4());
    let cancel = state.tasks.register_cancel(&task_id).await;
    let cookies = load_cookies_for_request(input.cookie_path.as_deref(), &state)?;
    let resource_dir = app.path().resource_dir().ok();
    let task_for_body = task_id.clone();
    let cancel_for_body = cancel.clone();
    let app_for_body = app.clone();

    state.tasks.spawn_queued(app, task_id.clone(), cancel, move |tasks| async move {
        let result = tasks
            .run_cloud_upload_task(
                task_for_body,
                input,
                app_for_body,
                resource_dir,
                cookies,
                cancel_for_body,
                None,
            )
            .await;
        match result {
            Ok(files) => {
                let paths = files
                    .iter()
                    .map(|file| file.remote_path.as_str())
                    .collect::<Vec<_>>()
                    .join("；");
                Ok(format!("百度网盘保存完成：{paths}"))
            }
            Err(err) => Err(err),
        }
    });

    Ok(StartDownloadResponse { task_id })
}

#[tauri::command]
pub async fn cancel_download(task_id: String, state: State<'_, AppState>) -> AppResult<()> {
    if state.tasks.cancel(&task_id).await {
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
pub fn get_cloud_config(state: State<'_, AppState>) -> AppResult<CloudConfig> {
    state.baidu_token_store.load_config()
}

#[tauri::command]
pub fn save_cloud_config(
    input: SaveCloudConfigRequest,
    state: State<'_, AppState>,
) -> AppResult<CloudConfig> {
    state.baidu_token_store.save_config(input.config)
}

#[tauri::command]
pub async fn baidu_auth_status(state: State<'_, AppState>) -> AppResult<CloudAuthStatus> {
    state.baidu_token_store.auth_status().await
}

#[tauri::command]
pub async fn baidu_auth_start(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<BaiduAuthStartResponse> {
    let response = state.baidu_token_store.auth_start()?;
    app.opener()
        .open_url(response.auth_url.clone(), None::<String>)
        .map_err(|err| AppError::Cloud {
            provider: Some(crate::models::cloud::CloudProvider::BaiduNetdisk),
            message: format!("打开百度授权页面失败：{err}"),
        })?;
    Ok(response)
}

#[tauri::command]
pub async fn baidu_auth_finish(
    input: BaiduAuthFinishRequest,
    state: State<'_, AppState>,
) -> AppResult<CloudAuthStatus> {
    state.baidu_token_store.auth_finish(input).await
}

#[tauri::command]
pub fn baidu_logout(state: State<'_, AppState>) -> AppResult<CloudAuthStatus> {
    state.baidu_token_store.logout()
}

#[tauri::command]
pub async fn baidu_upload_test_file(state: State<'_, AppState>) -> AppResult<CloudFileResult> {
    upload_baidu_test_file(
        state.baidu_token_store.clone(),
        state.config_store.app_dir().to_path_buf(),
    )
    .await
}

#[tauri::command]
pub fn get_pending_cloud_upload_count(state: State<'_, AppState>) -> AppResult<usize> {
    let store = CloudUploadSessionStore::new(state.config_store.app_dir());
    Ok(store.load()?.len())
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
                        stage: DownloadStage::DownloadingSegments,
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
                        status: TaskStatus::Completed,
                        message: Some(format!("FFmpeg 已安装：{}", path.to_string_lossy())),
                    },
                );
            }
            Err(err) => {
                let _ = app.emit(
                    "download://failed",
                    DownloadDoneEvent {
                        task_id: event_task_id,
                        status: TaskStatus::Failed,
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
pub fn open_url(url: String, app: AppHandle) -> AppResult<()> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::InvalidInput {
            message: "链接地址无效".to_string(),
        });
    }

    app.opener()
        .open_url(url.to_string(), None::<String>)
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

