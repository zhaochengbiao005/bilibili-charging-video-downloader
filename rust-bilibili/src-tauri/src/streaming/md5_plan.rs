use std::sync::{
    atomic::AtomicBool,
    Arc,
};

use crate::{
    download::{
        bili_http::{md5_hex, plan_ranges},
        ensure_not_cancelled,
    },
    error::{AppError, AppResult},
    models::cloud::{CloudFilePlan, CloudProvider, CloudUploadMode},
    streaming::bili_stream::{BiliStreamClient, BiliStreamSpec},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Md5PlanInput {
    pub provider: CloudProvider,
    pub mode: CloudUploadMode,
    pub remote_path: String,
    pub part_size: u64,
    pub content_type: Option<String>,
}

pub async fn build_cloud_file_plan(
    client: &BiliStreamClient,
    spec: &BiliStreamSpec,
    input: Md5PlanInput,
    cancel: &Arc<AtomicBool>,
) -> AppResult<CloudFilePlan> {
    ensure_not_cancelled(cancel, &spec.task_id)?;
    if input.part_size == 0 {
        return Err(AppError::InvalidInput {
            message: "云盘分片大小必须大于 0".to_string(),
        });
    }
    if input.remote_path.trim().is_empty() {
        return Err(AppError::InvalidInput {
            message: "云盘保存路径不能为空".to_string(),
        });
    }

    let size_bytes = client
        .probe_size(spec)
        .await?
        .ok_or_else(|| AppError::Download {
            task_id: Some(spec.task_id.clone()),
            message: "无法获取 B站流大小，不能生成云盘上传计划".to_string(),
        })?;

    let mut block_md5 = Vec::new();
    for (start, end) in plan_ranges(size_bytes, input.part_size) {
        ensure_not_cancelled(cancel, &spec.task_id)?;
        let bytes = client.read_range(spec, start, end, cancel).await?;
        block_md5.push(md5_hex(&bytes));
    }

    Ok(CloudFilePlan {
        provider: input.provider,
        mode: input.mode,
        remote_path: input.remote_path,
        size_bytes,
        part_size: input.part_size,
        block_md5,
        content_type: input.content_type,
    })
}
