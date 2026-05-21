use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{AppError, AppResult},
    models::{config::AppConfig, history::HistoryItem},
};

#[derive(Debug, Clone)]
pub struct ConfigStore {
    app_dir: PathBuf,
    config_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct HistoryStore {
    history_path: PathBuf,
}

impl ConfigStore {
    pub fn new(app_dir: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(&app_dir)?;
        let downloads_dir = app_dir.join("downloads");
        fs::create_dir_all(&downloads_dir)?;

        Ok(Self {
            config_path: app_dir.join("config.json"),
            app_dir,
        })
    }

    pub fn app_dir(&self) -> &Path {
        &self.app_dir
    }

    pub fn default_outdir(&self) -> String {
        self.app_dir.join("downloads").to_string_lossy().into_owned()
    }

    pub fn load(&self) -> AppResult<AppConfig> {
        if !self.config_path.exists() {
            return Ok(AppConfig::with_default_outdir(self.default_outdir()));
        }

        let content = fs::read_to_string(&self.config_path)?;
        let mut config: AppConfig = serde_json::from_str(&content)?;
        if config.default_outdir.trim().is_empty() {
            config.default_outdir = self.default_outdir();
        }
        Ok(config)
    }

    pub fn save(&self, config: &AppConfig) -> AppResult<()> {
        write_json_atomic(&self.config_path, config)
    }
}

impl HistoryStore {
    pub fn new(app_dir: PathBuf) -> Self {
        Self {
            history_path: app_dir.join("history.json"),
        }
    }

    pub fn load(&self) -> AppResult<Vec<HistoryItem>> {
        if !self.history_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.history_path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, items: &[HistoryItem]) -> AppResult<()> {
        write_json_atomic(&self.history_path, items)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let mut items = self.load()?;
        items.retain(|item| item.id != id);
        self.save(&items)
    }

    pub fn clear(&self) -> AppResult<()> {
        self.save(&[])
    }
}

pub fn app_data_dir() -> AppResult<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| AppError::Io {
        message: "无法定位系统数据目录".to_string(),
    })?;
    Ok(base.join("BilibiliDownloader"))
}

fn write_json_atomic<T: serde::Serialize + ?Sized>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = path.with_extension("tmp");
    let content = serde_json::to_vec_pretty(value)?;
    fs::write(&temp_path, content)?;

    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp_path, path)?;
    Ok(())
}
