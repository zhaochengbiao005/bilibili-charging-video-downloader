use std::sync::Arc;

use crate::{
    api::BilibiliClient,
    cloud::baidu::BaiduTokenStore,
    danmaku::DanmakuClient,
    download::TaskOrchestrator,
    downloader::DownloadClient,
    error::AppResult,
    ffmpeg::FfmpegManager,
    storage::{app_data_dir, ConfigStore, CookieStore, HistoryStore},
};

#[derive(Debug)]
pub struct AppState {
    pub client: BilibiliClient,
    pub config_store: ConfigStore,
    pub history_store: HistoryStore,
    pub cookie_store: CookieStore,
    pub baidu_token_store: BaiduTokenStore,
    pub ffmpeg: FfmpegManager,
    /// 任务编排模块：队列、取消表、事件收尾全部在这里。
    pub tasks: Arc<TaskOrchestrator>,
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        let app_dir = app_data_dir()?;
        let config_store = ConfigStore::new(app_dir.clone())?;
        let history_store = HistoryStore::new(app_dir.clone());
        let cookie_store = CookieStore::new(app_dir.clone());
        let baidu_token_store = BaiduTokenStore::new(app_dir);

        let client = BilibiliClient::new()?;
        let downloader = DownloadClient::new()?;
        let ffmpeg = FfmpegManager::new();
        let danmaku = DanmakuClient::new();
        let tasks = Arc::new(TaskOrchestrator::new(
            client.clone(),
            downloader,
            ffmpeg.clone(),
            danmaku,
            config_store.clone(),
            history_store.clone(),
            baidu_token_store.clone(),
            config_store.app_dir().to_path_buf(),
        ));

        Ok(Self {
            client,
            config_store,
            history_store,
            cookie_store,
            baidu_token_store,
            ffmpeg,
            tasks,
        })
    }
}
