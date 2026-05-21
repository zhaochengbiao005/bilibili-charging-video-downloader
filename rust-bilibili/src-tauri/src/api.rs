use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, ORIGIN, REFERER, USER_AGENT};
use serde::Deserialize;

use crate::{
    error::{AppError, AppResult},
    models::video::{VideoData, VideoPage},
};

const BASE_URL: &str = "https://api.bilibili.com";

#[derive(Debug, Clone)]
pub struct BilibiliClient {
    client: reqwest::Client,
}

impl BilibiliClient {
    pub fn new() -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .default_headers(default_headers())
            .timeout(Duration::from_secs(20))
            .cookie_store(true)
            .build()?;

        Ok(Self { client })
    }

    pub async fn video_info(&self, bvid: &str) -> AppResult<VideoData> {
        let response = self
            .client
            .get(format!("{BASE_URL}/x/web-interface/view"))
            .query(&[("bvid", bvid)])
            .send()
            .await?
            .error_for_status()?
            .json::<ApiResponse<ViewData>>()
            .await?;

        if response.code != 0 {
            return Err(AppError::Api {
                code: response.code,
                message: response.message,
            });
        }

        let data = response.data.ok_or_else(|| AppError::Api {
            code: response.code,
            message: "B站响应缺少视频信息".to_string(),
        })?;

        Ok(data.into_video_data())
    }
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
        ),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("application/json, text/plain, */*"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"));
    headers.insert(ORIGIN, HeaderValue::from_static("https://www.bilibili.com"));
    headers.insert(REFERER, HeaderValue::from_static("https://www.bilibili.com/"));
    headers
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

fn format_count(count: u64) -> String {
    if count >= 100_000_000 {
        format!("{:.1}亿", count as f64 / 100_000_000.0)
    } else if count >= 10_000 {
        format!("{:.1}万", count as f64 / 10_000.0)
    } else {
        count.to_string()
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    code: i32,
    #[serde(default)]
    message: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct ViewData {
    bvid: String,
    title: String,
    #[serde(default)]
    owner: Owner,
    #[serde(default)]
    pic: String,
    #[serde(default)]
    stat: Stat,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    pages: Vec<ViewPage>,
    #[serde(default)]
    rights: Rights,
    #[serde(default)]
    desc: String,
}

impl ViewData {
    fn into_video_data(self) -> VideoData {
        let pages = self
            .pages
            .into_iter()
            .map(|page| VideoPage {
                cid: page.cid,
                page: page.page,
                part: page.part,
            })
            .collect();

        VideoData {
            id: self.bvid,
            title: self.title,
            author: self.owner.name,
            thumbnail: self.pic,
            views: format_count(self.stat.view),
            duration: format_duration(self.duration),
            duration_sec: self.duration,
            pages,
            qualities: vec![
                "1080P".to_string(),
                "720P".to_string(),
                "480P".to_string(),
                "360P".to_string(),
            ],
            is_charging: Some(self.rights.elec_high == 1),
            is_vip: Some(self.rights.vip_free == 1),
            vip_type: None,
            is_login: None,
            login_name: None,
            login_level: None,
            desc: Some(self.desc),
            error: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct Owner {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct Stat {
    #[serde(default)]
    view: u64,
}

#[derive(Debug, Default, Deserialize)]
struct Rights {
    #[serde(default)]
    elec_high: u32,
    #[serde(default)]
    vip_free: u32,
}

#[derive(Debug, Deserialize)]
struct ViewPage {
    cid: u64,
    page: u32,
    part: String,
}
