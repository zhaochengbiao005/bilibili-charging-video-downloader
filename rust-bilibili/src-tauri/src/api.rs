use std::time::Duration;

use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CONTENT_LENGTH, CONTENT_RANGE, COOKIE, ORIGIN,
    RANGE, REFERER, SET_COOKIE, USER_AGENT,
};
use serde::Deserialize;

use crate::{
    auth::{
        build_qrcode_svg, parse_set_cookie_headers, CookieSet, LoginStatus, QrLoginPollOutcome,
        QrLoginPollResponse, QrLoginStartResponse,
    },
    error::{AppError, AppResult},
    models::{
        download::{DashStreams, DashTrack, DurlSegment, PlayUrlResponse},
        video::{AudioStreamOption, StreamOption, VideoData, VideoPage},
    },
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
        if let Some(page) = video.pages.first() {
            if let Ok(playurl) = self.playurl(bvid, page.cid, 80, cookies).await {
                video.apply_playurl_sizes(&playurl);
            }
        }
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

    pub async fn playurl(
        &self,
        bvid: &str,
        cid: u64,
        qn: u32,
        cookies: Option<&CookieSet>,
    ) -> AppResult<PlayUrlResponse> {
        let mut request = self
            .client
            .get(format!("{BASE_URL}/x/player/playurl"))
            .query(&[
                ("bvid", bvid.to_string()),
                ("cid", cid.to_string()),
                ("qn", qn.to_string()),
                ("platform", "web".to_string()),
                ("fnval", "4048".to_string()),
                ("fnver", "0".to_string()),
                ("fourk", "1".to_string()),
                ("force_host", "2".to_string()),
            ])
            .header(REFERER, format!("https://www.bilibili.com/video/{bvid}"));
        if let Some(cookies) = cookies {
            request = request.header(COOKIE, cookies.to_header());
        }

        let response_text = request.send().await?.error_for_status()?.text().await?;
        let response: ApiResponse<PlayUrlData> =
            serde_json::from_str(&response_text).map_err(|err| AppError::Io {
                message: format!("播放地址响应解析失败: {err}"),
            })?;

        if response.code != 0 {
            return Err(AppError::Api {
                code: response.code,
                message: response.message,
            });
        }

        let data = response.data.ok_or_else(|| AppError::Api {
            code: response.code,
            message: "B站响应缺少播放地址".to_string(),
        })?;

        let mut playurl = data.into_playurl_response();
        self.enrich_playurl_track_sizes(&mut playurl, bvid).await;
        Ok(playurl)
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

    async fn enrich_playurl_track_sizes(&self, playurl: &mut PlayUrlResponse, bvid: &str) {
        let Some(dash) = &mut playurl.dash else {
            return;
        };
        let referer = format!("https://www.bilibili.com/video/{bvid}");

        for track in dash.video.iter_mut().chain(dash.audio.iter_mut()) {
            if track.size_bytes.is_some() {
                continue;
            }

            let mut urls = std::iter::once(&track.base_url)
                .chain(track.backup_urls.iter())
                .filter(|url| !url.trim().is_empty());

            for url in &mut urls {
                if let Some(size) = self.probe_content_length(url, &referer).await {
                    track.size_bytes = Some(size);
                    break;
                }
            }
        }
    }

    async fn probe_content_length(&self, url: &str, referer: &str) -> Option<u64> {
        if let Ok(response) = self.client.head(url).header(REFERER, referer).send().await {
            if response.status().is_success() {
                if let Some(size) = header_content_length(response.headers()) {
                    return Some(size);
                }
            }
        }

        let response = self
            .client
            .get(url)
            .header(REFERER, referer)
            .header(RANGE, "bytes=0-0")
            .send()
            .await
            .ok()?;

        header_content_range_total(response.headers())
            .or_else(|| header_content_length(response.headers()))
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

fn header_content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
}

fn header_content_range_total(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_RANGE)?
        .to_str()
        .ok()?
        .rsplit('/')
        .next()?
        .parse::<u64>()
        .ok()
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
            author_avatar: self.owner.face,
            thumbnail: self.pic,
            views: format_count(self.stat.view),
            duration: format_duration(self.duration),
            duration_sec: self.duration,
            pages,
            qualities,
            streams,
            audio_streams: default_audio_stream_options(),
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

impl VideoData {
    fn apply_playurl_sizes(&mut self, playurl: &PlayUrlResponse) {
        if let Some(dash) = &playurl.dash {
            self.audio_streams = build_audio_stream_options(&dash.audio);

            for stream in &mut self.streams {
                let video_size = dash
                    .video
                    .iter()
                    .filter(|track| track.id == stream.qn)
                    .max_by_key(|track| (track.id, track.bandwidth.unwrap_or_default()))
                    .and_then(|track| track.size_bytes);
                let audio_size = dash
                    .audio
                    .iter()
                    .max_by_key(|track| track.bandwidth.unwrap_or_default())
                    .and_then(|track| track.size_bytes);
                stream.size_bytes = match (video_size, audio_size) {
                    (Some(video), Some(audio)) => Some(video.saturating_add(audio)),
                    (Some(video), None) => Some(video),
                    _ => None,
                };
            }
        } else {
            let durl_size = playurl.durl.iter().map(|segment| segment.size).sum::<u64>();
            if durl_size > 0 {
                for stream in &mut self.streams {
                    if stream.qn == playurl.quality {
                        stream.size_bytes = Some(durl_size);
                    }
                }
            }
        }
    }
}

fn default_audio_stream_options() -> Vec<AudioStreamOption> {
    [
        ("320kbps 高品质", 30280),
        ("192kbps 标准", 30232),
        ("128kbps 基础", 30216),
    ]
    .into_iter()
    .map(|(label, id)| AudioStreamOption {
        id: format!("audio_{id}"),
        label: label.to_string(),
        bandwidth: None,
        size_bytes: None,
        available: false,
    })
    .collect()
}

fn build_audio_stream_options(tracks: &[DashTrack]) -> Vec<AudioStreamOption> {
    let audio_tiers = [
        (30280, "320kbps 高品质"),
        (30232, "192kbps 标准"),
        (30216, "128kbps 基础"),
    ];

    audio_tiers
        .into_iter()
        .map(|(id, label)| {
            let track = tracks.iter().find(|track| track.id == id);
            AudioStreamOption {
                id: format!("audio_{id}"),
                label: label.to_string(),
                bandwidth: track.and_then(|track| track.bandwidth),
                size_bytes: track.and_then(|track| track.size_bytes),
                available: track.is_some(),
            }
        })
        .collect()
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
            size_bytes: None,
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
    #[serde(default)]
    face: String,
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
    face: String,
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
            avatar: self
                .is_login
                .then_some(self.face)
                .filter(|face| !face.is_empty()),
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

#[derive(Debug, Default, Deserialize)]
struct PlayUrlData {
    #[serde(default)]
    quality: u32,
    #[serde(default)]
    timelength: u64,
    #[serde(default)]
    accept_quality: Vec<u32>,
    dash: Option<DashData>,
    #[serde(default)]
    durl: Vec<DurlData>,
}

impl PlayUrlData {
    fn into_playurl_response(self) -> PlayUrlResponse {
        PlayUrlResponse {
            quality: self.quality,
            timelength: self.timelength,
            accept_quality: self.accept_quality,
            dash: self.dash.map(DashData::into_dash_streams),
            durl: self.durl.into_iter().map(DurlData::into_segment).collect(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct DashData {
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    video: Vec<DashTrackData>,
    #[serde(default)]
    audio: Vec<DashTrackData>,
}

impl DashData {
    fn into_dash_streams(self) -> DashStreams {
        DashStreams {
            duration: self.duration,
            video: self
                .video
                .into_iter()
                .filter_map(DashTrackData::into_track)
                .collect(),
            audio: self
                .audio
                .into_iter()
                .filter_map(DashTrackData::into_track)
                .collect(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct DashTrackData {
    #[serde(default)]
    id: u32,
    #[serde(default)]
    codecs: String,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(default, rename = "frameRate")]
    frame_rate_camel: Option<String>,
    frame_rate: Option<String>,
    bandwidth: Option<u64>,
    #[serde(default, rename = "mimeType")]
    mime_type_camel: Option<String>,
    mime_type: Option<String>,
    #[serde(default, rename = "baseUrl")]
    base_url_camel: String,
    #[serde(default)]
    base_url: String,
    #[serde(default, rename = "backupUrl")]
    backup_url_camel: Vec<String>,
    #[serde(default)]
    backup_url: Vec<String>,
    #[serde(default)]
    size: Option<u64>,
}

impl DashTrackData {
    fn into_track(self) -> Option<DashTrack> {
        let base_url = first_non_empty_string([self.base_url, self.base_url_camel]);
        (!base_url.trim().is_empty()).then_some(DashTrack {
            id: self.id,
            codecs: self.codecs,
            width: self.width,
            height: self.height,
            frame_rate: self.frame_rate.or(self.frame_rate_camel),
            bandwidth: self.bandwidth,
            size_bytes: self.size,
            mime_type: self.mime_type.or(self.mime_type_camel),
            base_url,
            backup_urls: merge_url_lists(self.backup_url, self.backup_url_camel),
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct DurlData {
    #[serde(default)]
    order: u32,
    #[serde(default)]
    length: u64,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "backupUrl")]
    backup_url_camel: Vec<String>,
    #[serde(default)]
    backup_url: Vec<String>,
}

impl DurlData {
    fn into_segment(self) -> DurlSegment {
        DurlSegment {
            order: self.order,
            length: self.length,
            size: self.size,
            url: self.url,
            backup_urls: merge_url_lists(self.backup_url, self.backup_url_camel),
        }
    }
}

fn first_non_empty_string(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

fn merge_url_lists(primary: Vec<String>, secondary: Vec<String>) -> Vec<String> {
    primary
        .into_iter()
        .chain(secondary)
        .filter(|url| !url.trim().is_empty())
        .fold(Vec::new(), |mut urls, url| {
            if !urls.contains(&url) {
                urls.push(url);
            }
            urls
        })
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

    #[tokio::test]
    async fn public_playurl_contract_returns_downloadable_streams_when_enabled() -> AppResult<()> {
        let Ok(bvid) = std::env::var("BILI_LIVE_PLAYURL_BVID") else {
            return Ok(());
        };

        let client = BilibiliClient::new()?;
        let video = client.video_info(&bvid, None).await?;
        let cid = video
            .pages
            .first()
            .expect("public video should have cid")
            .cid;
        let playurl = client.playurl(&bvid, cid, 16, None).await?;

        let dash_count = playurl
            .dash
            .as_ref()
            .map(|dash| dash.video.len() + dash.audio.len())
            .unwrap_or_default();
        assert!(dash_count > 0 || !playurl.durl.is_empty());
        Ok(())
    }

    #[test]
    fn view_data_maps_owner_face_to_author_avatar() {
        let raw = r#"{
            "bvid": "BV1xx411c7mD",
            "title": "demo",
            "owner": { "name": "UP主", "face": "https://i0.hdslb.com/bfs/face/demo.jpg" },
            "pic": "https://i0.hdslb.com/bfs/archive/demo.jpg",
            "stat": { "view": 1524000 },
            "duration": 188,
            "pages": [{ "cid": 1, "page": 1, "part": "P1" }],
            "rights": {},
            "desc": ""
        }"#;
        let data: ViewData = serde_json::from_str(raw).expect("view data should parse");
        let video = data.into_video_data();

        assert_eq!(video.author, "UP主");
        assert_eq!(
            video.author_avatar,
            "https://i0.hdslb.com/bfs/face/demo.jpg"
        );
    }

    #[test]
    fn nav_data_maps_face_to_login_avatar() {
        let raw = r#"{
            "isLogin": true,
            "uname": "潮汕英豪-胡培强",
            "face": "https://i0.hdslb.com/bfs/face/user.jpg",
            "mid": 42,
            "level_info": { "current_level": 5 },
            "vipType": 0
        }"#;
        let data: NavData = serde_json::from_str(raw).expect("nav data should parse");
        let status = data.into_login_status(String::new());

        assert!(status.is_login);
        assert_eq!(
            status.avatar.as_deref(),
            Some("https://i0.hdslb.com/bfs/face/user.jpg")
        );
    }

    #[test]
    fn playurl_sizes_map_to_stream_and_audio_options() {
        let mut video = VideoData {
            id: "BV1xx411c7mD".to_string(),
            title: "demo".to_string(),
            author: "UP主".to_string(),
            author_avatar: String::new(),
            thumbnail: String::new(),
            views: "1".to_string(),
            duration: "0:01".to_string(),
            duration_sec: 1,
            pages: vec![VideoPage {
                cid: 1,
                page: 1,
                part: "P1".to_string(),
            }],
            qualities: vec!["1080P".to_string()],
            streams: default_stream_options(false, false),
            audio_streams: default_audio_stream_options(),
            is_charging: Some(false),
            is_vip: Some(false),
            vip_type: None,
            is_login: None,
            login_name: None,
            login_level: None,
            desc: None,
            error: None,
        };
        let playurl = PlayUrlResponse {
            quality: 80,
            timelength: 1,
            accept_quality: vec![80],
            dash: Some(DashStreams {
                duration: 1,
                video: vec![DashTrack {
                    id: 80,
                    codecs: "avc1".to_string(),
                    width: Some(1920),
                    height: Some(1080),
                    frame_rate: Some("30".to_string()),
                    bandwidth: Some(2_000_000),
                    size_bytes: Some(20_000_000),
                    mime_type: Some("video/mp4".to_string()),
                    base_url: "https://example.test/video.m4s".to_string(),
                    backup_urls: vec![],
                }],
                audio: vec![DashTrack {
                    id: 30280,
                    codecs: "mp4a".to_string(),
                    width: None,
                    height: None,
                    frame_rate: None,
                    bandwidth: Some(320_000),
                    size_bytes: Some(3_200_000),
                    mime_type: Some("audio/mp4".to_string()),
                    base_url: "https://example.test/audio.m4s".to_string(),
                    backup_urls: vec![],
                }],
            }),
            durl: vec![],
        };

        video.apply_playurl_sizes(&playurl);

        let stream_1080p = video
            .streams
            .iter()
            .find(|stream| stream.qn == 80)
            .expect("1080P stream should exist");
        assert_eq!(stream_1080p.size_bytes, Some(23_200_000));
        assert_eq!(video.audio_streams[0].label, "320kbps 高品质");
        assert_eq!(video.audio_streams[0].size_bytes, Some(3_200_000));
        assert!(video.audio_streams[0].available);
        assert!(!video.audio_streams[1].available);
    }
}
