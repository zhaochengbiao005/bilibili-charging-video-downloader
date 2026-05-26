use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{
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

fn plan_ranges(size_bytes: u64, part_size: u64) -> Vec<(u64, u64)> {
    if size_bytes == 0 || part_size == 0 {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = 0_u64;
    while start < size_bytes {
        let end = (start + part_size - 1).min(size_bytes - 1);
        ranges.push((start, end));
        start = end + 1;
    }
    ranges
}

fn md5_hex(bytes: &[u8]) -> String {
    format!("{:x}", md5::compute(bytes))
}

fn ensure_not_cancelled(cancel: &Arc<AtomicBool>, task_id: &str) -> AppResult<()> {
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::Download {
            task_id: Some(task_id.to_string()),
            message: "任务已取消".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_cover_file_without_overlap() {
        assert_eq!(plan_ranges(10, 4), vec![(0, 3), (4, 7), (8, 9)]);
        assert_eq!(plan_ranges(8, 4), vec![(0, 3), (4, 7)]);
        assert!(plan_ranges(0, 4).is_empty());
    }

    #[test]
    fn md5_hex_matches_known_value() {
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    }
}
