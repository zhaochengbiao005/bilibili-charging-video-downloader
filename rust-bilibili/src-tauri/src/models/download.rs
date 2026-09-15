use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlayUrlRequest {
    pub bvid: String,
    pub cid: u64,
    pub qn: u32,
    pub cookie_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlayUrlResponse {
    pub quality: u32,
    pub timelength: u64,
    pub accept_quality: Vec<u32>,
    pub dash: Option<DashStreams>,
    pub durl: Vec<DurlSegment>,
}

impl PlayUrlResponse {
    /// 实际可下载流时长（毫秒）。优先 DURL 分段 length / DASH duration，
    /// 避免直接相信 timelength（充电试看时它常写成全片时长）。
    pub fn stream_duration_ms(&self) -> u64 {
        let durl_ms: u64 = self.durl.iter().map(|segment| segment.length).sum();
        if durl_ms > 0 {
            return durl_ms;
        }
        if let Some(dash) = &self.dash {
            if dash.duration > 0 {
                return dash.duration.saturating_mul(1000);
            }
        }
        self.timelength
    }

    /// 判断当前 playurl 是否为充电/付费试看流（而非全片）。
    pub fn is_preview_stream(&self, meta_duration_sec: u64) -> bool {
        if meta_duration_sec == 0 {
            return false;
        }

        // 充电试看专用编码档（如 *-1-448.mp4）
        let has_preview_encode = self.durl.iter().any(|segment| {
            looks_like_upower_preview_url(&segment.url)
                || segment
                    .backup_urls
                    .iter()
                    .any(|url| looks_like_upower_preview_url(url))
        });
        if has_preview_encode && meta_duration_sec > 30 {
            return true;
        }

        let meta_ms = meta_duration_sec.saturating_mul(1000);
        let stream_ms = {
            let durl_ms: u64 = self.durl.iter().map(|segment| segment.length).sum();
            if durl_ms > 0 {
                durl_ms
            } else if let Some(dash) = &self.dash {
                if !dash.video.is_empty() && dash.duration > 0 {
                    dash.duration.saturating_mul(1000)
                } else {
                    0
                }
            } else {
                0
            }
        };
        if stream_ms == 0 {
            return false;
        }

        // 有完整 DASH 视频轨时，以 dash 时长对比稿件时长
        if let Some(dash) = &self.dash {
            if !dash.video.is_empty() {
                return stream_ms < meta_ms / 2 && stream_ms.saturating_add(3_000) < meta_ms;
            }
        }

        // 仅 DURL、且明显短于全片 → 典型充电试看
        stream_ms < meta_ms / 2 && stream_ms.saturating_add(5_000) < meta_ms
    }

    pub fn preview_block_message(&self, meta_duration_sec: u64) -> String {
        let stream_sec = self.stream_duration_ms() / 1000;
        format!(
            "当前账号仅能获取充电试看流（约 {stream_sec}s / 全片 {meta_duration_sec}s）。请确认已登录，并对该 UP 开通对应档位包月充电后重试。"
        )
    }
}

fn looks_like_upower_preview_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("-1-448.") || lower.contains("-448.mp4") || lower.contains("-448.m4s")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DashStreams {
    pub duration: u64,
    pub video: Vec<DashTrack>,
    pub audio: Vec<DashTrack>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DashTrack {
    pub id: u32,
    pub codecs: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<String>,
    pub bandwidth: Option<u64>,
    pub size_bytes: Option<u64>,
    pub mime_type: Option<String>,
    pub base_url: String,
    pub backup_urls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DurlSegment {
    pub order: u32,
    pub length: u64,
    pub size: u64,
    pub url: String,
    pub backup_urls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StartDownloadRequest {
    pub bvid: String,
    pub cid: Option<u64>,
    pub page: Option<u32>,
    pub part: Option<String>,
    pub quality: String,
    pub format: String,
    pub outdir: String,
    pub cookie_path: Option<String>,
    pub skip_merge: bool,
    #[serde(default)]
    pub download_danmaku: bool,
    #[serde(default)]
    pub danmaku_mode: DanmakuMode,
    #[serde(default = "default_threads")]
    pub threads: usize,
    /// 互动视频下载模式
    #[serde(default)]
    pub stein_mode: crate::models::stein::SteinDownloadMode,
    /// 路径模式：从入口起依次选择的 choice edge_id 列表
    #[serde(default)]
    pub stein_path_edges: Vec<u64>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DanmakuMode {
    #[default]
    None,
    Ass,
    Burn,
}

impl StartDownloadRequest {
    pub fn effective_danmaku_mode(&self) -> DanmakuMode {
        if self.format != "video" {
            DanmakuMode::None
        } else if self.danmaku_mode != DanmakuMode::None {
            self.danmaku_mode
        } else if self.download_danmaku {
            DanmakuMode::Ass
        } else {
            DanmakuMode::None
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StartDownloadResponse {
    pub task_id: String,
}

/// 下载任务阶段词表：Rust / TS 双端唯一来源，serde 输出 snake_case。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStage {
    Queued,
    Resolving,
    DownloadingVideo,
    DownloadingAudio,
    DownloadingSegments,
    Merging,
    ConvertingAudio,
    DownloadingDanmaku,
    BurningDanmaku,
    CloudCalculatingMd5,
    CloudPrecreating,
    CloudUploadingVideo,
    CloudUploadingAudio,
    CloudUploadingDanmaku,
    CloudMuxingMp4,
    Completed,
}

/// 任务终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgressEvent {
    pub task_id: String,
    pub stage: DownloadStage,
    pub percent: f32,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub speed_bytes_per_sec: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadDoneEvent {
    pub task_id: String,
    pub status: TaskStatus,
    pub message: Option<String>,
}

fn default_threads() -> usize {
    8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_preview_playurl() -> PlayUrlResponse {
        PlayUrlResponse {
            quality: 64,
            timelength: 124_300,
            accept_quality: vec![112, 80, 64],
            dash: None,
            durl: vec![DurlSegment {
                order: 1,
                length: 14_977,
                size: 1_682_316,
                url: "https://upos-sz-estgcos.bilivideo.com/upgcxcode/52/00/1/1-1-448.mp4?e=1"
                    .to_string(),
                backup_urls: vec![],
            }],
        }
    }

    #[test]
    fn detects_upower_preview_by_448_encode_and_short_durl() {
        let playurl = sample_preview_playurl();
        assert!(playurl.is_preview_stream(125));
        assert_eq!(playurl.stream_duration_ms(), 14_977);
        assert!(playurl.preview_block_message(125).contains("试看"));
    }

    #[test]
    fn full_dash_is_not_preview() {
        let playurl = PlayUrlResponse {
            quality: 80,
            timelength: 125_000,
            accept_quality: vec![80],
            dash: Some(DashStreams {
                duration: 125,
                video: vec![DashTrack {
                    id: 80,
                    codecs: "avc1".to_string(),
                    width: Some(1920),
                    height: Some(1080),
                    frame_rate: Some("30".to_string()),
                    bandwidth: Some(2_000_000),
                    size_bytes: Some(20_000_000),
                    mime_type: Some("video/mp4".to_string()),
                    base_url: "https://example.test/v.m4s".to_string(),
                    backup_urls: vec![],
                }],
                audio: vec![],
            }),
            durl: vec![],
        };
        assert!(!playurl.is_preview_stream(125));
    }

    #[test]
    fn short_full_video_is_not_preview() {
        let playurl = PlayUrlResponse {
            quality: 64,
            timelength: 12_000,
            accept_quality: vec![64],
            dash: None,
            durl: vec![DurlSegment {
                order: 1,
                length: 12_000,
                size: 500_000,
                url: "https://example.test/full-1-64.mp4".to_string(),
                backup_urls: vec![],
            }],
        };
        assert!(!playurl.is_preview_stream(12));
    }

    #[test]
    fn stage_serialization_matches_ts_union() {
        assert_eq!(
            serde_json::to_string(&DownloadStage::CloudUploadingVideo).unwrap(),
            r#""cloud_uploading_video""#
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Cancelled).unwrap(),
            r#""cancelled""#
        );
        assert!(matches!(
            serde_json::from_str::<DownloadStage>("\"burning_danmaku\"").unwrap(),
            DownloadStage::BurningDanmaku
        ));
    }
}
