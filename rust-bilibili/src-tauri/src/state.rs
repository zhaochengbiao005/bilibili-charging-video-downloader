use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
};

use tokio::sync::RwLock;

use crate::{
    api::BilibiliClient,
    cloud::baidu::BaiduTokenStore,
    danmaku::DanmakuClient,
    downloader::DownloadClient,
    error::AppResult,
    ffmpeg::FfmpegManager,
    storage::{app_data_dir, ConfigStore, CookieStore, HistoryStore},
};

#[derive(Debug)]
pub struct AppState {
    pub client: BilibiliClient,
    pub downloader: DownloadClient,
    pub cancel_tokens: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    pub config_store: ConfigStore,
    pub history_store: HistoryStore,
    pub cookie_store: CookieStore,
    pub baidu_token_store: BaiduTokenStore,
    pub ffmpeg: FfmpegManager,
    pub danmaku: DanmakuClient,
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        let app_dir = app_data_dir()?;
        let config_store = ConfigStore::new(app_dir.clone())?;
        let history_store = HistoryStore::new(app_dir.clone());
        let cookie_store = CookieStore::new(app_dir.clone());
        let baidu_token_store = BaiduTokenStore::new(app_dir);

        Ok(Self {
            client: BilibiliClient::new()?,
            downloader: DownloadClient::new()?,
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
            config_store,
            history_store,
            cookie_store,
            baidu_token_store,
            ffmpeg: FfmpegManager::new(),
            danmaku: DanmakuClient::new(),
        })
    }
}
