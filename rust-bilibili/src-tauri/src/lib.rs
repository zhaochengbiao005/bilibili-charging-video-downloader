mod api;
mod auth;
mod commands;
mod error;
mod ffmpeg;
mod models;
mod state;
mod storage;

pub use error::{AppError, AppResult};

pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::new().expect("failed to initialize app state"))
        .invoke_handler(tauri::generate_handler![
            commands::fetch_info,
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
