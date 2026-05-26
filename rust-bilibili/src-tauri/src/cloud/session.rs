use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{error::AppResult, models::cloud::CloudUploadSession, storage::write_json_atomic};

#[derive(Debug, Clone)]
pub struct CloudUploadSessionStore {
    path: PathBuf,
}

impl CloudUploadSessionStore {
    pub fn new(app_dir: impl AsRef<Path>) -> Self {
        Self {
            path: app_dir.as_ref().join("cloud_upload_sessions.json"),
        }
    }

    pub fn load(&self) -> AppResult<Vec<CloudUploadSession>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save_all(&self, sessions: &[CloudUploadSession]) -> AppResult<()> {
        write_json_atomic(&self.path, sessions)
    }

    pub fn upsert(&self, session: CloudUploadSession) -> AppResult<()> {
        let mut sessions = self.load()?;
        if let Some(existing) = sessions
            .iter_mut()
            .find(|existing| existing.upload_id == session.upload_id)
        {
            *existing = session;
        } else {
            sessions.push(session);
        }
        self.save_all(&sessions)
    }

    pub fn remove(&self, upload_id: &str) -> AppResult<()> {
        let mut sessions = self.load()?;
        sessions.retain(|session| session.upload_id != upload_id);
        self.save_all(&sessions)
    }

    pub fn clear(&self) -> AppResult<()> {
        self.save_all(&[])
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::models::cloud::{CloudProvider, CloudUploadSession};

    use super::*;

    #[test]
    fn session_store_upserts_and_removes_sessions() -> AppResult<()> {
        let root = std::env::temp_dir().join(format!("bili_cloud_sessions_{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        let store = CloudUploadSessionStore::new(&root);

        let session = CloudUploadSession {
            provider: CloudProvider::BaiduNetdisk,
            upload_id: "upload-1".to_string(),
            remote_path: "/apps/demo/video.m4s".to_string(),
            size_bytes: 12,
            part_size: 4,
            block_md5: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            uploaded_parts: vec![0],
        };
        store.upsert(session.clone())?;

        let mut updated = session;
        updated.uploaded_parts = vec![0, 1];
        store.upsert(updated)?;
        assert_eq!(store.load()?.len(), 1);
        assert_eq!(store.load()?[0].uploaded_parts, vec![0, 1]);

        store.remove("upload-1")?;
        assert!(store.load()?.is_empty());

        fs::remove_dir_all(root)?;
        Ok(())
    }
}
