use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    auth::{parse_cookie_file, CookieSet},
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

#[derive(Debug, Clone)]
pub struct CookieStore {
    cookie_path: PathBuf,
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
        self.app_dir
            .join("downloads")
            .to_string_lossy()
            .into_owned()
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

impl CookieStore {
    pub fn new(app_dir: PathBuf) -> Self {
        Self {
            cookie_path: app_dir.join("cookies.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.cookie_path
    }

    pub fn load(&self) -> AppResult<Option<CookieSet>> {
        if !self.cookie_path.exists() {
            return Ok(None);
        }
        Ok(Some(parse_cookie_file(&self.cookie_path)?))
    }

    pub fn save(&self, cookies: &CookieSet) -> AppResult<()> {
        if !cookies.has_login_cookie() {
            return Err(AppError::InvalidInput {
                message: "Cookie 中缺少 SESSDATA，无法保存为默认登录 Cookie".to_string(),
            });
        }
        write_json_atomic(&self.cookie_path, cookies)
    }

    pub fn import_from_path(&self, path: &Path) -> AppResult<CookieSet> {
        if !path.exists() {
            return Err(AppError::InvalidInput {
                message: "Cookie 文件不存在".to_string(),
            });
        }
        let cookies = parse_cookie_file(path)?;
        self.save(&cookies)?;
        Ok(cookies)
    }

    pub fn clear(&self) -> AppResult<()> {
        if self.cookie_path.exists() {
            fs::remove_file(&self.cookie_path)?;
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_store_roundtrips_saved_config() -> AppResult<()> {
        let root = std::env::temp_dir().join(format!("bili_config_test_{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }

        let store = ConfigStore::new(root.clone())?;
        let mut config =
            AppConfig::with_default_outdir(root.join("videos").to_string_lossy().into_owned());
        config.auto_merge = false;
        config.max_history = 17;

        store.save(&config)?;
        let loaded = store.load()?;

        assert_eq!(loaded.default_outdir, config.default_outdir);
        assert_eq!(loaded.auto_merge, config.auto_merge);
        assert_eq!(loaded.max_history, config.max_history);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn cookie_store_imports_and_loads_default_cookie() -> AppResult<()> {
        let root = std::env::temp_dir().join(format!("bili_cookie_test_{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;

        let source_path = root.join("source.json");
        fs::write(&source_path, r#"{"SESSDATA":"sess","bili_jct":"csrf"}"#)?;
        let store = CookieStore::new(root.clone());

        store.import_from_path(&source_path)?;
        let loaded = store.load()?.expect("cookie should be saved");

        assert_eq!(
            loaded.cookies.get("SESSDATA").map(String::as_str),
            Some("sess")
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
