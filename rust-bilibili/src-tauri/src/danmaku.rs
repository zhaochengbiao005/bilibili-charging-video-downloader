use std::path::{Path, PathBuf};

use quick_xml::{events::Event, reader::Reader};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, REFERER, USER_AGENT};
use tokio::fs;

use crate::{
    auth::CookieSet,
    error::{AppError, AppResult},
};

const DANMAKU_URL: &str = "https://api.bilibili.com/x/v1/dm/list.so";

#[derive(Debug, Clone)]
pub struct DanmakuClient {
    client: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq)]
struct DanmakuEntry {
    start: f32,
    mode: DanmakuMode,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DanmakuMode {
    Scroll,
    Top,
    Bottom,
}

impl DanmakuClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn download_ass(
        &self,
        cid: u64,
        bvid: &str,
        output_path: &Path,
        cookies: Option<&CookieSet>,
    ) -> AppResult<PathBuf> {
        let xml = self.fetch_xml(cid, bvid, cookies).await?;
        let entries = parse_danmaku_xml(&xml)?;
        let ass = render_ass(&entries);
        let ass_path = output_path.with_extension("ass");
        if let Some(parent) = ass_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&ass_path, ass).await?;
        Ok(ass_path)
    }

    async fn fetch_xml(
        &self,
        cid: u64,
        bvid: &str,
        cookies: Option<&CookieSet>,
    ) -> AppResult<String> {
        let mut request = self
            .client
            .get(DANMAKU_URL)
            .query(&[("oid", cid.to_string())])
            .header(ACCEPT, "text/xml,application/xml;q=0.9,*/*;q=0.8")
            .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9")
            .header(
                USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
            )
            .header(REFERER, format!("https://www.bilibili.com/video/{bvid}"));
        if let Some(cookies) = cookies {
            request = request.header(COOKIE, cookies.to_header());
        }

        request
            .send()
            .await?
            .error_for_status()?
            .text()
            .await
            .map_err(AppError::from)
    }
}

fn parse_danmaku_xml(xml: &str) -> AppResult<Vec<DanmakuEntry>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut entries = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) if event.name().as_ref() == b"d" => {
                let mut meta = None;
                for attr in event.attributes().flatten() {
                    if attr.key.as_ref() == b"p" {
                        meta = Some(
                            attr.decode_and_unescape_value(reader.decoder())
                                .map_err(|err| AppError::Io {
                                    message: format!("弹幕元数据解析失败: {err}"),
                                })?
                                .into_owned(),
                        );
                        break;
                    }
                }

                let text = read_danmaku_text(&mut reader)?;

                if let Some(meta) = meta {
                    if let Some(entry) = parse_danmaku_entry(&meta, text) {
                        entries.push(entry);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(AppError::Io {
                    message: format!("弹幕 XML 解析失败: {err}"),
                });
            }
        }
    }

    Ok(entries)
}

fn read_danmaku_text(reader: &mut Reader<&[u8]>) -> AppResult<String> {
    let mut text_content = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Text(text)) => {
                let decoded = text.decode().map_err(|err| AppError::Io {
                    message: format!("弹幕内容解析失败: {err}"),
                })?;
                let unescaped =
                    quick_xml::escape::unescape(&decoded).map_err(|err| AppError::Io {
                        message: format!("弹幕内容解析失败: {err}"),
                    })?;
                text_content.push_str(&unescaped);
            }
            Ok(Event::CData(text)) => {
                let decoded = text.decode().map_err(|err| AppError::Io {
                    message: format!("弹幕内容解析失败: {err}"),
                })?;
                text_content.push_str(&decoded);
            }
            Ok(Event::GeneralRef(reference)) => {
                let decoded = reference.decode().map_err(|err| AppError::Io {
                    message: format!("弹幕内容解析失败: {err}"),
                })?;
                text_content.push_str(match decoded.as_ref() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    other => other,
                });
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"d" => break,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(AppError::Io {
                    message: format!("弹幕内容解析失败: {err}"),
                });
            }
        }
    }

    Ok(text_content)
}

fn parse_danmaku_entry(meta: &str, text: String) -> Option<DanmakuEntry> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let mut parts = meta.split(',');
    let start = parts.next()?.parse::<f32>().ok()?;
    let mode = match parts.next()?.parse::<u32>().ok()? {
        4 => DanmakuMode::Bottom,
        5 => DanmakuMode::Top,
        1 | 6 => DanmakuMode::Scroll,
        _ => DanmakuMode::Scroll,
    };

    Some(DanmakuEntry {
        start: start.max(0.0),
        mode,
        text: text.to_string(),
    })
}

fn render_ass(entries: &[DanmakuEntry]) -> String {
    let mut output = String::from(
        "[Script Info]\n\
        ScriptType: v4.00+\n\
        Collisions: Normal\n\
        PlayResX: 1920\n\
        PlayResY: 1080\n\
        Timer: 100.0000\n\
        WrapStyle: 2\n\
        ScaledBorderAndShadow: yes\n\n\
        [V4+ Styles]\n\
        Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
        Style: Scroll,Microsoft YaHei,48,&H00FFFFFF,&H00FFFFFF,&H8052A7FF,&H40000000,0,0,0,0,100,100,0,0,1,2,1,7,30,30,36,1\n\
        Style: Top,Microsoft YaHei,48,&H00FFFFFF,&H00FFFFFF,&H8052A7FF,&H40000000,0,0,0,0,100,100,0,0,1,2,1,8,30,30,40,1\n\
        Style: Bottom,Microsoft YaHei,48,&H00FFFFFF,&H00FFFFFF,&H8052A7FF,&H40000000,0,0,0,0,100,100,0,0,1,2,1,2,30,30,48,1\n\n\
        [Events]\n\
        Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for (index, entry) in entries.iter().enumerate() {
        let start = entry.start;
        let end = start + 5.0;
        let text = escape_ass_text(&entry.text);
        let (style, effect) = match entry.mode {
            DanmakuMode::Scroll => {
                let y = 48 + ((index % 15) as i32 * 58);
                (
                    "Scroll",
                    format!("{{\\move(1920,{y},-{},{y})}}", text_width_hint(&entry.text)),
                )
            }
            DanmakuMode::Top => ("Top", String::new()),
            DanmakuMode::Bottom => ("Bottom", String::new()),
        };
        output.push_str(&format!(
            "Dialogue: 0,{},{},{style},,0,0,0,,{effect}{text}\n",
            ass_time(start),
            ass_time(end),
        ));
    }

    output
}

fn text_width_hint(text: &str) -> i32 {
    80 + text.chars().count().min(80) as i32 * 28
}

fn ass_time(seconds: f32) -> String {
    let centiseconds = (seconds.max(0.0) * 100.0).round() as u64;
    let cs = centiseconds % 100;
    let total_seconds = centiseconds / 100;
    let secs = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = total_seconds / 3600;
    format!("{hours}:{minutes:02}:{secs:02}.{cs:02}")
}

fn escape_ass_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('\r', "")
        .replace('\n', "\\N")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_xml_and_renders_ass_dialogues() {
        let xml = r#"<i><d p="1.25,1,25,16777215,1,0,0,0">滚动 &amp; 弹幕</d><d p="3,5,25,16777215,1,0,0,0">顶部</d></i>"#;
        let entries = parse_danmaku_xml(xml).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].start, 1.25);
        assert_eq!(entries[0].mode, DanmakuMode::Scroll);
        assert_eq!(entries[0].text, "滚动 & 弹幕");
        assert_eq!(entries[1].mode, DanmakuMode::Top);

        let ass = render_ass(&entries);
        assert!(ass.contains("[Events]"));
        assert!(ass.contains("Dialogue: 0,0:00:01.25,0:00:06.25,Scroll"));
        assert!(ass.contains("滚动 & 弹幕"));
        assert!(ass.contains("Dialogue: 0,0:00:03.00,0:00:08.00,Top"));
    }
}
