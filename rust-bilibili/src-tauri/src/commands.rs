use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::{
    auth::LoginStatus,
    error::{AppError, AppResult},
    models::{
        config::{ConfigResponse, SaveConfigRequest},
        download::{
            DownloadDoneEvent, DownloadProgressEvent, StartDownloadRequest, StartDownloadResponse,
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
pub async fn start_download(
    input: StartDownloadRequest,
    app: AppHandle,
    _state: State<'_, AppState>,
) -> AppResult<StartDownloadResponse> {
    if !is_valid_bvid(&input.bvid) {
        return Err(AppError::InvalidInput {
            message: "请输入有效的 BVID".to_string(),
        });
    }

    let task_id = format!("dl_{}", Uuid::new_v4());
    let event_task_id = task_id.clone();
    let label = format!("{} {} {}", input.bvid, input.quality, input.format);

    tauri::async_runtime::spawn(async move {
        let _ = app.emit(
            "download://log",
            format!("下载任务已创建，下载核心将在 Phase 3 接入: {label}"),
        );
        let _ = app.emit(
            "download://progress",
            DownloadProgressEvent {
                task_id: event_task_id.clone(),
                stage: "queued".to_string(),
                percent: 0.0,
                bytes_done: 0,
                bytes_total: 0,
                speed_bytes_per_sec: 0,
                message: Some("任务已进入后端队列".to_string()),
            },
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
        let _ = app.emit(
            "download://failed",
            DownloadDoneEvent {
                task_id: event_task_id,
                status: "failed".to_string(),
                message: Some("下载核心尚未接入，当前阶段仅验证 Tauri 事件链路".to_string()),
            },
        );
    });

    Ok(StartDownloadResponse { task_id })
}

#[tauri::command]
pub async fn cancel_download(task_id: String, app: AppHandle) -> AppResult<()> {
    app.emit(
        "download://failed",
        DownloadDoneEvent {
            task_id,
            status: "cancelled".to_string(),
            message: Some("任务已取消".to_string()),
        },
    )
    .map_err(|err| AppError::Download {
        task_id: None,
        message: err.to_string(),
    })
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
pub fn check_ffmpeg(state: State<'_, AppState>) -> crate::ffmpeg::FfmpegStatus {
    state.ffmpeg.check()
}

#[tauri::command]
pub async fn install_ffmpeg(app: AppHandle) -> StartDownloadResponse {
    let task_id = format!("ffmpeg_{}", Uuid::new_v4());
    let event_task_id = task_id.clone();

    tauri::async_runtime::spawn(async move {
        let _ = app.emit(
            "download://log",
            "FFmpeg 自动安装将在 Phase 4 接入，当前先完成命令链路".to_string(),
        );
        let _ = app.emit(
            "download://failed",
            DownloadDoneEvent {
                task_id: event_task_id,
                status: "failed".to_string(),
                message: Some("FFmpeg 自动安装尚未接入".to_string()),
            },
        );
    });

    StartDownloadResponse { task_id }
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

    app.opener()
        .open_path(path, None::<String>)
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
