use std::{path::Path, process::Command, time::Duration};

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{
        config::AppConfig,
        download::{DownloadDoneEvent, DownloadProgressEvent, StartDownloadRequest, StartDownloadResponse},
        history::HistoryItem,
        video::{FetchInfoRequest, VideoData},
    },
    state::AppState,
};

#[tauri::command]
pub async fn fetch_info(
    input: FetchInfoRequest,
    state: State<'_, AppState>,
) -> AppResult<VideoData> {
    let bvid = input.bvid.trim();
    if !is_valid_bvid(bvid) {
        return Err(AppError::InvalidInput {
            message: "请输入有效的 BVID".to_string(),
        });
    }

    state.client.video_info(bvid).await
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
pub fn get_config(state: State<'_, AppState>) -> AppResult<AppConfig> {
    state.config_store.load()
}

#[tauri::command]
pub fn save_config(config: AppConfig, state: State<'_, AppState>) -> AppResult<()> {
    state.config_store.save(&config)
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
pub fn choose_output_dir() -> Option<String> {
    None
}

#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    if path.trim().is_empty() {
        return Err(AppError::InvalidInput {
            message: "路径不能为空".to_string(),
        });
    }

    open_with_system(&path)?;
    Ok(())
}

#[tauri::command]
pub fn check_cookie(cookie_path: String) -> AppResult<serde_json::Value> {
    if cookie_path.trim().is_empty() || !Path::new(&cookie_path).exists() {
        return Ok(serde_json::json!({ "is_login": false }));
    }

    Ok(serde_json::json!({
        "is_login": false,
        "path": cookie_path,
        "message": "Cookie 登录校验将在 Phase 2 接入"
    }))
}

#[tauri::command]
pub fn start_qr_login() -> AppResult<serde_json::Value> {
    Err(AppError::InvalidInput {
        message: "扫码登录将在 Phase 2 接入".to_string(),
    })
}

fn is_valid_bvid(input: &str) -> bool {
    input.starts_with("BV") && input.len() >= 12 && input.chars().all(|c| c.is_ascii_alphanumeric())
}

fn open_with_system(path: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn()?;
    }

    Ok(())
}
