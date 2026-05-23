use std::{fs::OpenOptions, io::Write, panic};

mod api;
mod auth;
mod commands;
mod danmaku;
mod downloader;
mod error;
mod ffmpeg;
mod models;
mod state;
mod storage;

pub use error::{AppError, AppResult};

pub fn run() {
    let app_state = state::AppState::new().expect("failed to initialize app state");
    install_panic_logger(app_state.config_store.app_dir().to_path_buf());
    let log_dir = app_state.config_store.app_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: log_dir,
                        file_name: Some("BilibiliDownloader".to_string()),
                    },
                )])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::fetch_info,
            commands::enrich_video_sizes,
            commands::fetch_image_data_url,
            commands::fetch_playurl,
            commands::start_download,
            commands::cancel_download,
            commands::get_history,
            commands::delete_history_item,
            commands::clear_history,
            commands::get_config,
            commands::save_config,
            commands::get_quality_options,
            commands::get_default_outdir,
            commands::get_app_dir,
            commands::check_ffmpeg,
            commands::install_ffmpeg,
            commands::choose_output_dir,
            commands::open_path,
            commands::choose_cookie_file,
            commands::check_login,
            commands::check_cookie,
            commands::clear_cookie,
            commands::start_qr_login,
            commands::poll_qr_login,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn install_panic_logger(app_dir: std::path::PathBuf) {
    panic::set_hook(Box::new(move |panic_info| {
        let log_dir = app_dir.join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("panic.log"))
        {
            let _ = writeln!(
                file,
                "[{}] {panic_info}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            );
        }
    }));
}
