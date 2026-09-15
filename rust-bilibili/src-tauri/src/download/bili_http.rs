//! B站 CDN 流原语：UA、体积探测、分片规划、MD5、header 解析。
//!
//! 所有下载/直传管线从这里取，CDN 行为知识只有一份实现、一处可测。

use reqwest::{
    header::{HeaderMap, CONTENT_LENGTH, CONTENT_RANGE, COOKIE, RANGE, REFERER},
    StatusCode,
};

/// B站 CDN / 接口共用的桌面 UA。改这里即改全部客户端。
pub const BILIBILI_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

pub fn header_content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
}

pub fn header_content_range_total(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_RANGE)?
        .to_str()
        .ok()?
        .rsplit('/')
        .next()?
        .parse::<u64>()
        .ok()
}

pub fn md5_hex(bytes: &[u8]) -> String {
    format!("{:x}", md5::compute(bytes))
}

/// 把 total 按 part_size 切成不重叠的 (start, end) 闭区间列表。
pub fn plan_ranges(total: u64, part_size: u64) -> Vec<(u64, u64)> {
    if total == 0 || part_size == 0 {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = 0_u64;
    while start < total {
        let end = (start + part_size - 1).min(total - 1);
        ranges.push((start, end));
        start = end + 1;
    }
    ranges
}

/// 用 `Range: bytes=0-0` 探测总体积：206 → Content-Range 总值，200 → Content-Length。
/// 部分 CDN 节点对 HEAD 回 404，所以不用 HEAD。
pub async fn probe_content_size(
    client: &reqwest::Client,
    url: &str,
    referer: &str,
    cookie_header: Option<&str>,
) -> Option<u64> {
    let mut request = client
        .get(url)
        .header(REFERER, referer)
        .header(RANGE, "bytes=0-0");
    if let Some(cookie_header) = cookie_header {
        request = request.header(COOKIE, cookie_header);
    }
    let response = request.send().await.ok()?;
    match response.status() {
        StatusCode::PARTIAL_CONTENT => {
            header_content_range_total(response.headers()).filter(|total| *total > 0)
        }
        StatusCode::OK => header_content_length(response.headers()),
        _ => None,
    }
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

    #[test]
    fn parses_content_range_total() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_RANGE, "bytes 0-0/12345".parse().unwrap());

        assert_eq!(header_content_range_total(&headers), Some(12345));
    }
}
