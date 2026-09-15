"""B站视频API客户端 - 视频信息获取与播放地址解析"""

import time
import json
import hashlib
import urllib.parse
from typing import Optional

import requests


class BilibiliAPI:
    """B站 API 客户端，处理视频元数据和播放地址获取"""

    BASE_URL = "https://api.bilibili.com"

    DEFAULT_HEADERS = {
        "User-Agent": (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
            "AppleWebKit/537.36 (KHTML, like Gecko) "
            "Chrome/125.0.0.0 Safari/537.36"
        ),
        "Accept": "application/json, text/plain, */*",
        "Accept-Language": "zh-CN,zh;q=0.9,en;q=0.8",
        "Origin": "https://www.bilibili.com",
    }

    def __init__(self, sessdata: str = "", bili_jct: str = "", dedeuserid: str = ""):
        self.session = requests.Session()
        self.session.headers.update(self.DEFAULT_HEADERS)
        self._set_cookies(sessdata, bili_jct, dedeuserid)

    def _set_cookies(self, sessdata: str, bili_jct: str, dedeuserid: str):
        if sessdata:
            self.session.cookies.set("SESSDATA", sessdata, domain=".bilibili.com")
        if bili_jct:
            self.session.cookies.set("bili_jct", bili_jct, domain=".bilibili.com")
        if dedeuserid:
            self.session.cookies.set("DedeUserID", dedeuserid, domain=".bilibili.com")

    # ------------------------------------------------------------------
    # 登录状态验证
    # ------------------------------------------------------------------

    def check_login(self) -> dict:
        """验证当前 Cookie 是否有效，返回用户信息"""
        url = f"{self.BASE_URL}/x/web-interface/nav"
        resp = self.session.get(url, headers={"Referer": "https://www.bilibili.com/"})
        resp.raise_for_status()
        data = resp.json()
        return {
            "is_login": data.get("data", {}).get("isLogin", False),
            "username": data.get("data", {}).get("uname", ""),
            "uid": data.get("data", {}).get("mid", 0),
            "level": data.get("data", {}).get("level_info", {}).get("current_level", 0),
            "vip_type": data.get("data", {}).get("vipStatus", 0),
            "error_code": data.get("code", -1),
            "error_msg": data.get("message", ""),
        }

    # ------------------------------------------------------------------
    # 视频信息
    # ------------------------------------------------------------------

    def get_video_info(self, bvid: str) -> dict:
        """获取视频基本信息（标题、分P、cid等）"""
        url = f"{self.BASE_URL}/x/web-interface/view"
        params = {"bvid": bvid}
        resp = self.session.get(url, params=params)
        resp.raise_for_status()
        data = resp.json()
        if data["code"] != 0:
            raise APIError(data["code"], data.get("message", "unknown error"))
        return data["data"]

    def get_cid_by_bvid(self, bvid: str) -> list[dict]:
        """获取视频的所有分P cid列表"""
        info = self.get_video_info(bvid)
        pages = info.get("pages", [])
        return [{"cid": p["cid"], "title": p["part"], "page": p["page"], "duration": p["duration"]} for p in pages]

    # ------------------------------------------------------------------
    # 播放地址
    # ------------------------------------------------------------------

    def get_playurl(
        self,
        bvid: str,
        cid: int,
        qn: int = 80,
        fnval: int = 4048,
        platform: str = "web",
        fourk: bool = False,
    ) -> dict:
        """获取视频播放地址（DASH / DURL）

        fnval:
          16  DASH
          4048  DASH + 全部格式支持（推荐）
          4096  DASH + HDR

        qn 常用值:
          16  360P
          32  480P
          64  720P
          80  1080P
          112 1080P+ (高码率)
          116 1080P60
          120 4K
          125 HDR
        """
        url = f"{self.BASE_URL}/x/player/playurl"
        params = {
            "bvid": bvid,
            "cid": cid,
            "qn": qn,
            "platform": platform,
            "fnval": fnval,
            "fnver": 0,
            "fourk": 1 if fourk else 0,
            "force_host": 2,
            "client_ts": int(time.time()),
        }
        headers = {"Referer": f"https://www.bilibili.com/video/{bvid}"}
        resp = self.session.get(url, params=params, headers=headers)
        resp.raise_for_status()
        data = resp.json()
        if data["code"] != 0:
            raise APIError(data["code"], data.get("message", "unknown error"))
        return data["data"]

    # ------------------------------------------------------------------
    # 充电视频检测
    # ------------------------------------------------------------------

    def check_charging_status(self, bvid: str) -> dict:
        """检测视频是否为充电/付费专属视频（含 UPower 高档充电）"""
        info = self.get_video_info(bvid)
        rights = info.get("rights", {})
        is_upower = bool(info.get("is_upower_exclusive"))
        is_upower_play = bool(info.get("is_upower_play"))
        is_upower_preview = bool(info.get("is_upower_preview"))
        is_charging = rights.get("elec_high", 0) == 1 or is_upower or is_upower_preview
        return {
            "bvid": bvid,
            "title": info.get("title", ""),
            "is_charging": is_charging,
            "is_upower_exclusive": is_upower,
            "is_upower_play": is_upower_play,
            "is_upower_preview": is_upower_preview,
            "need_pay": info.get("elec", 0) == 1 or is_charging,
            "need_vip": rights.get("vip_free", 0) == 0 and rights.get("bp", 0) == 0,
            "downloadable": rights.get("download", 0) == 1,
            "copyright": info.get("copyright", ""),
            "duration": info.get("duration", 0),
        }

    def check_video_access(self, bvid: str, cid: int) -> dict:
        """检测视频的实际可访问性（付费状态 / 是否试看）"""
        info = self.get_video_info(bvid)
        meta_sec = int(info.get("duration") or 0)
        pages = info.get("pages") or []
        for page in pages:
            if page.get("cid") == cid and page.get("duration"):
                meta_sec = int(page["duration"])
                break

        url = f"{self.BASE_URL}/x/player/playurl"
        params = {
            "bvid": bvid, "cid": cid,
            "qn": 80, "platform": "web",
            "fnval": 4048, "fnver": 0, "fourk": 1,
            "force_host": 2, "client_ts": int(time.time()),
        }
        headers = {"Referer": f"https://www.bilibili.com/video/{bvid}"}
        resp = self.session.get(url, params=params, headers=headers)
        resp.raise_for_status()
        data = resp.json()
        result = {"code": data.get("code", -1), "message": data.get("message", ""), "meta_sec": meta_sec}
        if data.get("code") == 0:
            d = data.get("data", {})
            result["quality"] = d.get("quality")
            result["accept_quality"] = d.get("accept_quality")
            result["format"] = d.get("format")
            result["has_dash"] = "dash" in d and bool(d.get("dash"))
            result["has_durl"] = "durl" in d and bool(d.get("durl"))
            result["video_codecid"] = d.get("video_codecid")
            result["timelength"] = d.get("timelength", 0)
            stream_ms = 0
            if d.get("durl"):
                stream_ms = sum(s.get("length", 0) for s in d["durl"])
                result["durl_total_ms"] = stream_ms
                urls = [s.get("url", "") for s in d["durl"]]
                result["is_preview_encode"] = any(
                    "-1-448." in u or "-448.mp4" in u for u in urls
                )
            elif d.get("dash") and d["dash"].get("duration"):
                stream_ms = int(d["dash"]["duration"]) * 1000
            result["stream_ms"] = stream_ms
            result["is_preview"] = self.is_preview_stream(
                meta_sec=meta_sec,
                stream_ms=stream_ms,
                has_dash_video=bool((d.get("dash") or {}).get("video")),
                is_preview_encode=result.get("is_preview_encode", False),
            )
        return result

    @staticmethod
    def is_preview_stream(
        meta_sec: int,
        stream_ms: int,
        has_dash_video: bool = False,
        is_preview_encode: bool = False,
    ) -> bool:
        """判断 playurl 是否为充电试看流。"""
        if is_preview_encode and meta_sec > 30:
            return True
        if meta_sec <= 0 or stream_ms <= 0:
            return False
        meta_ms = meta_sec * 1000
        if has_dash_video:
            return stream_ms < meta_ms // 2 and stream_ms + 3000 < meta_ms
        return stream_ms < meta_ms // 2 and stream_ms + 5000 < meta_ms

    @staticmethod
    def preview_block_message(meta_sec: int, stream_ms: int) -> str:
        stream_sec = max(stream_ms // 1000, 0)
        return (
            f"当前账号仅能获取充电试看流（约 {stream_sec}s / 全片 {meta_sec}s）。"
            "请确认已登录，并对该 UP 开通对应档位包月充电后重试。"
        )

    # ------------------------------------------------------------------
    # 视频流 URL 提取
    # ------------------------------------------------------------------

    @staticmethod
    def extract_dash_urls(playurl_data: dict) -> dict:
        """从 playurl 响应中提取 DASH 流信息

        返回包含 duration 和每个 track 的所有 segment URL 列表。
        B站 DASH 可能使用 media_segments (多分片) 或 SegmentBase (单文件)。
        """
        dash = playurl_data.get("dash") or playurl_data.get("dash", {})
        if not dash:
            return {"duration": 0, "video": [], "audio": []}

        result = {"duration": dash.get("duration", 0), "video": [], "audio": []}

        for track in dash.get("video", []):
            result["video"].append(BilibiliAPI._parse_dash_track(track, dash))

        for track in dash.get("audio", []):
            result["audio"].append(BilibiliAPI._parse_dash_track(track, dash))

        return result

    @staticmethod
    def _parse_dash_track(track: dict, dash: dict) -> dict:
        """解析单个 DASH track，提取所有 segment URL"""
        info = {
            "id": track.get("id", 0),
            "codecs": track.get("codecs", "unknown"),
            "width": track.get("width", 0),
            "height": track.get("height", 0),
            "frame_rate": track.get("frame_rate", "unknown"),
            "bandwidth": track.get("bandwidth", 0),
            "mime_type": track.get("mime_type", ""),
            "segment_urls": [],
            "base_url": track.get("base_url", ""),
        }

        # 方式1: media_segments — 显式的多分片列表
        segments = track.get("media_segments") or track.get("segments") or []
        if segments:
            for seg in segments:
                url = seg.get("url") or seg.get("base_url", "")
                if url:
                    info["segment_urls"].append(url)
            return info

        # 方式2: SegmentTemplate — 按模板生成分片 URL
        tmpl = track.get("SegmentTemplate") or track.get("segment_template") or {}
        if tmpl:
            media_tmpl = tmpl.get("media", "")
            duration = tmpl.get("duration", 0)
            timescale = tmpl.get("timescale", 1)
            start_num = tmpl.get("start_number", 0)
            total_segments = tmpl.get("end_number", 0)
            if total_segments == 0 and dash.get("duration", 0) > 0 and duration > 0:
                seg_dur = duration / timescale
                total_dur = dash["duration"]
                total_segments = int(total_dur / seg_dur) + 1

            base = track.get("base_url", "")
            base_dir = base[: base.rfind("/") + 1] if "/" in base else ""
            for i in range(total_segments):
                num = start_num + i
                seg_path = media_tmpl.replace("{Number}", str(num))
                url = base_dir + seg_path
                info["segment_urls"].append(url)
            return info

        # 方式3: 只有 base_url — 尝试从 base_url 获取全部内容
        if track.get("base_url"):
            info["segment_urls"].append(track["base_url"])

        return info

    @staticmethod
    def extract_durl_urls(playurl_data: dict) -> list[dict]:
        """从 playurl 响应中提取 DURL (FLV) 流 URL 列表"""
        durl = playurl_data.get("durl", [])
        result = []
        for seg in durl:
            result.append({
                "order": seg.get("order", 0),
                "length": seg.get("length", 0),
                "size": seg.get("size", 0),
                "url": seg.get("url", ""),
                "backup_urls": seg.get("backup_url", []),
            })
        return result


class APIError(Exception):
    """B站 API 返回错误"""
    def __init__(self, code: int, message: str):
        self.code = code
        self.message = message
        super().__init__(f"API error {code}: {message}")
