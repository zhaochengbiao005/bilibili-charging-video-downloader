use std::{
    io::Read,
    path::{Path, PathBuf},
};

use flate2::read::{DeflateDecoder, ZlibDecoder};
use quick_xml::{events::Event, reader::Reader};
use reqwest::{
    header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, COOKIE, ORIGIN, REFERER, USER_AGENT},
    Response,
};
use tokio::fs;

use crate::{
    auth::CookieSet,
    error::{AppError, AppResult},
};

const DANMAKU_URL: &str = "https://api.bilibili.com/x/v1/dm/list.so";
const DANMAKU_COMMENT_URL: &str = "https://comment.bilibili.com";

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
        let ass = self.render_ass_text(cid, bvid, cookies).await?;
        let ass_path = output_path.with_extension("ass");
        if let Some(parent) = ass_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&ass_path, ass).await?;
        Ok(ass_path)
    }

    pub async fn render_ass_text(
        &self,
        cid: u64,
        bvid: &str,
        cookies: Option<&CookieSet>,
    ) -> AppResult<String> {
        let mut xml = self.fetch_xml(cid, bvid, cookies).await?;
        let mut entries = parse_danmaku_xml(&xml);
        if entries.is_empty() {
            xml = self.fetch_comment_xml(cid, bvid, cookies).await?;
            entries = parse_danmaku_xml(&xml);
        }
        if entries.is_empty() {
            return Err(AppError::Io {
                message: "没有获取到可显示的弹幕内容".to_string(),
            });
        }
        Ok(render_ass(&entries))
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
            .header(ACCEPT, "application/xml,text/xml,*/*;q=0.8")
            .header(ACCEPT_ENCODING, "gzip, br")
            .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8")
            .header(ORIGIN, "https://www.bilibili.com")
            .header(
                USER_AGENT,
                crate::download::bili_http::BILIBILI_UA,
            )
            .header(REFERER, format!("https://www.bilibili.com/video/{bvid}"));
        if let Some(cookies) = cookies {
            request = request.header(COOKIE, cookies.to_header());
        }

        response_text(request.send().await?.error_for_status()?).await
    }

    async fn fetch_comment_xml(
        &self,
        cid: u64,
        bvid: &str,
        cookies: Option<&CookieSet>,
    ) -> AppResult<String> {
        let mut request = self
            .client
            .get(format!("{DANMAKU_COMMENT_URL}/{cid}.xml"))
            .header(ACCEPT, "application/xml,text/xml,*/*;q=0.8")
            .header(ACCEPT_ENCODING, "gzip, br")
            .header(ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8")
            .header(ORIGIN, "https://www.bilibili.com")
            .header(
                USER_AGENT,
                crate::download::bili_http::BILIBILI_UA,
            )
            .header(REFERER, format!("https://www.bilibili.com/video/{bvid}"));
        if let Some(cookies) = cookies {
            request = request.header(COOKIE, cookies.to_header());
        }

        response_text(request.send().await?.error_for_status()?).await
    }
}

async fn response_text(response: Response) -> AppResult<String> {
    let bytes = response.bytes().await?;
    decode_response_bytes(&bytes)
}

fn decode_response_bytes(bytes: &[u8]) -> AppResult<String> {
    if looks_like_xml(bytes) {
        return Ok(String::from_utf8_lossy(bytes).into_owned());
    }

    for decoder in [
        decode_zlib_deflate as fn(&[u8]) -> Option<Vec<u8>>,
        decode_raw_deflate,
    ] {
        if let Some(decoded) = decoder(bytes) {
            if looks_like_xml(&decoded) {
                return Ok(String::from_utf8_lossy(&decoded).into_owned());
            }
        }
    }

    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn looks_like_xml(bytes: &[u8]) -> bool {
    let trimmed = bytes
        .iter()
        .copied()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .take(32)
        .collect::<Vec<_>>();
    trimmed.starts_with(b"<?xml") || trimmed.starts_with(b"<i") || trimmed.starts_with(b"<d")
}

fn decode_zlib_deflate(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).ok()?;
    Some(decoded)
}

fn decode_raw_deflate(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(bytes);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).ok()?;
    Some(decoded)
}

fn parse_danmaku_xml(xml: &str) -> Vec<DanmakuEntry> {
    let entries = parse_danmaku_xml_lossy(xml);
    if entries.is_empty() {
        parse_danmaku_xml_strict(xml).unwrap_or_default()
    } else {
        entries
    }
}

fn parse_danmaku_xml_strict(xml: &str) -> AppResult<Vec<DanmakuEntry>> {
    let sanitized_xml = sanitize_xml_entities(xml);
    let mut reader = Reader::from_str(&sanitized_xml);
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

fn parse_danmaku_xml_lossy(xml: &str) -> Vec<DanmakuEntry> {
    let mut entries = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = xml[cursor..].find("<d") {
        let start = cursor + relative_start;
        if !is_danmaku_tag_start(xml, start) {
            cursor = start + "<d".len();
            continue;
        }

        let Some(relative_end) = xml[start..].find("</d>") else {
            break;
        };
        let text_end = start + relative_end;
        let Some(relative_tag_end) = xml[start..text_end].find('>') else {
            cursor = text_end + "</d>".len();
            continue;
        };
        let tag_end = start + relative_tag_end;
        let tag = &xml[start..=tag_end];
        let text = decode_basic_xml_entities(&xml[tag_end + 1..text_end]);

        if let Some(meta) = extract_attr_value(tag, "p") {
            if let Some(entry) = parse_danmaku_entry(&meta, text) {
                entries.push(entry);
            }
        }

        cursor = text_end + "</d>".len();
    }

    entries
}

fn is_danmaku_tag_start(xml: &str, start: usize) -> bool {
    xml[start + "<d".len()..]
        .chars()
        .next()
        .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '>' | '/'))
}

fn extract_attr_value(tag: &str, attr_name: &str) -> Option<String> {
    let mut cursor = 0;

    while let Some(relative_name_start) = tag[cursor..].find(attr_name) {
        let name_start = cursor + relative_name_start;
        let before_ok = tag[..name_start]
            .chars()
            .next_back()
            .is_none_or(|ch| ch.is_whitespace() || matches!(ch, '<' | '/'));
        let after_name = name_start + attr_name.len();
        let after_ok = tag[after_name..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_whitespace() || ch == '=');

        if !before_ok || !after_ok {
            cursor = after_name;
            continue;
        }

        let mut value_start = after_name + leading_whitespace_len(&tag[after_name..]);
        if !tag[value_start..].starts_with('=') {
            cursor = after_name;
            continue;
        }
        value_start += '='.len_utf8();
        value_start += leading_whitespace_len(&tag[value_start..]);

        let tag_end = tag.rfind('>').unwrap_or(tag.len());
        if value_start >= tag_end {
            return None;
        }

        let first = tag[value_start..].chars().next()?;
        let value = if matches!(first, '"' | '\'') {
            value_start += first.len_utf8();
            let search = &tag[value_start..tag_end];
            let value_end = search
                .find(first)
                .map(|offset| value_start + offset)
                .unwrap_or(tag_end);
            &tag[value_start..value_end]
        } else {
            let search = &tag[value_start..tag_end];
            let value_end = search
                .find(|ch: char| ch.is_whitespace() || ch == '>')
                .map(|offset| value_start + offset)
                .unwrap_or(tag_end);
            &tag[value_start..value_end]
        };

        return Some(decode_basic_xml_entities(value.trim()));
    }

    None
}

fn leading_whitespace_len(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

fn decode_basic_xml_entities(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut cursor = 0;

    while cursor < text.len() {
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };

        if ch != '&' {
            decoded.push(ch);
            cursor += ch.len_utf8();
            continue;
        }

        let rest = &text[cursor + 1..];
        if let Some(end_offset) = rest.find(';') {
            let entity = &rest[..end_offset];
            if let Some(ch) = decode_xml_entity(entity) {
                decoded.push(ch);
            } else {
                decoded.push('&');
                decoded.push_str(entity);
                decoded.push(';');
            }
            cursor += end_offset + 2;
        } else {
            decoded.push('&');
            cursor += 1;
        }
    }

    decoded
}

fn decode_xml_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => entity
            .strip_prefix("#x")
            .and_then(|digits| u32::from_str_radix(digits, 16).ok())
            .or_else(|| {
                entity
                    .strip_prefix('#')
                    .and_then(|digits| digits.parse::<u32>().ok())
            })
            .and_then(char::from_u32),
    }
}

fn sanitize_xml_entities(xml: &str) -> String {
    let mut sanitized = String::with_capacity(xml.len());
    let mut index = 0;

    while index < xml.len() {
        let Some(ch) = xml[index..].chars().next() else {
            break;
        };

        if ch != '&' {
            sanitized.push(ch);
            index += ch.len_utf8();
            continue;
        }

        let rest = &xml[index + 1..];
        if let Some(end_offset) = rest.find(';') {
            let entity = &rest[..end_offset];
            if is_valid_xml_entity(entity) {
                sanitized.push_str(&xml[index..=index + end_offset + 1]);
                index += end_offset + 2;
                continue;
            }
        }

        sanitized.push_str("&amp;");
        index += 1;
    }

    sanitized
}

fn is_valid_xml_entity(entity: &str) -> bool {
    matches!(entity, "amp" | "lt" | "gt" | "quot" | "apos")
        || entity
            .strip_prefix("#x")
            .filter(|digits| !digits.is_empty())
            .is_some_and(|digits| digits.chars().all(|ch| ch.is_ascii_hexdigit()))
        || entity
            .strip_prefix('#')
            .filter(|digits| !digits.is_empty())
            .is_some_and(|digits| digits.chars().all(|ch| ch.is_ascii_digit()))
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
        Style: Scroll,Microsoft YaHei,44,&H00FFFFFF,&H00FFFFFF,&H80464646,&H50000000,0,0,0,0,100,100,0,0,1,2,0,7,30,30,36,1\n\
        Style: Top,Microsoft YaHei,44,&H00FFFFFF,&H00FFFFFF,&H80464646,&H50000000,0,0,0,0,100,100,0,0,1,2,0,8,30,30,40,1\n\
        Style: Bottom,Microsoft YaHei,44,&H00FFFFFF,&H00FFFFFF,&H80464646,&H50000000,0,0,0,0,100,100,0,0,1,2,0,2,30,30,48,1\n\n\
        [Events]\n\
        Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for (index, entry) in entries.iter().enumerate() {
        let start = entry.start;
        let end = start + 5.0;
        let text = escape_ass_text(&entry.text);
        let (style, effect) = match entry.mode {
            DanmakuMode::Scroll => {
                let y = 48 + ((index % 15) as i32 * 54);
                (
                    "Scroll",
                    format!("{{\\move(1920,{y},{},{y})}}", -text_width_hint(&entry.text)),
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
        let entries = parse_danmaku_xml(xml);
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

    #[test]
    fn tolerates_unescaped_ampersands_in_danmaku_text() {
        let xml = r#"<i><d p="2,1,25,16777215,1,0,0,0">A&B; C&D</d></i>"#;
        let entries = parse_danmaku_xml(xml);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "A&B; C&D");
    }

    #[test]
    fn falls_back_to_lossy_parser_for_bad_xml() {
        let xml = r#"<i><d p="4,1,25,16777215,1,0,0,0">能用<坏标签</d><d p="6,5,25,16777215,1,0,0,0">顶部&amp;弹幕</d></i>"#;
        let entries = parse_danmaku_xml(xml);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "能用<坏标签");
        assert_eq!(entries[1].text, "顶部&弹幕");
        assert_eq!(entries[1].mode, DanmakuMode::Top);
    }

    #[test]
    fn skips_malformed_attribute_and_keeps_later_entries() {
        let xml = r#"<i><d p=`4,1,25,16777215,1,0,0,0">坏属性</d><d p="6,4,25,16777215,1,0,0,0">底部弹幕</d></i>"#;
        let entries = parse_danmaku_xml(xml);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "底部弹幕");
        assert_eq!(entries[0].mode, DanmakuMode::Bottom);
    }

    #[test]
    fn renders_empty_ass_when_no_valid_danmaku_entries_exist() {
        let entries = parse_danmaku_xml("<i><d p=`bad>坏属性</d><d>没有元数据</d></i>");
        let ass = render_ass(&entries);

        assert!(entries.is_empty());
        assert!(ass.contains("[Events]"));
        assert!(!ass.contains("Dialogue:"));
    }

    #[test]
    fn parses_real_bilibili_dm_shape() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><i><chatserver>chat.bilibili.com</chatserver><chatid>37011327429</chatid><d p="8.96900,5,25,16777215,1774871197,0,84aeb754,2078529265494333184,10">其实日语里就是这意思，翻译组说实话而已</d><d p="11.57200,1,25,16777215,1774713557,0,84c79d4f,2077206892719400448,5">Grilled cheese</d><d p="7.61900,1,25,16777215,1774616830,0,52cce0c1,2076395487128430336,1">不是变态女？</d></i>"#;
        let entries = parse_danmaku_xml(xml);

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].start, 8.969);
        assert_eq!(entries[0].mode, DanmakuMode::Top);
        assert_eq!(entries[2].text, "不是变态女？");
    }

    #[test]
    fn parses_full_real_bilibili_dm_payload() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><i><chatserver>chat.bilibili.com</chatserver><chatid>37011327429</chatid><mission>0</mission><maxlimit>100</maxlimit><state>0</state><real_name>0</real_name><source>k-v</source><d p="8.96900,5,25,16777215,1774871197,0,84aeb754,2078529265494333184,10">其实日语里就是这意思，翻译组说实话而已</d><d p="9.34700,5,25,16777215,1774775909,0,1a86942d,2077729932050178304,10">这不就是字面翻译吗</d><d p="8.15000,5,25,16777215,1774628133,0,8b395577,2076490297961670400,10">金克丝</d><d p="9.15500,1,25,16777215,1776129974,0,6d8d6737,2089088656089699328,9">龙哥被震住了</d><d p="9.73300,1,25,16777215,1774620598,0,a604d287,2076427093406691072,9">这就是原意哦</d><d p="8.05000,5,25,14811775,1776230570,0,d8b94053,2089932515367852800,9">其实说的就是这个</d><d p="1.91400,1,25,16777215,1776459840,0,355967cb,2091855769049117696,8">并非呕吐女，原义瑟琴女</d><d p="9.78300,1,25,16777215,1774702318,0,bc1a1832,2077112607701608192,8">有没有可能这才是直译</d><d p="1.46300,5,25,15138834,1774683262,0,5b231b64,2076952756132182272,8">不是エロ女吗</d><d p="10.91900,1,25,16777215,1776459873,0,355967cb,2091856053321948672,7">原义迪奥毛</d><d p="6.03800,1,25,16777215,1775930730,0,3d70b81d,2087417277023699456,7">我这里听的是ero onna色女人</d><d p="8.56800,1,25,16777215,1775918326,0,2d52dd9d,2087313224923565824,6">、</d><d p="11.83700,1,25,16777215,1774932969,0,32ab9291,2079047450228304896,6">啥</d><d p="10.65500,1,25,16777215,1774923936,0,dd14c6bf,2078971679370993152,6">？</d><d p="11.57200,1,25,16777215,1774713557,0,84c79d4f,2077206892719400448,5">Grilled cheese</d><d p="9.50100,1,25,16777215,1774711584,0,c589ce9b,2077190334362253056,5">什么东西？</d><d p="9.94900,1,25,16777215,1774682427,0,b7a41094,2076945752331915264,5">？</d><d p="6.33700,1,25,16777215,1774656776,0,eeab5101,2076730579142866688,4">？</d><d p="1.49000,1,25,16777215,1774653081,0,c9edfae3,2076699580208361472,4">确实是这个意思チンガス</d><d p="7.97400,1,25,16777215,1774636289,0,9edff67f,2076558716874210304,3">？</d><d p="11.70100,1,25,16777215,1774636277,0,9edff67f,2076558613937492736,3">？</d><d p="10.43100,1,25,16777215,1774622250,0,5d7d6acc,2076440952444578560,3">？</d><d p="9.58100,1,25,16777215,1774621478,0,75c4dc43,2076434474291877120,2">？</d><d p="0.00100,1,25,16777215,1774619446,0,6afe6ecf,2076417427365172480,2">？</d><d p="9.08300,1,25,16777215,1774618808,0,89df73f3,2076412076984653056,2">？</d><d p="10.42200,1,25,16777215,1774618645,0,eb15a6ac,2076410705774475264,1">？</d><d p="7.61900,1,25,16777215,1774616830,0,52cce0c1,2076395487128430336,1">不是变态女？</d></i>"#;
        let entries = parse_danmaku_xml(xml);
        let ass = render_ass(&entries);

        assert_eq!(entries.len(), 27);
        assert!(ass.contains("Dialogue: 0,0:00:08.97,0:00:13.97,Top"));
        assert!(ass.contains("其实日语里就是这意思"));
        assert!(ass.contains("不是变态女？"));
    }

    #[tokio::test]
    async fn live_danmaku_contract_when_enabled() -> AppResult<()> {
        let Ok(cid) = std::env::var("BILI_LIVE_DANMAKU_CID") else {
            return Ok(());
        };
        let cid = cid.parse::<u64>().expect("cid should be numeric");
        let bvid =
            std::env::var("BILI_LIVE_DANMAKU_BVID").unwrap_or_else(|_| "BV1qtXbB3ESu".to_string());
        let client = DanmakuClient::new();
        let xml = client.fetch_comment_xml(cid, &bvid, None).await?;
        let entries = parse_danmaku_xml(&xml);
        eprintln!(
            "live danmaku len={} d_tags={} entries={} prefix={}",
            xml.len(),
            xml.matches("<d ").count(),
            entries.len(),
            &xml[..xml.len().min(120)]
        );

        assert!(!entries.is_empty());
        Ok(())
    }
}
