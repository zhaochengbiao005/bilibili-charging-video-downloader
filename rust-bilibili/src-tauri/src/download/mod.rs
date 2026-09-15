//! 下载与云盘直传共用的流解析策略。
//!
//! 本地下载管线（downloader.rs + commands.rs）与云盘直传管线（cloud/direct.rs）
//! 都从这里取画质映射、轨道选择与地址挑选，保证 B站流知识只有一份实现、一处可测。

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{
    error::{AppError, AppResult},
    models::{
        download::{DashTrack, StartDownloadRequest},
        video::VideoData,
    },
};

pub mod bili_http;
pub mod task;

pub use crate::models::download::DownloadStage;
pub use task::TaskOrchestrator;

/// 画质标签 → B站 qn 编号。
pub fn quality_to_qn(quality: &str) -> u32 {
    if quality.contains("8K") {
        127
    } else if quality.contains("杜比") || quality.to_ascii_lowercase().contains("dolby") {
        126
    } else if quality.contains("HDR") {
        125
    } else if quality.contains("4K") {
        120
    } else if quality.contains("1080P60") {
        116
    } else if quality.contains("1080P+") {
        112
    } else if quality.contains("720P60") {
        74
    } else if quality.contains("1080P") {
        80
    } else if quality.contains("720P") {
        64
    } else if quality.contains("480P") {
        32
    } else {
        16
    }
}

/// 按 qn 选视频轨：只看不超过请求档位的轨，杜比/HDR 变体按需加权，兜底取最高码率轨。
pub fn select_video_track(tracks: &[DashTrack], qn: u32) -> Option<DashTrack> {
    let wants_dolby = qn == 126;
    let wants_hdr = qn == 125;
    tracks
        .iter()
        .filter(|track| track.id <= qn)
        .max_by_key(|track| {
            (
                track.id,
                u8::from(wants_dolby && is_dolby_track(track)),
                u8::from(wants_hdr && is_hdr_track(track)),
                track.bandwidth.unwrap_or_default(),
            )
        })
        .or_else(|| {
            tracks
                .iter()
                .max_by_key(|track| track.bandwidth.unwrap_or_default())
        })
        .cloned()
}

pub fn is_dolby_track(track: &DashTrack) -> bool {
    let codecs = track.codecs.to_ascii_lowercase();
    codecs.contains("dvh")
        || codecs.contains("dvhe")
        || codecs.contains("dolby")
        || track
            .mime_type
            .as_deref()
            .is_some_and(|mime| mime.to_ascii_lowercase().contains("dolby"))
}

pub fn is_hdr_track(track: &DashTrack) -> bool {
    let codecs = track.codecs.to_ascii_lowercase();
    codecs.contains("hev1") || codecs.contains("hvc1") || track.id == 125
}

/// 音频轨：优先精确匹配 30280/30232/30216 档位，兜底取最高码率轨。
pub fn select_audio_track(tracks: &[DashTrack], quality: &str) -> Option<DashTrack> {
    let target_id = if quality.contains("128kbps") {
        30216
    } else if quality.contains("192kbps") {
        30232
    } else {
        30280
    };

    tracks
        .iter()
        .find(|track| track.id == target_id)
        .or_else(|| {
            tracks
                .iter()
                .max_by_key(|track| track.bandwidth.unwrap_or_default())
        })
        .cloned()
}

/// 主地址为空时取第一个非空备用地址。
pub fn first_url(primary: &str, backups: &[String]) -> AppResult<String> {
    if !primary.trim().is_empty() {
        return Ok(primary.to_string());
    }
    backups
        .iter()
        .find(|url| !url.trim().is_empty())
        .cloned()
        .ok_or_else(|| AppError::Download {
            task_id: None,
            message: "播放地址为空".to_string(),
        })
}

/// 分 P 选择：cid 精确匹配 → 页码匹配 → 第一分 P。
pub fn selected_page<'a>(
    video: &'a VideoData,
    input: &StartDownloadRequest,
) -> AppResult<&'a crate::models::video::VideoPage> {
    input
        .cid
        .and_then(|cid| video.pages.iter().find(|page| page.cid == cid))
        .or_else(|| {
            input
                .page
                .and_then(|page_no| video.pages.iter().find(|page| page.page == page_no))
        })
        .or_else(|| video.pages.first())
        .ok_or_else(|| AppError::InvalidInput {
            message: "视频没有可下载分P".to_string(),
        })
}

/// 统一取消检查：所有管线从这里取，取消是类型化错误（AppError::Cancelled），不再靠文案。
pub fn ensure_not_cancelled(cancel: &Arc<AtomicBool>, task_id: &str) -> AppResult<()> {
    if cancel.load(Ordering::SeqCst) {
        return Err(AppError::Cancelled {
            task_id: Some(task_id.to_string()),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dash_audio_track(id: u32, bandwidth: u64) -> DashTrack {
        DashTrack {
            id,
            codecs: "mp4a".to_string(),
            width: None,
            height: None,
            frame_rate: None,
            bandwidth: Some(bandwidth),
            size_bytes: Some(bandwidth),
            mime_type: Some("audio/mp4".to_string()),
            base_url: format!("https://example.test/{id}.m4s"),
            backup_urls: vec![],
        }
    }

    fn dash_video_track(id: u32, codecs: &str, bandwidth: u64) -> DashTrack {
        DashTrack {
            id,
            codecs: codecs.to_string(),
            width: Some(if id == 127 { 7680 } else { 3840 }),
            height: Some(if id == 127 { 4320 } else { 2160 }),
            frame_rate: Some("60".to_string()),
            bandwidth: Some(bandwidth),
            size_bytes: Some(bandwidth),
            mime_type: Some("video/mp4".to_string()),
            base_url: format!("https://example.test/video-{id}-{bandwidth}.m4s"),
            backup_urls: vec![],
        }
    }

    #[test]
    fn quality_to_qn_supports_8k_and_dolby() {
        assert_eq!(quality_to_qn("8K"), 127);
        assert_eq!(quality_to_qn("杜比视界"), 126);
        assert_eq!(quality_to_qn("8K 杜比视界"), 127);
        assert_eq!(quality_to_qn("HDR"), 125);
        assert_eq!(quality_to_qn("1080P+"), 112);
        assert_eq!(quality_to_qn("720P60"), 74);
    }

    #[test]
    fn select_video_track_prefers_requested_dolby_variant() {
        let tracks = vec![
            dash_video_track(126, "hev1.2.4.L153", 8_000_000),
            dash_video_track(126, "dvh1.08.07", 7_000_000),
            dash_video_track(120, "avc1.640034", 10_000_000),
        ];

        let selected = select_video_track(&tracks, 126).expect("track should be selected");

        assert_eq!(selected.id, 126);
        assert!(selected.codecs.contains("dvh"));
    }

    #[test]
    fn select_audio_track_prefers_matching_bilibili_audio_id() {
        let tracks = vec![
            dash_audio_track(30280, 320_000),
            dash_audio_track(30232, 192_000),
            dash_audio_track(30216, 128_000),
        ];

        assert_eq!(
            select_audio_track(&tracks, "320kbps 高品质").unwrap().id,
            30280
        );
        assert_eq!(
            select_audio_track(&tracks, "192kbps 标准").unwrap().id,
            30232
        );
        assert_eq!(
            select_audio_track(&tracks, "128kbps 基础").unwrap().id,
            30216
        );
    }

    #[test]
    fn first_url_prefers_primary_and_falls_back_to_backup() -> AppResult<()> {
        assert_eq!(first_url("https://a", &[])?, "https://a");
        assert_eq!(
            first_url("", &["".to_string(), "https://b".to_string()])?,
            "https://b"
        );
        Ok(())
    }
}
