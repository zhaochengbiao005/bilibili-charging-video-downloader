use std::{
    fs,
    future::Future,
    path::{Path, PathBuf},
};

use chrono::{Duration, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration as TokioDuration};
use uuid::Uuid;

use crate::{
    cloud::uploader::{CloudUploadFuture, CloudUploader},
    error::{AppError, AppResult},
    models::cloud::{
        BaiduAuthFinishRequest, BaiduAuthStartResponse, CloudAuthStatus, CloudConfig,
        CloudFilePlan, CloudFileResult, CloudPartResult, CloudProvider, CloudUploadSession,
    },
    storage::write_json_atomic,
};

const AUTHORIZE_URL: &str = "https://openapi.baidu.com/oauth/2.0/authorize";
const TOKEN_URL: &str = "https://openapi.baidu.com/oauth/2.0/token";
const PAN_FILE_URL: &str = "https://pan.baidu.com/rest/2.0/xpan/file";
const PCS_UPLOAD_URL: &str = "https://d.pcs.baidu.com/rest/2.0/pcs/superfile2";

#[derive(Debug, Clone)]
pub struct BaiduTokenStore {
    config_path: PathBuf,
    token_path: PathBuf,
    pending_state_path: PathBuf,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct BaiduNetdiskUploader {
    token_store: BaiduTokenStore,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct BaiduToken {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<String>,
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduErrorResponse {
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduPrecreateResponse {
    errno: Option<i32>,
    errmsg: Option<String>,
    uploadid: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduUploadPartResponse {
    errno: Option<i32>,
    errmsg: Option<String>,
    md5: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BaiduCreateResponse {
    errno: Option<i32>,
    errmsg: Option<String>,
    fs_id: Option<u64>,
    path: Option<String>,
    size: Option<u64>,
}

impl BaiduTokenStore {
    pub fn new(app_dir: impl AsRef<Path>) -> Self {
        let app_dir = app_dir.as_ref();
        Self {
            config_path: app_dir.join("cloud_config.json"),
            token_path: app_dir.join("baidu_token.json"),
            pending_state_path: app_dir.join("baidu_auth_state.json"),
            client: reqwest::Client::new(),
        }
    }

    pub fn load_config(&self) -> AppResult<CloudConfig> {
        if !self.config_path.exists() {
            return Ok(CloudConfig::default());
        }

        let content = fs::read_to_string(&self.config_path)?;
        let mut config: CloudConfig = serde_json::from_str(&content)?;
        normalize_cloud_config(&mut config);
        Ok(config)
    }

    pub fn save_config(&self, mut config: CloudConfig) -> AppResult<CloudConfig> {
        normalize_cloud_config(&mut config);
        write_json_atomic(&self.config_path, &config)?;
        Ok(config)
    }

    pub async fn auth_status(&self) -> AppResult<CloudAuthStatus> {
        let Some(token) = self.load_token()? else {
            return Ok(CloudAuthStatus::guest(
                CloudProvider::BaiduNetdisk,
                "百度网盘未授权",
            ));
        };

        if !token_is_expired(&token)? {
            return Ok(authorized_status(&token));
        }

        let Some(refresh_token) = token.refresh_token.clone() else {
            return Ok(CloudAuthStatus::guest(
                CloudProvider::BaiduNetdisk,
                "百度网盘授权已过期",
            ));
        };

        let config = self.load_config()?;
        let refreshed = self.refresh_token(&config, &refresh_token).await?;
        self.save_token(&refreshed)?;
        Ok(authorized_status(&refreshed))
    }

    pub async fn access_token(&self) -> AppResult<String> {
        let Some(token) = self.load_token()? else {
            return Err(AppError::AuthRequired);
        };

        if !token_is_expired(&token)? {
            return Ok(token.access_token);
        }

        let Some(refresh_token) = token.refresh_token.clone() else {
            return Err(AppError::AuthRequired);
        };
        let config = self.load_config()?;
        let refreshed = self.refresh_token(&config, &refresh_token).await?;
        let access_token = refreshed.access_token.clone();
        self.save_token(&refreshed)?;
        Ok(access_token)
    }

    pub fn auth_start(&self) -> AppResult<BaiduAuthStartResponse> {
        let config = self.load_config()?;
        let client_id = config.baidu.client_id.trim();
        let redirect_uri = config.baidu.redirect_uri.trim();
        if client_id.is_empty() {
            return Err(cloud_config_error("请先填写百度网盘 API Key"));
        }
        if looks_like_numeric_app_id(client_id) {
            return Err(cloud_config_error(
                "百度网盘授权需要填写 API Key，不是数字形式的应用 ID",
            ));
        }
        if redirect_uri.is_empty() {
            return Err(cloud_config_error("请先填写百度网盘回调地址"));
        }

        let state = format!("bd_{}", Uuid::new_v4());
        write_json_atomic(&self.pending_state_path, &state)?;
        let auth_url = Url::parse_with_params(
            AUTHORIZE_URL,
            &[
                ("response_type", "code"),
                ("client_id", client_id),
                ("redirect_uri", redirect_uri),
                ("scope", config.baidu.scope.trim()),
                ("state", state.as_str()),
                ("display", "tv"),
                ("qrcode", "1"),
                ("force_login", "1"),
            ],
        )
        .map_err(|err| AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: format!("生成百度授权链接失败：{err}"),
        })?;

        Ok(BaiduAuthStartResponse {
            auth_url: auth_url.to_string(),
            state,
        })
    }

    pub async fn auth_finish(&self, input: BaiduAuthFinishRequest) -> AppResult<CloudAuthStatus> {
        let code = input.code.trim();
        if code.is_empty() {
            return Err(cloud_config_error("百度授权码不能为空"));
        }
        self.validate_pending_state(input.state.as_deref())?;

        let config = self.load_config()?;
        ensure_baidu_secret(&config)?;

        let response = self
            .client
            .get(TOKEN_URL)
            .query(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", config.baidu.client_id.trim()),
                ("client_secret", config.baidu.client_secret.trim()),
                ("redirect_uri", config.baidu.redirect_uri.trim()),
            ])
            .send()
            .await?;
        let token = parse_token_response(response).await?;
        self.save_token(&token)?;
        let _ = fs::remove_file(&self.pending_state_path);
        Ok(authorized_status(&token))
    }

    pub fn logout(&self) -> AppResult<CloudAuthStatus> {
        if self.token_path.exists() {
            fs::remove_file(&self.token_path)?;
        }
        if self.pending_state_path.exists() {
            fs::remove_file(&self.pending_state_path)?;
        }
        Ok(CloudAuthStatus::guest(
            CloudProvider::BaiduNetdisk,
            "已退出百度网盘",
        ))
    }

    fn load_token(&self) -> AppResult<Option<BaiduToken>> {
        if !self.token_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.token_path)?;
        Ok(Some(serde_json::from_str(&content)?))
    }

    fn save_token(&self, token: &BaiduToken) -> AppResult<()> {
        write_json_atomic(&self.token_path, token)
    }

    fn validate_pending_state(&self, state: Option<&str>) -> AppResult<()> {
        let Some(state) = state.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(());
        };
        if !self.pending_state_path.exists() {
            return Ok(());
        }

        let expected_content = fs::read_to_string(&self.pending_state_path)?;
        let expected = serde_json::from_str::<String>(&expected_content)
            .unwrap_or_else(|_| expected_content.trim().to_string());
        if expected.trim() != state {
            return Err(cloud_config_error("百度授权状态校验失败，请重新授权"));
        }
        Ok(())
    }

    async fn refresh_token(
        &self,
        config: &CloudConfig,
        refresh_token: &str,
    ) -> AppResult<BaiduToken> {
        ensure_baidu_secret(config)?;
        let response = self
            .client
            .get(TOKEN_URL)
            .query(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", config.baidu.client_id.trim()),
                ("client_secret", config.baidu.client_secret.trim()),
            ])
            .send()
            .await?;
        parse_token_response(response).await
    }
}

impl BaiduNetdiskUploader {
    pub fn new(token_store: BaiduTokenStore) -> Self {
        Self {
            token_store,
            client: reqwest::Client::new(),
        }
    }

    async fn precreate_inner(&self, plan: CloudFilePlan) -> AppResult<CloudUploadSession> {
        if plan.provider != CloudProvider::BaiduNetdisk {
            return Err(cloud_config_error("百度网盘上传器不能处理其他云盘提供方"));
        }
        if plan.expected_part_count() != plan.block_md5.len() {
            return Err(AppError::InvalidInput {
                message: "分片 MD5 数量与文件大小不匹配".to_string(),
            });
        }

        let access_token = self.token_store.access_token().await?;
        let size = plan.size_bytes.to_string();
        let block_list = serde_json::to_string(&plan.block_md5)?;
        let response = self
            .client
            .post(PAN_FILE_URL)
            .query(&[
                ("method", "precreate"),
                ("access_token", access_token.as_str()),
            ])
            .form(&[
                ("path", plan.remote_path.as_str()),
                ("size", size.as_str()),
                ("isdir", "0"),
                ("autoinit", "1"),
                ("rtype", "3"),
                ("block_list", block_list.as_str()),
            ])
            .send()
            .await?;
        let payload = parse_baidu_json::<BaiduPrecreateResponse>(response).await?;
        ensure_baidu_errno(payload.errno, payload.errmsg, "百度预上传失败")?;

        let upload_id = payload.uploadid.ok_or_else(|| AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: "百度预上传响应缺少 uploadid".to_string(),
        })?;

        Ok(CloudUploadSession {
            provider: plan.provider,
            upload_id,
            remote_path: payload.path.unwrap_or(plan.remote_path),
            size_bytes: plan.size_bytes,
            part_size: plan.part_size,
            block_md5: plan.block_md5,
            uploaded_parts: Vec::new(),
        })
    }

    async fn upload_part_inner(
        &self,
        session: CloudUploadSession,
        part_index: usize,
        bytes: Vec<u8>,
    ) -> AppResult<CloudPartResult> {
        if part_index >= session.block_md5.len() {
            return Err(AppError::InvalidInput {
                message: "百度上传分片序号越界".to_string(),
            });
        }

        let access_token = self.token_store.access_token().await?;
        let bytes_uploaded = bytes.len() as u64;
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(format!("part-{part_index}"))
            .mime_str("application/octet-stream")
            .map_err(|err| AppError::Cloud {
                provider: Some(CloudProvider::BaiduNetdisk),
                message: format!("构建百度分片表单失败：{err}"),
            })?;
        let form = reqwest::multipart::Form::new().part("file", part);
        let partseq = part_index.to_string();
        let response = self
            .client
            .post(PCS_UPLOAD_URL)
            .query(&[
                ("method", "upload"),
                ("access_token", access_token.as_str()),
                ("type", "tmpfile"),
                ("path", session.remote_path.as_str()),
                ("uploadid", session.upload_id.as_str()),
                ("partseq", partseq.as_str()),
            ])
            .multipart(form)
            .send()
            .await?;
        let payload = parse_baidu_json::<BaiduUploadPartResponse>(response).await?;
        ensure_baidu_errno(payload.errno, payload.errmsg, "百度分片上传失败")?;

        Ok(CloudPartResult {
            part_index,
            bytes_uploaded,
            md5: payload
                .md5
                .or_else(|| session.block_md5.get(part_index).cloned()),
        })
    }

    async fn finish_inner(&self, session: CloudUploadSession) -> AppResult<CloudFileResult> {
        let access_token = self.token_store.access_token().await?;
        let size = session.size_bytes.to_string();
        let block_list = serde_json::to_string(&session.block_md5)?;
        let response = self
            .client
            .post(PAN_FILE_URL)
            .query(&[
                ("method", "create"),
                ("access_token", access_token.as_str()),
            ])
            .form(&[
                ("path", session.remote_path.as_str()),
                ("size", size.as_str()),
                ("isdir", "0"),
                ("rtype", "3"),
                ("uploadid", session.upload_id.as_str()),
                ("block_list", block_list.as_str()),
            ])
            .send()
            .await?;
        let payload = parse_baidu_json::<BaiduCreateResponse>(response).await?;
        ensure_baidu_errno(payload.errno, payload.errmsg, "百度创建文件失败")?;

        Ok(CloudFileResult {
            provider: session.provider,
            remote_path: payload.path.unwrap_or(session.remote_path),
            file_id: payload.fs_id.map(|id| id.to_string()),
            size_bytes: payload.size.unwrap_or(session.size_bytes),
        })
    }
}

impl CloudUploader for BaiduNetdiskUploader {
    fn auth_status(&self) -> CloudUploadFuture<'_, CloudAuthStatus> {
        Box::pin(async move { self.token_store.auth_status().await })
    }

    fn precreate(&self, plan: CloudFilePlan) -> CloudUploadFuture<'_, CloudUploadSession> {
        Box::pin(async move {
            retry_cloud_operation("百度预上传", || {
                let plan = plan.clone();
                async move { self.precreate_inner(plan).await }
            })
            .await
        })
    }

    fn upload_part<'a>(
        &'a self,
        session: &'a CloudUploadSession,
        part_index: usize,
        bytes: Vec<u8>,
    ) -> CloudUploadFuture<'a, CloudPartResult> {
        let session = session.clone();
        Box::pin(async move {
            retry_cloud_operation("百度分片上传", || {
                let session = session.clone();
                let bytes = bytes.clone();
                async move { self.upload_part_inner(session, part_index, bytes).await }
            })
            .await
        })
    }

    fn finish(&self, session: CloudUploadSession) -> CloudUploadFuture<'_, CloudFileResult> {
        Box::pin(async move {
            retry_cloud_operation("百度创建文件", || {
                let session = session.clone();
                async move { self.finish_inner(session).await }
            })
            .await
        })
    }
}

async fn retry_cloud_operation<T, Fut, Op>(label: &str, mut operation: Op) -> AppResult<T>
where
    Op: FnMut() -> Fut,
    Fut: Future<Output = AppResult<T>>,
{
    const MAX_ATTEMPTS: usize = 4;
    let mut attempt = 0_usize;
    loop {
        attempt += 1;
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) if attempt < MAX_ATTEMPTS && is_retryable_cloud_error(&err) => {
                let delay_ms = 600_u64 * 2_u64.pow((attempt - 1) as u32);
                tracing::warn!(
                    target: "cloud::baidu",
                    "{label}失败，准备第 {} 次重试：{}",
                    attempt + 1,
                    err
                );
                sleep(TokioDuration::from_millis(delay_ms)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

fn is_retryable_cloud_error(err: &AppError) -> bool {
    match err {
        AppError::Network { .. } => true,
        AppError::Cloud { message, .. } => {
            message.contains("HTTP 429")
                || message.contains("HTTP 500")
                || message.contains("HTTP 502")
                || message.contains("HTTP 503")
                || message.contains("HTTP 504")
                || message.contains("限速")
                || message.contains("too many")
                || message.contains("timeout")
        }
        _ => false,
    }
}

async fn parse_token_response(response: reqwest::Response) -> AppResult<BaiduToken> {
    let body = response.text().await?;
    if let Ok(token) = serde_json::from_str::<BaiduTokenResponse>(&body) {
        return Ok(BaiduToken {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at: token.expires_in.map(expires_at_after_seconds),
            scope: token.scope,
        });
    }

    let detail = serde_json::from_str::<BaiduErrorResponse>(&body)
        .ok()
        .and_then(|err| {
            err.error_description
                .or(err.error)
                .filter(|message| !message.trim().is_empty())
        })
        .unwrap_or(body);
    Err(AppError::Cloud {
        provider: Some(CloudProvider::BaiduNetdisk),
        message: format!("百度授权失败：{detail}"),
    })
}

async fn parse_baidu_json<T>(response: reqwest::Response) -> AppResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: format!("百度网盘请求失败，HTTP {status}；响应：{body}"),
        });
    }
    serde_json::from_str::<T>(&body).map_err(|err| AppError::Cloud {
        provider: Some(CloudProvider::BaiduNetdisk),
        message: format!("百度网盘响应解析失败：{err}；响应：{body}"),
    })
}

fn ensure_baidu_errno(errno: Option<i32>, errmsg: Option<String>, fallback: &str) -> AppResult<()> {
    match errno.unwrap_or(0) {
        0 => Ok(()),
        code => Err(AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: format!(
                "{fallback}，错误码 {code}{}",
                errmsg
                    .filter(|message| !message.trim().is_empty())
                    .map(|message| format!("：{message}"))
                    .unwrap_or_default()
            ),
        }),
    }
}

fn normalize_cloud_config(config: &mut CloudConfig) {
    if config.default_remote_dir.trim().is_empty() {
        config.default_remote_dir = CloudConfig::default().default_remote_dir;
    }
    if config.part_size_mb == 0 {
        config.part_size_mb = 4;
    }
    let redirect_uri = config.baidu.redirect_uri.trim();
    if redirect_uri.is_empty() || redirect_uri == "http://localhost:1421/baidu/callback" {
        config.baidu.redirect_uri = "oob".to_string();
    }
    if config.baidu.scope.trim().is_empty() {
        config.baidu.scope = "basic,netdisk".to_string();
    }
}

fn ensure_baidu_secret(config: &CloudConfig) -> AppResult<()> {
    if config.baidu.client_id.trim().is_empty() {
        return Err(cloud_config_error("请先填写百度网盘 API Key"));
    }
    if looks_like_numeric_app_id(config.baidu.client_id.trim()) {
        return Err(cloud_config_error(
            "百度网盘授权需要填写 API Key，不是数字形式的应用 ID",
        ));
    }
    if config.baidu.client_secret.trim().is_empty() {
        return Err(cloud_config_error("请先填写百度网盘 Secret Key"));
    }
    Ok(())
}

fn looks_like_numeric_app_id(value: &str) -> bool {
    let value = value.trim();
    value.len() >= 6 && value.chars().all(|ch| ch.is_ascii_digit())
}

fn token_is_expired(token: &BaiduToken) -> AppResult<bool> {
    let Some(expires_at) = token.expires_at.as_deref() else {
        return Ok(false);
    };
    let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at)
        .map_err(|err| AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: format!("百度 token 过期时间格式无效：{err}"),
        })?
        .with_timezone(&Utc);
    Ok(Utc::now() + Duration::minutes(5) >= expires_at)
}

fn authorized_status(token: &BaiduToken) -> CloudAuthStatus {
    CloudAuthStatus {
        provider: CloudProvider::BaiduNetdisk,
        is_authorized: true,
        account_name: None,
        expires_at: token.expires_at.clone(),
        message: Some("百度网盘已授权".to_string()),
    }
}

fn expires_at_after_seconds(seconds: i64) -> String {
    (Utc::now() + Duration::seconds(seconds)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn cloud_config_error(message: impl Into<String>) -> AppError {
    AppError::Cloud {
        provider: Some(CloudProvider::BaiduNetdisk),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bili_baidu_{name}_{}", std::process::id()))
    }

    #[test]
    fn cloud_config_roundtrips_defaults() -> AppResult<()> {
        let root = test_root("config");
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        let store = BaiduTokenStore::new(&root);

        let mut config = CloudConfig::default();
        config.default_remote_dir = "/apps/demo".to_string();
        config.baidu.client_id = "client".to_string();
        store.save_config(config.clone())?;

        let loaded = store.load_config()?;
        assert_eq!(loaded.default_remote_dir, "/apps/demo");
        assert_eq!(loaded.baidu.client_id, "client");

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn auth_start_requires_client_id() {
        let root = test_root("missing_client");
        let store = BaiduTokenStore::new(root);
        let result = store.auth_start();
        assert!(matches!(result, Err(AppError::Cloud { .. })));
    }

    #[test]
    fn auth_start_rejects_numeric_app_id() -> AppResult<()> {
        let root = test_root("numeric_app_id");
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        let store = BaiduTokenStore::new(&root);
        let mut config = CloudConfig::default();
        config.baidu.client_id = "24092476".to_string();
        config.baidu.client_secret = "secret".to_string();
        store.save_config(config)?;

        let result = store.auth_start();

        assert!(matches!(result, Err(AppError::Cloud { .. })));
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn auth_start_builds_authorize_url_and_state() -> AppResult<()> {
        let root = test_root("auth_url");
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        let store = BaiduTokenStore::new(&root);
        let mut config = CloudConfig::default();
        config.baidu.client_id = "client-id".to_string();
        config.baidu.client_secret = "secret".to_string();
        store.save_config(config)?;

        let response = store.auth_start()?;

        assert!(response.auth_url.contains("openapi.baidu.com"));
        assert!(response.auth_url.contains("client_id=client-id"));
        assert!(response.auth_url.contains("redirect_uri=oob"));
        assert!(response.auth_url.contains("display=tv"));
        assert!(response.auth_url.contains("qrcode=1"));
        assert!(response.auth_url.contains("force_login=1"));
        assert!(response.state.starts_with("bd_"));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn validate_pending_state_accepts_saved_json_string() -> AppResult<()> {
        let root = test_root("state_json");
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        let store = BaiduTokenStore::new(&root);
        write_json_atomic(&store.pending_state_path, &"bd_state_1".to_string())?;

        store.validate_pending_state(Some("bd_state_1"))?;

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn baidu_errno_zero_is_success() -> AppResult<()> {
        ensure_baidu_errno(Some(0), None, "失败")?;
        Ok(())
    }

    #[test]
    fn baidu_errno_nonzero_maps_cloud_error() {
        let result = ensure_baidu_errno(Some(-6), Some("鉴权失败".to_string()), "失败");
        assert!(matches!(result, Err(AppError::Cloud { .. })));
    }

    #[test]
    fn retryable_cloud_error_matches_network_and_rate_limits() {
        assert!(is_retryable_cloud_error(&AppError::Network {
            message: "connection reset".to_string(),
        }));
        assert!(is_retryable_cloud_error(&AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: "百度网盘请求失败，HTTP 503 Service Unavailable".to_string(),
        }));
        assert!(!is_retryable_cloud_error(&AppError::Cloud {
            provider: Some(CloudProvider::BaiduNetdisk),
            message: "百度授权失败：invalid_grant".to_string(),
        }));
    }
}
