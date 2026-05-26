use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex},
};

use uuid::Uuid;

use crate::{
    cloud::uploader::{CloudUploadFuture, CloudUploader},
    error::{AppError, AppResult},
    models::cloud::{
        CloudAuthStatus, CloudFilePlan, CloudFileResult, CloudPartResult, CloudProvider,
        CloudUploadSession,
    },
};

#[derive(Debug, Clone)]
pub struct MockCloudUploader {
    provider: CloudProvider,
    state: Arc<Mutex<MockCloudState>>,
}

#[derive(Debug)]
struct MockCloudState {
    authorized: bool,
    account_name: Option<String>,
    sessions: HashMap<String, MockSessionState>,
    completed_files: Vec<CloudFileResult>,
}

#[derive(Debug)]
struct MockSessionState {
    remote_path: String,
    size_bytes: u64,
    uploaded_parts: BTreeSet<usize>,
    bytes_uploaded: u64,
}

impl Default for MockCloudUploader {
    fn default() -> Self {
        Self::authorized("测试云盘")
    }
}

impl MockCloudUploader {
    pub fn authorized(account_name: impl Into<String>) -> Self {
        Self {
            provider: CloudProvider::BaiduNetdisk,
            state: Arc::new(Mutex::new(MockCloudState {
                authorized: true,
                account_name: Some(account_name.into()),
                sessions: HashMap::new(),
                completed_files: Vec::new(),
            })),
        }
    }

    pub fn unauthorized() -> Self {
        Self {
            provider: CloudProvider::BaiduNetdisk,
            state: Arc::new(Mutex::new(MockCloudState {
                authorized: false,
                account_name: None,
                sessions: HashMap::new(),
                completed_files: Vec::new(),
            })),
        }
    }

    pub fn completed_files(&self) -> AppResult<Vec<CloudFileResult>> {
        let state = self.lock_state()?;
        Ok(state.completed_files.clone())
    }

    fn lock_state(&self) -> AppResult<std::sync::MutexGuard<'_, MockCloudState>> {
        self.state.lock().map_err(|err| AppError::Cloud {
            provider: Some(self.provider),
            message: format!("云盘 mock 状态锁定失败: {err}"),
        })
    }

    fn auth_status_inner(&self) -> AppResult<CloudAuthStatus> {
        let state = self.lock_state()?;
        if state.authorized {
            Ok(CloudAuthStatus {
                provider: self.provider,
                is_authorized: true,
                account_name: state.account_name.clone(),
                expires_at: None,
                message: Some("已授权".to_string()),
            })
        } else {
            Ok(CloudAuthStatus::guest(self.provider, "未授权"))
        }
    }

    fn precreate_inner(&self, plan: CloudFilePlan) -> AppResult<CloudUploadSession> {
        if plan.provider != self.provider {
            return Err(AppError::InvalidInput {
                message: "云盘提供方不匹配".to_string(),
            });
        }
        if plan.remote_path.trim().is_empty() {
            return Err(AppError::InvalidInput {
                message: "云盘保存路径不能为空".to_string(),
            });
        }
        if plan.expected_part_count() != plan.block_md5.len() {
            return Err(AppError::InvalidInput {
                message: "分片 MD5 数量与文件大小不匹配".to_string(),
            });
        }

        let mut state = self.lock_state()?;
        if !state.authorized {
            return Err(AppError::AuthRequired);
        }

        let upload_id = format!("mock_{}", Uuid::new_v4());
        state.sessions.insert(
            upload_id.clone(),
            MockSessionState {
                remote_path: plan.remote_path.clone(),
                size_bytes: plan.size_bytes,
                uploaded_parts: BTreeSet::new(),
                bytes_uploaded: 0,
            },
        );

        Ok(CloudUploadSession {
            provider: plan.provider,
            upload_id,
            remote_path: plan.remote_path,
            size_bytes: plan.size_bytes,
            part_size: plan.part_size,
            block_md5: plan.block_md5,
            uploaded_parts: Vec::new(),
        })
    }

    fn upload_part_inner(
        &self,
        session: &CloudUploadSession,
        part_index: usize,
        bytes: Vec<u8>,
    ) -> AppResult<CloudPartResult> {
        let mut state = self.lock_state()?;
        let session_state =
            state
                .sessions
                .get_mut(&session.upload_id)
                .ok_or_else(|| AppError::Cloud {
                    provider: Some(session.provider),
                    message: "云盘上传会话不存在".to_string(),
                })?;

        if part_index >= session.block_md5.len() {
            return Err(AppError::InvalidInput {
                message: "云盘上传分片序号越界".to_string(),
            });
        }
        if session_state.uploaded_parts.insert(part_index) {
            session_state.bytes_uploaded = session_state
                .bytes_uploaded
                .saturating_add(bytes.len() as u64);
        }

        Ok(CloudPartResult {
            part_index,
            bytes_uploaded: bytes.len() as u64,
            md5: session.block_md5.get(part_index).cloned(),
        })
    }

    fn finish_inner(&self, mut session: CloudUploadSession) -> AppResult<CloudFileResult> {
        let mut state = self.lock_state()?;
        let session_state =
            state
                .sessions
                .remove(&session.upload_id)
                .ok_or_else(|| AppError::Cloud {
                    provider: Some(session.provider),
                    message: "云盘上传会话不存在".to_string(),
                })?;

        let uploaded_count = session_state.uploaded_parts.len();
        if uploaded_count != session.block_md5.len() {
            return Err(AppError::InvalidInput {
                message: format!(
                    "云盘上传分片未完成: {uploaded_count}/{}",
                    session.block_md5.len()
                ),
            });
        }

        session.uploaded_parts = session_state.uploaded_parts.into_iter().collect();
        let result = CloudFileResult {
            provider: session.provider,
            remote_path: session_state.remote_path,
            file_id: Some(format!("mock_file_{}", session.upload_id)),
            size_bytes: session_state.size_bytes,
        };
        state.completed_files.push(result.clone());
        Ok(result)
    }
}

impl CloudUploader for MockCloudUploader {
    fn auth_status(&self) -> CloudUploadFuture<'_, CloudAuthStatus> {
        Box::pin(std::future::ready(self.auth_status_inner()))
    }

    fn precreate(&self, plan: CloudFilePlan) -> CloudUploadFuture<'_, CloudUploadSession> {
        Box::pin(std::future::ready(self.precreate_inner(plan)))
    }

    fn upload_part<'a>(
        &'a self,
        session: &'a CloudUploadSession,
        part_index: usize,
        bytes: Vec<u8>,
    ) -> CloudUploadFuture<'a, CloudPartResult> {
        Box::pin(std::future::ready(
            self.upload_part_inner(session, part_index, bytes),
        ))
    }

    fn finish(&self, session: CloudUploadSession) -> CloudUploadFuture<'_, CloudFileResult> {
        Box::pin(std::future::ready(self.finish_inner(session)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cloud::CloudUploadMode;

    #[tokio::test]
    async fn mock_uploader_completes_precreate_upload_finish_flow() -> AppResult<()> {
        let uploader = MockCloudUploader::authorized("测试账号");
        let status = uploader.auth_status().await?;
        assert!(status.is_authorized);
        assert_eq!(status.account_name.as_deref(), Some("测试账号"));

        let plan = CloudFilePlan {
            provider: CloudProvider::BaiduNetdisk,
            mode: CloudUploadMode::RawDash,
            remote_path: "/apps/B站充电视频下载器/demo-video.m4s".to_string(),
            size_bytes: 6,
            part_size: 3,
            block_md5: vec!["part-a".to_string(), "part-b".to_string()],
            content_type: Some("video/mp4".to_string()),
        };
        let session = uploader.precreate(plan).await?;

        let first = uploader.upload_part(&session, 0, b"abc".to_vec()).await?;
        let second = uploader.upload_part(&session, 1, b"def".to_vec()).await?;
        assert_eq!(first.bytes_uploaded, 3);
        assert_eq!(second.md5.as_deref(), Some("part-b"));

        let result = uploader.finish(session).await?;
        assert_eq!(result.remote_path, "/apps/B站充电视频下载器/demo-video.m4s");
        assert_eq!(result.size_bytes, 6);
        assert_eq!(uploader.completed_files()?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn mock_uploader_requires_authorization() {
        let uploader = MockCloudUploader::unauthorized();
        let plan = CloudFilePlan {
            provider: CloudProvider::BaiduNetdisk,
            mode: CloudUploadMode::RawDash,
            remote_path: "/apps/B站充电视频下载器/demo.m4s".to_string(),
            size_bytes: 3,
            part_size: 3,
            block_md5: vec!["part-a".to_string()],
            content_type: None,
        };

        let result = uploader.precreate(plan).await;
        assert!(matches!(result, Err(AppError::AuthRequired)));
    }
}
