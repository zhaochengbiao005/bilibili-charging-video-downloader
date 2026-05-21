use crate::{
    api::BilibiliClient,
    error::AppResult,
    ffmpeg::FfmpegManager,
    storage::{app_data_dir, ConfigStore, HistoryStore},
};

#[derive(Debug)]
pub struct AppState {
    pub client: BilibiliClient,
    pub config_store: ConfigStore,
    pub history_store: HistoryStore,
    pub ffmpeg: FfmpegManager,
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        let app_dir = app_data_dir()?;
        let config_store = ConfigStore::new(app_dir.clone())?;
        let history_store = HistoryStore::new(app_dir);

        Ok(Self {
            client: BilibiliClient::new()?,
            config_store,
            history_store,
            ffmpeg: FfmpegManager::new(),
        })
    }
}
