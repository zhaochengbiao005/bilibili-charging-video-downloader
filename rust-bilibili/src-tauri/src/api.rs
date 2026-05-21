use std::time::Duration;

use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, COOKIE, ORIGIN, REFERER, SET_COOKIE,
    USER_AGENT,
};
use serde::Deserialize;

use crate::{
    auth::{
        build_qrcode_svg, parse_set_cookie_headers, CookieSet, LoginStatus, QrLoginPollOutcome,
        QrLoginPollResponse, QrLoginStartResponse,
    },
    error::{AppError, AppResult},
    models::video::{StreamOption, VideoData, VideoPage},
};

const BASE_URL: &str = "https://api.bilibili.com";
const PASSPORT_URL: &str = "https://passport.bilibili.com";

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

    pub async fn video_info(
        &self,
        bvid: &str,
        cookies: Option<&CookieSet>,
    ) -> AppResult<VideoData> {
        let login = match cookies {
            Some(cookies) => Some(self.check_login(cookies).await?),
            None => None,
        };

        let mut request = self
            .client
            .get(format!("{BASE_URL}/x/web-interface/view"))
            .query(&[("bvid", bvid)]);
        if let Some(cookies) = cookies {
            request = request.header(COOKIE, cookies.to_header());
        }

        let response = request
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

        let mut video = data.into_video_data();
        if let Some(login) = login {
            video.is_login = Some(login.is_login);
            video.login_name = login.username;
            video.login_level = login.level;
        }
        Ok(video)
    }

    pub async fn check_login(&self, cookies: &CookieSet) -> AppResult<LoginStatus> {
        if !cookies.has_login_cookie() {
            return Ok(LoginStatus::guest("Cookie 中缺少 SESSDATA"));
        }

        let response = self
            .client
            .get(format!("{BASE_URL}/x/web-interface/nav"))
            .header(COOKIE, cookies.to_header())
            .header(REFERER, "https://www.bilibili.com/")
            .send()
            .await?
            .error_for_status()?
            .json::<ApiResponse<NavData>>()
            .await?;

        if response.code != 0 {
            return Ok(LoginStatus::guest(response.message));
        }

        let Some(data) = response.data else {
            return Ok(LoginStatus::guest("B站响应缺少登录状态"));
        };

        Ok(data.into_login_status(response.message))
    }

    pub async fn start_qr_login(&self) -> AppResult<QrLoginStartResponse> {
        let response = self
            .client
            .get(format!(
                "{PASSPORT_URL}/x/passport-login/web/qrcode/generate"
            ))
            .send()
            .await?
            .error_for_status()?
            .json::<ApiResponse<QrGenerateData>>()
            .await?;

        if response.code != 0 {
            return Err(AppError::Api {
                code: response.code,
                message: response.message,
            });
        }

        let data = response.data.ok_or_else(|| AppError::Api {
            code: response.code,
            message: "B站响应缺少二维码登录数据".to_string(),
        })?;

        Ok(QrLoginStartResponse {
            qrcode_svg: build_qrcode_svg(&data.url)?,
            url: data.url,
            qrcode_key: data.qrcode_key,
            expires_in_sec: 180,
        })
    }

    pub async fn poll_qr_login(&self, qrcode_key: &str) -> AppResult<QrLoginPollOutcome> {
        let response = self
            .client
            .get(format!("{PASSPORT_URL}/x/passport-login/web/qrcode/poll"))
            .query(&[("qrcode_key", qrcode_key)])
            .send()
            .await?
            .error_for_status()?;

        let cookies = parse_set_cookie_headers(
            response
                .headers()
                .get_all(SET_COOKIE)
                .iter()
                .filter_map(|value| value.to_str().ok()),
        )
        .login_only();

        let body = response.json::<ApiResponse<QrPollData>>().await?;
        if body.code != 0 {
            return Err(AppError::Api {
                code: body.code,
                message: body.message,
            });
        }

        let data = body.data.ok_or_else(|| AppError::Api {
            code: body.code,
            message: "B站响应缺少扫码状态".to_string(),
        })?;

        let (status, message) = match data.code {
            0 => ("confirmed", "登录成功"),
            86038 => ("expired", "二维码已过期"),
            86090 => ("scanned", "已扫码，请在手机上确认登录"),
            86101 => ("waiting", "等待扫码"),
            _ => ("unknown", data.message.as_str()),
        };

        let login = if data.code == 0 && cookies.has_login_cookie() {
            Some(self.check_login(&cookies).await?)
        } else {
            None
        };

        Ok(QrLoginPollOutcome {
            response: QrLoginPollResponse {
                status: status.to_string(),
                message: message.to_string(),
                login,
            },
            cookies: (data.code == 0 && cookies.has_login_cookie()).then_some(cookies),
        })
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
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
    );
    headers.insert(ORIGIN, HeaderValue::from_static("https://www.bilibili.com"));
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://www.bilibili.com/"),
    );
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
        let is_charging = self.rights.elec_high == 1;
        let is_vip = self.rights.vip_free == 1;
        let streams = default_stream_options(is_charging, is_vip);
        let qualities = streams
            .iter()
            .filter(|stream| stream.available)
            .map(|stream| stream.label.clone())
            .collect();
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
            qualities,
            streams,
            is_charging: Some(is_charging),
            is_vip: Some(is_vip),
            vip_type: None,
            is_login: None,
            login_name: None,
            login_level: None,
            desc: Some(self.desc),
            error: None,
        }
    }
}

fn default_stream_options(is_charging: bool, is_vip: bool) -> Vec<StreamOption> {
    let restricted = is_charging || is_vip;
    [
        (80, "1080P", true, false),
        (64, "720P", false, false),
        (32, "480P", false, false),
        (16, "360P", false, false),
    ]
    .into_iter()
    .map(|(qn, label, maybe_login, maybe_vip)| {
        let requires_login = maybe_login && restricted;
        let requires_vip = maybe_vip && is_vip;
        StreamOption {
            id: format!("video_{qn}"),
            qn,
            label: label.to_string(),
            codec: None,
            width: None,
            height: None,
            frame_rate: None,
            bandwidth: None,
            requires_login,
            requires_vip,
            available: !requires_vip,
            unavailable_reason: requires_vip.then(|| "需要大会员权限".to_string()),
        }
    })
    .collect()
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

#[derive(Debug, Default, Deserialize)]
struct NavData {
    #[serde(default, rename = "isLogin")]
    is_login: bool,
    #[serde(default)]
    uname: String,
    #[serde(default)]
    mid: u64,
    #[serde(default)]
    level_info: LevelInfo,
    #[serde(default, rename = "vipType")]
    vip_type: u32,
}

impl NavData {
    fn into_login_status(self, api_message: String) -> LoginStatus {
        LoginStatus {
            is_login: self.is_login,
            username: self.is_login.then_some(self.uname),
            uid: self.is_login.then_some(self.mid),
            level: self.is_login.then_some(self.level_info.current_level),
            vip_type: self.is_login.then_some(self.vip_type),
            message: (!self.is_login).then_some(if api_message.is_empty() {
                "Cookie 未登录或已失效".to_string()
            } else {
                api_message
            }),
            cookie_path: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct LevelInfo {
    #[serde(default)]
    current_level: u32,
}

#[derive(Debug, Deserialize)]
struct QrGenerateData {
    url: String,
    qrcode_key: String,
}

#[derive(Debug, Deserialize)]
struct QrPollData {
    code: i32,
    #[serde(default)]
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn public_video_info_contract_returns_streams_when_enabled() -> AppResult<()> {
        let Ok(bvid) = std::env::var("BILI_LIVE_TEST_BVID") else {
            return Ok(());
        };

        let client = BilibiliClient::new()?;
        let video = client.video_info(&bvid, None).await?;

        assert_eq!(video.id, bvid);
        assert!(!video.title.trim().is_empty());
        assert!(!video.author.trim().is_empty());
        assert!(!video.pages.is_empty());
        assert!(!video.qualities.is_empty());
        assert!(!video.streams.is_empty());
        assert!(video.streams.iter().any(|stream| stream.available));
        Ok(())
    }
}
