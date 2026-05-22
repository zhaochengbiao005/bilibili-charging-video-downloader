use std::{collections::BTreeMap, path::Path};

use qrcode::{render::svg, QrCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

const LOGIN_COOKIE_NAMES: [&str; 3] = ["SESSDATA", "bili_jct", "DedeUserID"];

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct CookieSet {
    pub cookies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoginStatus {
    pub is_login: bool,
    pub username: Option<String>,
    pub avatar: Option<String>,
    pub uid: Option<u64>,
    pub level: Option<u32>,
    pub vip_type: Option<u32>,
    pub message: Option<String>,
    pub cookie_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QrLoginStartResponse {
    pub url: String,
    pub qrcode_key: String,
    pub qrcode_svg: String,
    pub expires_in_sec: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QrLoginPollResponse {
    pub status: String,
    pub message: String,
    pub login: Option<LoginStatus>,
}

#[derive(Debug, Clone)]
pub struct QrLoginPollOutcome {
    pub response: QrLoginPollResponse,
    pub cookies: Option<CookieSet>,
}

impl CookieSet {
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into().trim().to_string();
        let value = value.into().trim().to_string();
        if !name.is_empty() && !value.is_empty() {
            self.cookies.insert(name, value);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    pub fn has_login_cookie(&self) -> bool {
        self.cookies
            .get("SESSDATA")
            .is_some_and(|value| !value.trim().is_empty())
    }

    pub fn to_header(&self) -> String {
        self.cookies
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn login_only(&self) -> Self {
        let mut filtered = Self::default();
        for name in LOGIN_COOKIE_NAMES {
            if let Some(value) = self.cookies.get(name) {
                filtered.insert(name, value);
            }
        }
        filtered
    }
}

impl LoginStatus {
    pub fn guest(message: impl Into<String>) -> Self {
        Self {
            is_login: false,
            username: None,
            avatar: None,
            uid: None,
            level: None,
            vip_type: None,
            message: Some(message.into()),
            cookie_path: None,
        }
    }

    pub fn with_cookie_path(mut self, path: Option<&Path>) -> Self {
        self.cookie_path = path.map(|path| path.to_string_lossy().into_owned());
        self
    }
}

pub fn parse_cookie_file(path: &Path) -> AppResult<CookieSet> {
    let content = std::fs::read_to_string(path)?;
    parse_cookie_content(&content)
}

pub fn parse_cookie_content(content: &str) -> AppResult<CookieSet> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput {
            message: "Cookie 文件为空".to_string(),
        });
    }

    let cookies = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_json_cookies(trimmed)?
    } else if trimmed.lines().any(is_netscape_cookie_line) {
        parse_netscape_cookies(trimmed)
    } else {
        parse_cookie_header(trimmed)
    };

    if cookies.is_empty() {
        return Err(AppError::InvalidInput {
            message: "未在文件中找到可用 Cookie".to_string(),
        });
    }

    Ok(cookies)
}

pub fn build_qrcode_svg(url: &str) -> AppResult<String> {
    let code = QrCode::new(url.as_bytes()).map_err(|err| AppError::InvalidInput {
        message: format!("二维码生成失败: {err}"),
    })?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .dark_color(svg::Color("#18181b"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

pub fn parse_set_cookie_headers<'a>(headers: impl IntoIterator<Item = &'a str>) -> CookieSet {
    let mut cookies = CookieSet::default();
    for header in headers {
        if let Some((name, rest)) = header.split_once('=') {
            let value = rest.split(';').next().unwrap_or_default();
            cookies.insert(name, value);
        }
    }
    cookies
}

fn parse_json_cookies(content: &str) -> AppResult<CookieSet> {
    let value: Value = serde_json::from_str(content)?;
    let mut cookies = CookieSet::default();
    collect_json_cookies(&value, &mut cookies);
    Ok(cookies)
}

fn collect_json_cookies(value: &Value, cookies: &mut CookieSet) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_json_cookies(item, cookies);
            }
        }
        Value::Object(map) => {
            if let (Some(name), Some(value)) = (
                map.get("name").and_then(Value::as_str),
                map.get("value").and_then(Value::as_str),
            ) {
                cookies.insert(name, value);
                return;
            }

            if let Some(items) = map.get("cookies") {
                collect_json_cookies(items, cookies);
            }

            for (name, value) in map {
                if name == "cookies" {
                    continue;
                }
                if let Some(value) = value.as_str() {
                    cookies.insert(name, value);
                }
            }
        }
        _ => {}
    }
}

fn is_netscape_cookie_line(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return false;
    }
    let fields: Vec<&str> = line.split_whitespace().collect();
    fields.len() >= 7 && fields[1].eq_ignore_ascii_case("TRUE")
        || fields.len() >= 7 && fields[1].eq_ignore_ascii_case("FALSE")
}

fn parse_netscape_cookies(content: &str) -> CookieSet {
    let mut cookies = CookieSet::default();
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = if line.contains('\t') {
            line.split('\t').collect()
        } else {
            line.split_whitespace().collect()
        };

        if fields.len() >= 7 {
            cookies.insert(fields[5], fields[6..].join(" "));
        }
    }
    cookies
}

fn parse_cookie_header(content: &str) -> CookieSet {
    let mut cookies = CookieSet::default();
    let normalized = content
        .lines()
        .map(|line| line.trim().strip_prefix("Cookie:").unwrap_or(line.trim()))
        .collect::<Vec<_>>()
        .join("; ");

    for part in normalized.split(';') {
        let part = part.trim();
        if part.starts_with('$') {
            continue;
        }
        if let Some((name, value)) = part.split_once('=') {
            cookies.insert(name, value);
        }
    }

    cookies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_json_cookie_object() -> AppResult<()> {
        let cookies =
            parse_cookie_content(r#"{"SESSDATA":"sess","bili_jct":"csrf","DedeUserID":"42"}"#)?;

        assert_eq!(
            cookies.cookies.get("SESSDATA").map(String::as_str),
            Some("sess")
        );
        assert!(cookies.has_login_cookie());
        Ok(())
    }

    #[test]
    fn parses_cookie_editor_array() -> AppResult<()> {
        let cookies = parse_cookie_content(
            r#"[{"name":"SESSDATA","value":"sess"},{"name":"bili_jct","value":"csrf"}]"#,
        )?;

        assert_eq!(
            cookies.cookies.get("bili_jct").map(String::as_str),
            Some("csrf")
        );
        Ok(())
    }

    #[test]
    fn parses_netscape_cookie_file() -> AppResult<()> {
        let cookies = parse_cookie_content(
            "# Netscape HTTP Cookie File\n.bilibili.com\tTRUE\t/\tTRUE\t0\tSESSDATA\tsess\n",
        )?;

        assert_eq!(
            cookies.cookies.get("SESSDATA").map(String::as_str),
            Some("sess")
        );
        Ok(())
    }

    #[test]
    fn parses_cookie_header_text() -> AppResult<()> {
        let cookies = parse_cookie_content("SESSDATA=sess; bili_jct=csrf; DedeUserID=42")?;

        assert_eq!(
            cookies.to_header(),
            "DedeUserID=42; SESSDATA=sess; bili_jct=csrf"
        );
        Ok(())
    }
}
