use std::time::Duration;

use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, COOKIE, ORIGIN, RANGE, REFERER, SET_COOKIE,
    USER_AGENT,
};
use serde::Deserialize;

use crate::{
    auth::{
        build_qrcode_svg, parse_set_cookie_headers, CookieSet, LoginStatus, QrLoginPollOutcome,
        QrLoginPollResponse, QrLoginStartResponse,
    },
    download::bili_http::{header_content_length, header_content_range_total},
    download::{is_dolby_track, is_hdr_track},
    error::{AppError, AppResult},
    models::{
        download::{DashStreams, DashTrack, DurlSegment, PlayUrlResponse},
        stein::{SteinChoice, SteinGraph, SteinNode, SteinSegment},
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
        select_video_by_bvid(self.video_info_list(bvid, cookies).await?, bvid).ok_or_else(|| {
            AppError::Api {
                code: -1,
                message: "B站响应缺少视频信息".to_string(),
            }
        })
    }

    pub async fn video_info_list(
        &self,
        bvid: &str,
        cookies: Option<&CookieSet>,
    ) -> AppResult<Vec<VideoData>> {
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

        let mut videos = data.into_video_data_list();
        for (index, video) in videos.iter_mut().enumerate() {
            if let Some(page) = video.pages.first() {
                if index == 0 {
                    if video.is_stein == Some(true) {
                        match self
                            .crawl_stein_graph(&video.id, page.cid, cookies)
                            .await
                        {
                            Ok(graph) => {
                                video.access_message = Some(format!(
                                    "互动视频：共 {} 个分片、{} 个剧情节点。可整图下载或按路径拼接。",
                                    graph.segment_count,
                                    graph.nodes.len()
                                ));
                                video.stein_graph = Some(graph);
                            }
                            Err(err) => {
                                video.access_message = Some(format!(
                                    "互动视频解析剧情图失败：{err}。仍可尝试下载入口分片。"
                                ));
                            }
                        }
                    }
                    if let Ok(playurl) = self.playurl(&video.id, page.cid, 127, cookies).await {
                        // 互动视频入口时长很短，不要用 preview 规则误伤；仅非 stein 应用 playurl 尺寸
                        if video.is_stein != Some(true) {
                            video.apply_playurl_sizes(&playurl);
                        } else {
                            // 仍更新画质列表尺寸（用入口流），但不触发试看拦截标记
                            let is_preview = video.is_preview;
                            let access = video.access_message.clone();
                            video.apply_playurl_sizes(&playurl);
                            video.is_preview = is_preview;
                            if let Some(msg) = access {
                                video.access_message = Some(msg);
                            }
                        }
                    }
                }
            }
            if let Some(login) = &login {
                video.is_login = Some(login.is_login);
                video.login_name = login.username.clone();
                video.login_level = login.level;
            }
        }
        Ok(videos)
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

    /// 爬取互动视频剧情图：BFS edgeinfo_v2，收集全部 cid 与选项边。
    pub async fn crawl_stein_graph(
        &self,
        bvid: &str,
        entry_cid: u64,
        cookies: Option<&CookieSet>,
    ) -> AppResult<SteinGraph> {
        let graph_version = self.player_graph_version(bvid, entry_cid, cookies).await?;
        let mut visited_edges = std::collections::HashSet::new();
        let mut queue: std::collections::VecDeque<Option<u64>> =
            std::collections::VecDeque::from([None]);
        let mut nodes: Vec<SteinNode> = Vec::new();
        let mut segment_map: std::collections::BTreeMap<u64, SteinSegment> =
            std::collections::BTreeMap::new();
        // edge_id → 到达该节点时播放的 cid
        let mut play_cid_by_edge: std::collections::HashMap<u64, u64> =
            std::collections::HashMap::new();
        let mut entry_edge_id = 1_u64;

        // 入口分片
        segment_map.insert(
            entry_cid,
            SteinSegment {
                cid: entry_cid,
                title: "入口".to_string(),
                edge_id: 0,
            },
        );

        const MAX_EDGES: usize = 300;
        while let Some(edge_id) = queue.pop_front() {
            if nodes.len() >= MAX_EDGES {
                break;
            }
            let key = edge_id.unwrap_or(0);
            if !visited_edges.insert(key) {
                continue;
            }

            let info = self
                .edgeinfo_v2(bvid, graph_version, edge_id, cookies)
                .await?;
            let eid = info.edge_id.unwrap_or(key);
            if edge_id.is_none() {
                entry_edge_id = if eid == 0 { 1 } else { eid };
                play_cid_by_edge.insert(entry_edge_id, entry_cid);
            }
            let title = info.title.unwrap_or_else(|| format!("节点{eid}"));
            let is_leaf = info.is_leaf.unwrap_or(0) == 1;

            // 选项出现时机
            let (start_time_r_ms, pause_video) = first_question_timing(&info.edges);

            for video in info
                .preload
                .as_ref()
                .and_then(|p| p.video.as_ref())
                .into_iter()
                .flatten()
            {
                if video.cid != 0 {
                    segment_map.entry(video.cid).or_insert_with(|| SteinSegment {
                        cid: video.cid,
                        title: title.clone(),
                        edge_id: eid,
                    });
                }
            }

            // story_list 里的当前节点 cid 也可作为本 edge 播放源
            if let Some(story) = info.story_list.as_ref() {
                for s in story {
                    if s.is_current.unwrap_or(0) == 1 && s.cid.unwrap_or(0) != 0 {
                        play_cid_by_edge.entry(eid).or_insert(s.cid.unwrap_or(0));
                        segment_map
                            .entry(s.cid.unwrap_or(0))
                            .or_insert_with(|| SteinSegment {
                                cid: s.cid.unwrap_or(0),
                                title: title.clone(),
                                edge_id: eid,
                            });
                    }
                }
            }

            let mut choices = Vec::new();
            for choice in flatten_stein_choices(&info.edges) {
                let next_edge = choice.id.unwrap_or(0);
                let cid = choice.cid.unwrap_or(0);
                let option = choice
                    .option
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| format!("选项{next_edge}"));
                if cid != 0 {
                    segment_map.entry(cid).or_insert_with(|| SteinSegment {
                        cid,
                        title: option.clone(),
                        edge_id: next_edge,
                    });
                    if next_edge != 0 {
                        // 选择后跳转到 next_edge 时播放该 cid
                        play_cid_by_edge.entry(next_edge).or_insert(cid);
                    }
                }
                if next_edge != 0 {
                    choices.push(SteinChoice {
                        edge_id: next_edge,
                        option,
                        cid,
                    });
                    if !visited_edges.contains(&next_edge) {
                        queue.push_back(Some(next_edge));
                    }
                }
            }

            // 更新入口标题
            if edge_id.is_none() {
                if let Some(seg) = segment_map.get_mut(&entry_cid) {
                    seg.title = title.clone();
                    seg.edge_id = eid;
                }
            }

            let play_cid = play_cid_by_edge.get(&eid).copied().unwrap_or(0);
            nodes.push(SteinNode {
                edge_id: eid,
                title,
                is_leaf,
                play_cid,
                start_time_r_ms,
                pause_video,
                choices,
            });
        }

        // 确保 entry_edge_id 在 nodes 中
        if entry_edge_id == 0 {
            entry_edge_id = nodes.first().map(|n| n.edge_id).unwrap_or(1);
        }
        // 补全 play_cid
        for node in &mut nodes {
            if node.play_cid == 0 {
                if node.edge_id == entry_edge_id {
                    node.play_cid = entry_cid;
                } else if let Some(cid) = play_cid_by_edge.get(&node.edge_id) {
                    node.play_cid = *cid;
                }
            }
        }

        let segments: Vec<SteinSegment> = segment_map.into_values().collect();
        let segment_count = segments.len() as u32;
        Ok(SteinGraph {
            graph_version,
            entry_edge_id,
            entry_cid,
            segment_count,
            segments,
            nodes,
        })
    }

    async fn player_graph_version(
        &self,
        bvid: &str,
        cid: u64,
        cookies: Option<&CookieSet>,
    ) -> AppResult<u64> {
        let mut request = self
            .client
            .get(format!("{BASE_URL}/x/player/v2"))
            .query(&[("bvid", bvid.to_string()), ("cid", cid.to_string())])
            .header(REFERER, format!("https://www.bilibili.com/video/{bvid}"));
        if let Some(cookies) = cookies {
            request = request.header(COOKIE, cookies.to_header());
        }
        let response = request
            .send()
            .await?
            .error_for_status()?
            .json::<ApiResponse<PlayerV2Data>>()
            .await?;
        if response.code != 0 {
            return Err(AppError::Api {
                code: response.code,
                message: response.message,
            });
        }
        let data = response.data.ok_or_else(|| AppError::Api {
            code: -1,
            message: "播放器响应缺少 data".to_string(),
        })?;
        data.interaction
            .and_then(|i| i.graph_version)
            .ok_or_else(|| AppError::Api {
                code: -1,
                message: "互动视频缺少 graph_version".to_string(),
            })
    }

    async fn edgeinfo_v2(
        &self,
        bvid: &str,
        graph_version: u64,
        edge_id: Option<u64>,
        cookies: Option<&CookieSet>,
    ) -> AppResult<SteinEdgeInfoData> {
        let mut params = vec![
            ("bvid".to_string(), bvid.to_string()),
            ("graph_version".to_string(), graph_version.to_string()),
            ("portal".to_string(), "0".to_string()),
            ("screen".to_string(), "0".to_string()),
        ];
        if let Some(edge_id) = edge_id {
            params.push(("edge_id".to_string(), edge_id.to_string()));
        }
        let mut request = self
            .client
            .get(format!("{BASE_URL}/x/stein/edgeinfo_v2"))
            .query(&params)
            .header(REFERER, format!("https://www.bilibili.com/video/{bvid}"));
        if let Some(cookies) = cookies {
            request = request.header(COOKIE, cookies.to_header());
        }
        let response = request
            .send()
            .await?
            .error_for_status()?
            .json::<ApiResponse<SteinEdgeInfoData>>()
            .await?;
        if response.code != 0 {
            return Err(AppError::Api {
                code: response.code,
                message: response.message,
            });
        }
        response.data.ok_or_else(|| AppError::Api {
            code: -1,
            message: "剧情图响应缺少 data".to_string(),
        })
    }

    pub async fn enrich_video_sizes(
        &self,
        mut video: VideoData,
        cid: Option<u64>,
        cookies: Option<&CookieSet>,
    ) -> AppResult<VideoData> {
        let cid = cid
            .or_else(|| video.pages.first().map(|page| page.cid))
            .ok_or_else(|| AppError::InvalidInput {
                message: "视频缺少分 P 信息，无法获取容量".to_string(),
            })?;
        let playurl = self.playurl(&video.id, cid, 127, cookies).await?;
        video.apply_playurl_sizes(&playurl);
        Ok(video)
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
        HeaderValue::from_static(crate::download::bili_http::BILIBILI_UA),
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
    ugc_season: Option<UgcSeason>,
    #[serde(default)]
    rights: Rights,
    #[serde(default)]
    desc: String,
    /// 高档充电 / UPower 专属稿
    #[serde(default)]
    is_upower_exclusive: bool,
    /// 当前账号是否可完整播放
    #[serde(default)]
    is_upower_play: bool,
    /// 当前是否处于试看态
    #[serde(default)]
    is_upower_preview: bool,
}

impl ViewData {
    fn into_video_data_list(self) -> Vec<VideoData> {
        let is_charging =
            self.rights.elec_high == 1 || self.is_upower_exclusive || self.is_upower_preview;
        let is_vip = self.rights.vip_free == 1;
        let is_stein = self.rights.is_stein_gate == 1;
        let is_upower_play = self.is_upower_play;
        let access_message = if is_stein {
            Some("互动视频：将解析剧情图并支持整图/路径下载。".to_string())
        } else if is_charging && !is_upower_play {
            Some(
                "该视频为充电专属内容，当前账号仅可试看。请登录并对该 UP 开通对应档位包月充电后再下载完整版。"
                    .to_string(),
            )
        } else {
            None
        };
        let streams = default_stream_options(is_charging, is_vip);
        let qualities = streams
            .iter()
            .filter(|stream| stream.available)
            .map(|stream| stream.label.clone())
            .collect();
        let pages: Vec<VideoPage> = self
            .pages
            .into_iter()
            .map(|page| VideoPage {
                cid: page.cid,
                page: page.page,
                part: page.part,
                duration: format_duration(page.duration),
                duration_sec: page.duration,
                thumbnail: if page.first_frame.trim().is_empty() {
                    None
                } else {
                    Some(page.first_frame)
                },
            })
            .collect();

        let base_video = VideoData {
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
            is_upower_play: Some(is_upower_play),
            is_preview: Some(self.is_upower_preview && !is_upower_play),
            is_vip: Some(is_vip),
            vip_type: None,
            is_login: None,
            login_name: None,
            login_level: None,
            desc: Some(self.desc),
            access_message,
            is_stein: Some(is_stein),
            stein_graph: None,
            error: None,
        };

        let Some(ugc_season) = self.ugc_season else {
            return vec![base_video];
        };

        let mut videos = ugc_season.into_video_data_list(&base_video, is_charging, is_vip);
        if videos.is_empty() {
            vec![base_video]
        } else {
            if let Some(existing) = videos.iter_mut().find(|video| video.id == base_video.id) {
                *existing = base_video;
            } else {
                videos.insert(0, base_video);
            }
            dedupe_videos_by_bvid(videos)
        }
    }
}

fn select_video_by_bvid(videos: Vec<VideoData>, bvid: &str) -> Option<VideoData> {
    if let Some(index) = videos
        .iter()
        .position(|video| video.id.eq_ignore_ascii_case(bvid))
    {
        return videos.get(index).cloned();
    }
    videos.into_iter().next()
}

fn dedupe_videos_by_bvid(videos: Vec<VideoData>) -> Vec<VideoData> {
    videos.into_iter().fold(Vec::new(), |mut unique, video| {
        if !unique.iter().any(|item: &VideoData| item.id == video.id) {
            unique.push(video);
        }
        unique
    })
}

impl VideoData {
    fn apply_playurl_sizes(&mut self, playurl: &PlayUrlResponse) {
        let meta_sec = self
            .pages
            .first()
            .map(|page| page.duration_sec)
            .filter(|sec| *sec > 0)
            .unwrap_or(self.duration_sec);
        if playurl.is_preview_stream(meta_sec) {
            self.is_preview = Some(true);
            self.is_charging = Some(true);
            let message = playurl.preview_block_message(meta_sec);
            self.access_message = Some(message);
        } else if self.is_preview == Some(true) {
            // playurl 已给出全片，清除解析阶段的试看标记
            self.is_preview = Some(false);
            if self.is_upower_play != Some(false) {
                self.access_message = None;
            }
        }

        if let Some(dash) = &playurl.dash {
            self.audio_streams = build_audio_stream_options(&dash.audio);
            self.streams = build_stream_options_from_dash(
                &dash.video,
                &dash.audio,
                self.is_charging.unwrap_or(false),
                self.is_vip.unwrap_or(false),
            );
            self.qualities = self
                .streams
                .iter()
                .filter(|stream| stream.available)
                .map(|stream| stream.label.clone())
                .collect();
            if !self.qualities.is_empty() {
                return;
            }

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

fn build_stream_options_from_dash(
    video_tracks: &[DashTrack],
    audio_tracks: &[DashTrack],
    is_charging: bool,
    is_vip: bool,
) -> Vec<StreamOption> {
    let restricted = is_charging || is_vip;
    let best_audio_size = audio_tracks
        .iter()
        .max_by_key(|track| track.bandwidth.unwrap_or_default())
        .and_then(|track| track.size_bytes)
        .unwrap_or_default();
    let mut streams = Vec::new();

    for &(qn, base_label, maybe_login, maybe_vip) in VIDEO_QUALITY_TIERS {
        let best_track = video_tracks
            .iter()
            .filter(|track| track.id == qn)
            .max_by_key(|track| {
                (
                    track.bandwidth.unwrap_or_default(),
                    dolby_track_score(track),
                    hdr_track_score(track),
                )
            });

        if let Some(track) = best_track {
            let mut variants = Vec::new();
            if is_dolby_track(track) {
                variants.push("杜比视界");
            }
            if is_hdr_track(track) {
                variants.push("HDR");
            }
            let label = if variants.is_empty() {
                base_label.to_string()
            } else {
                format!("{base_label} {}", variants.join(" "))
            };
            let video_size = track.size_bytes.unwrap_or_default();
            let size_bytes = match (video_size, best_audio_size) {
                (0, 0) => None,
                (video, 0) => Some(video),
                (0, audio) => Some(audio),
                (video, audio) => Some(video.saturating_add(audio)),
            };
            streams.push(StreamOption {
                id: format!("video_{}_{}", qn, stream_variant_id(track)),
                qn,
                label,
                codec: Some(track.codecs.clone()),
                width: track.width,
                height: track.height,
                frame_rate: track.frame_rate.clone(),
                bandwidth: track.bandwidth,
                size_bytes,
                requires_login: maybe_login && restricted,
                requires_vip: maybe_vip && is_vip,
                available: true,
                unavailable_reason: None,
            });
        } else {
            let requires_vip = maybe_vip && is_vip;
            streams.push(StreamOption {
                id: format!("video_{qn}"),
                qn,
                label: base_label.to_string(),
                codec: None,
                width: None,
                height: None,
                frame_rate: None,
                bandwidth: None,
                size_bytes: None,
                requires_login: maybe_login && restricted,
                requires_vip,
                available: false,
                unavailable_reason: Some(if requires_vip {
                    "需要大会员权限".to_string()
                } else {
                    "当前视频无此画质".to_string()
                }),
            });
        }
    }

    streams
}

fn default_stream_options(is_charging: bool, is_vip: bool) -> Vec<StreamOption> {
    let restricted = is_charging || is_vip;
    VIDEO_QUALITY_TIERS
        .iter()
        .map(|&(qn, label, maybe_login, maybe_vip)| {
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

const VIDEO_QUALITY_TIERS: &[(u32, &str, bool, bool)] = &[
    (127, "8K", true, true),
    (126, "杜比视界", true, true),
    (125, "HDR", true, true),
    (120, "4K", true, true),
    (116, "1080P60", true, false),
    (112, "1080P+", true, false),
    (74, "720P60", true, false),
    (80, "1080P", true, false),
    (64, "720P", false, false),
    (32, "480P", false, false),
    (16, "360P", false, false),
];

fn dolby_track_score(track: &DashTrack) -> u8 {
    u8::from(is_dolby_track(track))
}

fn hdr_track_score(track: &DashTrack) -> u8 {
    u8::from(is_hdr_track(track))
}

fn stream_variant_id(track: &DashTrack) -> &'static str {
    if is_dolby_track(track) {
        "dolby"
    } else if is_hdr_track(track) {
        "hdr"
    } else {
        "std"
    }
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
    #[serde(default)]
    is_stein_gate: u32,
}

#[derive(Debug, Deserialize)]
struct ViewPage {
    cid: u64,
    page: u32,
    part: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    first_frame: String,
}

#[derive(Debug, Default, Deserialize)]
struct UgcSeason {
    #[serde(default)]
    sections: Vec<UgcSection>,
}

impl UgcSeason {
    fn into_video_data_list(
        self,
        base: &VideoData,
        is_charging: bool,
        is_vip: bool,
    ) -> Vec<VideoData> {
        self.sections
            .into_iter()
            .flat_map(|section| section.episodes)
            .filter_map(|episode| episode.into_video_data(base, is_charging, is_vip))
            .collect()
    }
}

#[derive(Debug, Default, Deserialize)]
struct UgcSection {
    #[serde(default)]
    episodes: Vec<UgcEpisode>,
}

#[derive(Debug, Default, Deserialize)]
struct UgcEpisode {
    #[serde(default)]
    bvid: String,
    #[serde(default)]
    cid: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    arc: Option<UgcArc>,
}

impl UgcEpisode {
    fn into_video_data(
        self,
        base: &VideoData,
        is_charging: bool,
        is_vip: bool,
    ) -> Option<VideoData> {
        let bvid = if self.bvid.trim().is_empty() {
            self.arc.as_ref()?.bvid.clone()
        } else {
            self.bvid
        };
        if bvid.trim().is_empty() {
            return None;
        }

        let arc = self.arc.unwrap_or_default();
        let cid = if self.cid != 0 { self.cid } else { arc.cid };
        if cid == 0 {
            return None;
        }

        let title = first_non_empty_string([self.title, arc.title, base.title.clone()]);
        let duration_sec = arc.duration;
        let thumbnail = first_non_empty_string([arc.pic, base.thumbnail.clone()]);
        let owner_name = first_non_empty_string([arc.author.name, base.author.clone()]);
        let owner_face = first_non_empty_string([arc.author.face, base.author_avatar.clone()]);
        let streams = default_stream_options(is_charging, is_vip);
        let qualities = streams
            .iter()
            .filter(|stream| stream.available)
            .map(|stream| stream.label.clone())
            .collect();

        Some(VideoData {
            id: bvid,
            title: title.clone(),
            author: owner_name,
            author_avatar: owner_face,
            thumbnail: thumbnail.clone(),
            views: format_count(arc.stat.view),
            duration: format_duration(duration_sec),
            duration_sec,
            pages: vec![VideoPage {
                cid,
                page: 1,
                part: title,
                duration: format_duration(duration_sec),
                duration_sec,
                thumbnail: if thumbnail.trim().is_empty() {
                    None
                } else {
                    Some(thumbnail)
                },
            }],
            qualities,
            streams,
            audio_streams: default_audio_stream_options(),
            is_charging: Some(is_charging),
            is_upower_play: base.is_upower_play,
            is_preview: base.is_preview,
            is_vip: Some(is_vip),
            vip_type: base.vip_type,
            is_login: base.is_login,
            login_name: base.login_name.clone(),
            login_level: base.login_level,
            desc: Some(first_non_empty_string([
                arc.desc,
                base.desc.clone().unwrap_or_default(),
            ])),
            access_message: base.access_message.clone(),
            is_stein: base.is_stein,
            stein_graph: None,
            error: None,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct UgcArc {
    #[serde(default)]
    bvid: String,
    #[serde(default)]
    cid: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    pic: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    author: Owner,
    #[serde(default)]
    stat: Stat,
    #[serde(default)]
    desc: String,
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
struct PlayerV2Data {
    #[serde(default)]
    interaction: Option<PlayerInteraction>,
}

#[derive(Debug, Default, Deserialize)]
struct PlayerInteraction {
    #[serde(default)]
    graph_version: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct SteinEdgeInfoData {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    edge_id: Option<u64>,
    #[serde(default)]
    is_leaf: Option<u32>,
    #[serde(default)]
    edges: Option<SteinEdges>,
    #[serde(default)]
    preload: Option<SteinPreload>,
    #[serde(default)]
    story_list: Option<Vec<SteinStoryItem>>,
}

#[derive(Debug, Default, Deserialize)]
struct SteinStoryItem {
    #[serde(default)]
    cid: Option<u64>,
    #[serde(default)]
    is_current: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct SteinEdges {
    #[serde(default)]
    choices: Option<Vec<SteinChoiceData>>,
    #[serde(default)]
    questions: Option<Vec<SteinQuestion>>,
}

#[derive(Debug, Default, Deserialize)]
struct SteinQuestion {
    #[serde(default)]
    choices: Option<Vec<SteinChoiceData>>,
    #[serde(default)]
    start_time_r: Option<u64>,
    #[serde(default)]
    pause_video: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct SteinChoiceData {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    option: Option<String>,
    #[serde(default)]
    cid: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct SteinPreload {
    #[serde(default)]
    video: Option<Vec<SteinPreloadVideo>>,
}

#[derive(Debug, Default, Deserialize)]
struct SteinPreloadVideo {
    #[serde(default)]
    cid: u64,
}

fn flatten_stein_choices(edges: &Option<SteinEdges>) -> Vec<&SteinChoiceData> {
    let Some(edges) = edges else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(choices) = &edges.choices {
        out.extend(choices.iter());
    }
    if let Some(questions) = &edges.questions {
        for q in questions {
            if let Some(choices) = &q.choices {
                out.extend(choices.iter());
            }
        }
    }
    out
}

fn first_question_timing(edges: &Option<SteinEdges>) -> (u64, bool) {
    let Some(edges) = edges else {
        return (800, true);
    };
    let Some(questions) = &edges.questions else {
        return (800, true);
    };
    let Some(q) = questions.first() else {
        return (800, true);
    };
    let start = q.start_time_r.unwrap_or(800);
    // B站 start_time_r 多为毫秒；若值很小（<=30）按秒理解
    let start_ms = if start > 0 && start <= 30 {
        start * 1000
    } else {
        start
    };
    let pause = q.pause_video.unwrap_or(1) != 0;
    (start_ms, pause)
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
        let video = data
            .into_video_data_list()
            .into_iter()
            .next()
            .expect("view data should map to one video");

        assert_eq!(video.author, "UP主");
        assert_eq!(
            video.author_avatar,
            "https://i0.hdslb.com/bfs/face/demo.jpg"
        );
    }

    #[test]
    fn view_data_expands_ugc_season_episodes() {
        let raw = r#"{
            "bvid": "BVcurrent111",
            "title": "当前视频",
            "owner": { "name": "UP主", "face": "https://i0.hdslb.com/bfs/face/up.jpg" },
            "pic": "https://i0.hdslb.com/bfs/archive/current.jpg",
            "stat": { "view": 100 },
            "duration": 188,
            "pages": [{ "cid": 11, "page": 1, "part": "当前P" }],
            "rights": {},
            "desc": "合集简介",
            "ugc_season": {
                "sections": [{
                    "episodes": [
                        {
                            "bvid": "BVcurrent111",
                            "cid": 11,
                            "title": "当前视频",
                            "arc": {
                                "bvid": "BVcurrent111",
                                "cid": 11,
                                "title": "当前视频",
                                "pic": "https://i0.hdslb.com/bfs/archive/current.jpg",
                                "duration": 188,
                                "author": { "name": "UP主", "face": "https://i0.hdslb.com/bfs/face/up.jpg" },
                                "stat": { "view": 100 },
                                "desc": "当前简介"
                            }
                        },
                        {
                            "bvid": "BVnext22222",
                            "cid": 22,
                            "title": "合集第二集",
                            "arc": {
                                "bvid": "BVnext22222",
                                "cid": 22,
                                "title": "合集第二集",
                                "pic": "https://i0.hdslb.com/bfs/archive/next.jpg",
                                "duration": 567,
                                "author": { "name": "UP主2", "face": "https://i0.hdslb.com/bfs/face/up2.jpg" },
                                "stat": { "view": 200 },
                                "desc": "第二集简介"
                            }
                        }
                    ]
                }]
            }
        }"#;
        let data: ViewData = serde_json::from_str(raw).expect("view data should parse");
        let videos = data.into_video_data_list();

        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].id, "BVcurrent111");
        assert_eq!(videos[1].id, "BVnext22222");
        assert_eq!(videos[1].pages[0].cid, 22);
        assert_eq!(
            videos[1].pages[0].thumbnail.as_deref(),
            Some("https://i0.hdslb.com/bfs/archive/next.jpg")
        );
    }

    #[test]
    fn video_info_prefers_base_video_when_ugc_season_contains_same_bvid() {
        let raw = r#"{
            "bvid": "BVcurrent111",
            "title": "当前视频",
            "owner": { "name": "UP主", "face": "https://i0.hdslb.com/bfs/face/up.jpg" },
            "pic": "https://i0.hdslb.com/bfs/archive/current.jpg",
            "stat": { "view": 100 },
            "duration": 188,
            "pages": [
                { "cid": 11, "page": 1, "part": "P1" },
                { "cid": 12, "page": 2, "part": "P2" }
            ],
            "rights": {},
            "desc": "合集简介",
            "ugc_season": {
                "sections": [{
                    "episodes": [{
                        "bvid": "BVcurrent111",
                        "cid": 11,
                        "title": "当前视频合集条目",
                        "arc": {
                            "bvid": "BVcurrent111",
                            "cid": 11,
                            "title": "当前视频合集条目",
                            "duration": 188
                        }
                    }]
                }]
            }
        }"#;
        let data: ViewData = serde_json::from_str(raw).expect("view data should parse");
        let videos = data.into_video_data_list();

        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].id, "BVcurrent111");
        assert_eq!(videos[0].title, "当前视频");
        assert_eq!(videos[0].pages.len(), 2);
        assert_eq!(videos[0].pages[1].cid, 12);
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
                duration: "0:01".to_string(),
                duration_sec: 1,
                thumbnail: None,
            }],
            qualities: vec!["1080P".to_string()],
            streams: default_stream_options(false, false),
            audio_streams: default_audio_stream_options(),
            is_charging: Some(false),
            is_upower_play: Some(true),
            is_preview: Some(false),
            is_vip: Some(false),
            vip_type: None,
            is_login: None,
            login_name: None,
            login_level: None,
            desc: None,
            access_message: None,
            is_stein: Some(false),
            stein_graph: None,
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

    #[test]
    fn dash_stream_options_include_8k_and_dolby_labels() {
        let video_tracks = vec![
            DashTrack {
                id: 127,
                codecs: "avc1.640034".to_string(),
                width: Some(7680),
                height: Some(4320),
                frame_rate: Some("60".to_string()),
                bandwidth: Some(50_000_000),
                size_bytes: Some(500_000_000),
                mime_type: Some("video/mp4".to_string()),
                base_url: "https://example.test/8k.m4s".to_string(),
                backup_urls: vec![],
            },
            DashTrack {
                id: 126,
                codecs: "dvh1.08.07".to_string(),
                width: Some(3840),
                height: Some(2160),
                frame_rate: Some("60".to_string()),
                bandwidth: Some(35_000_000),
                size_bytes: Some(350_000_000),
                mime_type: Some("video/mp4".to_string()),
                base_url: "https://example.test/dolby.m4s".to_string(),
                backup_urls: vec![],
            },
        ];
        let audio_tracks = vec![DashTrack {
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
        }];

        let streams = build_stream_options_from_dash(&video_tracks, &audio_tracks, false, false);

        assert!(streams.iter().any(|stream| stream.label == "8K"));
        assert!(streams
            .iter()
            .any(|stream| stream.label.contains("杜比视界")));
    }
}
