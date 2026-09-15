# -*- coding: utf-8 -*-
"""
Access-control surface probe for Bilibili UPower video BV159dABuEyd.
Goal: evidence whether full stream can be obtained without charged-account cookie.
"""
from __future__ import annotations

import hashlib
import json
import re
import time
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlencode, urlparse, urlunparse

import requests

BVID = "BV159dABuEyd"
CID = 38167710052
AID = 116533938360751
UP_MID = 11186360
META_MS = 125_000
PREVIEW_MS_EXPECT = 15_000

OUT = Path("analysis_BV159dABuEyd/bypass")
OUT.mkdir(parents=True, exist_ok=True)

UA_WEB = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36"
)
UA_APP = (
    "Mozilla/5.0 BiliDroid/7.66.0 (bbcallen@gmail.com) os/android model/Pixel 6 "
    "mobi_app/android build/7660300 channel/master innerVer/7660310 osVer/13 network/2"
)
UA_TV = "Mozilla/5.0 BiliTV/1.0.0"

# Historically published appkeys used by open-source bilibili clients (signature surface only)
APPKEYS = [
    ("1d8b6e7d45233436", "560c52ccd288fed045859ed18bffd973"),  # android
    ("27eb53fc9058f8c3", "c2ed53a74eeefe3cf99fbd01d8c9c375"),  # ios
    ("4409e2ce8ffd12b8", "59b43e04ad6965f34319062b478f83dd"),  # android TV
]


def save(name: str, obj: Any) -> None:
    p = OUT / name
    p.write_text(json.dumps(obj, ensure_ascii=False, indent=2), encoding="utf-8")
    print("saved", p.name)


def summarize_playurl(data: dict) -> dict:
    d = data.get("data") or {}
    if not isinstance(d, dict):
        d = {}
    durl = d.get("durl") or []
    dash = d.get("dash") or {}
    vtracks = dash.get("video") or []
    atracks = dash.get("audio") or []
    durl_ms = sum(int(s.get("length") or 0) for s in durl)
    dash_ms = int((dash.get("duration") or 0) * 1000) if dash else 0
    stream_ms = durl_ms or dash_ms or int(d.get("timelength") or 0)
    fullish = stream_ms >= META_MS * 0.8
    previewish = 0 < stream_ms < META_MS * 0.4
    return {
        "code": data.get("code"),
        "message": data.get("message"),
        "quality": d.get("quality"),
        "format": d.get("format"),
        "timelength_meta": d.get("timelength"),
        "has_dash": bool(vtracks or atracks),
        "dash_v": len(vtracks),
        "dash_a": len(atracks),
        "durl_n": len(durl),
        "stream_ms": stream_ms,
        "durl_size": sum(int(s.get("size") or 0) for s in durl),
        "verdict": "FULL" if fullish else ("PREVIEW" if previewish else "UNKNOWN/EMPTY"),
        "first_url_host": (
            (durl[0].get("url") or "").split("/")[2]
            if durl and (durl[0].get("url") or "").startswith("http")
            else (
                (vtracks[0].get("base_url") or "").split("/")[2]
                if vtracks and (vtracks[0].get("base_url") or "").startswith("http")
                else ""
            )
        ),
    }


def get_json(url: str, params: dict | None = None, headers: dict | None = None, timeout: int = 15) -> dict:
    try:
        r = requests.get(url, params=params, headers=headers or {}, timeout=timeout)
        try:
            return {"_http": r.status_code, **r.json()}
        except Exception:
            return {"_http": r.status_code, "_text": r.text[:500]}
    except Exception as e:
        return {"_error": str(e)}


def app_sign(params: dict, appsec: str) -> dict:
    items = sorted((k, str(v)) for k, v in params.items() if k != "sign")
    query = urlencode(items)
    sign = hashlib.md5((query + appsec).encode()).hexdigest()
    out = dict(params)
    out["sign"] = sign
    return out


def probe_web_param_injection() -> list[dict]:
    """Try client-side flags that might flip preview off."""
    base = {
        "bvid": BVID,
        "cid": CID,
        "qn": 80,
        "fnval": 4048,
        "fnver": 0,
        "fourk": 1,
        "force_host": 2,
        "platform": "web",
    }
    injects = [
        {},
        {"try_look": 0},
        {"try_look": 1},
        {"is_preview": 0},
        {"is_preview": 1},
        {"preview": 0},
        {"need_vip": 0},
        {"vip_status": 1},
        {"is_upower_play": 1},
        {"upower": 1},
        {"gaia_source": "pre-load"},
        {"from_spmid": "333.788"},
        {"otype": "json"},
        {"type": "mp4"},
        {"high_quality": 1},
        {"download": 1},
        {"npcybs": 0},
        {"fnval": 0},
        {"fnval": 16},
        {"fnval": 80},
        {"fnval": 144},
        {"fnval": 208},
        {"fnval": 976},
        {"fnval": 4048},
        {"fnval": 8192},
        {"fnval": 12240},
    ]
    headers = {
        "User-Agent": UA_WEB,
        "Referer": f"https://www.bilibili.com/video/{BVID}",
        "Origin": "https://www.bilibili.com",
    }
    rows = []
    for extra in injects:
        params = {**base, **extra, "client_ts": int(time.time())}
        # ensure single fnval if overridden in extra alone without base clash
        name = "web_" + ("_".join(f"{k}{v}" for k, v in extra.items()) or "base")
        data = get_json("https://api.bilibili.com/x/player/playurl", params, headers)
        row = {"name": name, "params_extra": extra, **summarize_playurl(data)}
        rows.append(row)
        print(row["name"], row["verdict"], row.get("stream_ms"), row.get("code"))
    save("01_web_param_injection.json", rows)
    return rows


def probe_alternate_hosts() -> list[dict]:
    headers_web = {
        "User-Agent": UA_WEB,
        "Referer": f"https://www.bilibili.com/video/{BVID}",
    }
    headers_app = {
        "User-Agent": UA_APP,
        "Referer": "https://www.bilibili.com",
    }
    params_common = {
        "bvid": BVID,
        "cid": CID,
        "aid": AID,
        "qn": 80,
        "fnval": 4048,
        "fnver": 0,
        "fourk": 1,
    }
    endpoints = [
        ("web_playurl", "https://api.bilibili.com/x/player/playurl", headers_web, params_common),
        ("wbi_playurl", "https://api.bilibili.com/x/player/wbi/playurl", headers_web, params_common),
        ("player_v2", "https://api.bilibili.com/x/player/v2", headers_web, {"bvid": BVID, "cid": CID}),
        ("app_v2_playurl", "https://app.bilibili.com/x/v2/playurl", headers_app, {**params_common, "build": 7660300, "mobi_app": "android", "platform": "android"}),
        ("app_playurl", "https://api.bilibili.com/x/playurl", headers_app, {**params_common, "otype": "json", "platform": "android", "qn": 80}),
        ("tv_playurl", "https://api.bilibili.com/x/tv/playurl", headers_app, {**params_common, "platform": "android", "mobi_app": "android_tv_yst"}),
        ("intl_playurl", "https://api.biliintl.com/intl/gateway/v2/app/playurl", headers_app, {"aid": AID, "cid": CID, "qn": 80, "fnval": 16}),
        ("pgc_web", "https://api.bilibili.com/pgc/player/web/playurl", headers_web, {"avid": AID, "cid": CID, "qn": 80, "fnval": 16}),
        ("pugv_web", "https://api.bilibili.com/pugv/player/web/playurl", headers_web, {"aid": AID, "cid": CID, "qn": 80, "fnval": 16}),
        ("html5_playurl", "https://api.bilibili.com/x/player/playurl", headers_web, {**params_common, "platform": "html5", "high_quality": 1}),
        ("stein_playurl", "https://api.bilibili.com/x/stein/playurl", headers_web, params_common),
        ("player_pagelist", "https://api.bilibili.com/x/player/pagelist", headers_web, {"bvid": BVID}),
    ]
    rows = []
    for name, url, headers, params in endpoints:
        data = get_json(url, params, headers)
        if "data" in data and isinstance(data.get("data"), dict) and (
            "durl" in data["data"] or "dash" in data["data"] or "timelength" in data["data"]
        ):
            row = {"name": name, "url": url, **summarize_playurl(data)}
        else:
            row = {
                "name": name,
                "url": url,
                "code": data.get("code"),
                "message": data.get("message") or data.get("_error") or data.get("_text", "")[:120],
                "http": data.get("_http"),
                "verdict": "NO_STREAM",
            }
        rows.append(row)
        print(name, row.get("verdict"), row.get("stream_ms"), row.get("code"), row.get("message"))
        save(f"02_{name}.json", data)
    save("02_hosts_summary.json", rows)
    return rows


def probe_appkey_signed() -> list[dict]:
    rows = []
    for appkey, appsec in APPKEYS:
        params = {
            "appkey": appkey,
            "cid": CID,
            "avid": AID,
            "qn": 80,
            "otype": "json",
            "platform": "android",
            "fnval": 16,
            "fnver": 0,
            "fourk": 1,
            "build": 7660300,
            "mobi_app": "android",
            "ts": int(time.time()),
        }
        signed = app_sign(params, appsec)
        headers = {"User-Agent": UA_APP}
        for name, url in [
            (f"signed_{appkey[:6]}_x_playurl", "https://api.bilibili.com/x/playurl"),
            (f"signed_{appkey[:6]}_v2", "https://app.bilibili.com/x/v2/playurl"),
            (f"signed_{appkey[:6]}_player", "https://api.bilibili.com/x/player/playurl"),
        ]:
            data = get_json(url, signed, headers)
            if isinstance(data.get("data"), dict) and (
                "durl" in data["data"] or "dash" in data["data"]
            ):
                row = {"name": name, **summarize_playurl(data)}
            else:
                # some app endpoints nest differently
                d = data.get("data")
                if isinstance(d, dict) and isinstance(d.get("dash"), dict):
                    row = {"name": name, **summarize_playurl(data)}
                else:
                    row = {
                        "name": name,
                        "code": data.get("code"),
                        "message": str(data.get("message") or data.get("_error") or "")[:160],
                        "verdict": "NO_STREAM",
                    }
            rows.append(row)
            print(row["name"], row.get("verdict"), row.get("stream_ms"), row.get("code"))
            save(f"03_{name}.json", data)
    save("03_appkey_summary.json", rows)
    return rows


def probe_html_page_leak() -> dict:
    """Fetch video HTML for embedded playinfo / __INITIAL_STATE__ full urls."""
    headers = {
        "User-Agent": UA_WEB,
        "Referer": "https://www.bilibili.com/",
        "Accept-Language": "zh-CN,zh;q=0.9",
    }
    r = requests.get(f"https://www.bilibili.com/video/{BVID}", headers=headers, timeout=20)
    text = r.text
    out = {
        "http": r.status_code,
        "len": len(text),
        "has_playinfo": "__playinfo__" in text,
        "has_initial": "__INITIAL_STATE__" in text,
    }
    # extract __playinfo__
    m = re.search(r"window\.__playinfo__\s*=\s*(\{.+?\})\s*;?\s*</script>", text, re.S)
    if not m:
        m = re.search(r"<script>window\.__playinfo__=(\{.+?\})</script>", text, re.S)
    playinfo = None
    if m:
        try:
            playinfo = json.loads(m.group(1))
            out["playinfo_summary"] = summarize_playurl(playinfo if "data" in playinfo else {"data": playinfo, "code": 0})
        except Exception as e:
            out["playinfo_error"] = str(e)
            out["playinfo_raw_head"] = m.group(1)[:300]
    # search for bilivideo urls in page
    urls = sorted(set(re.findall(r"https://[^\"'\\s]+bilivideo\\.com[^\"'\\s]*", text)))
    out["bilivideo_url_count"] = len(urls)
    out["bilivideo_hosts"] = sorted({urlparse(u).netloc for u in urls})[:20]
    # upower / elec markers
    for key in ["is_upower_exclusive", "is_upower_play", "is_upower_preview", "elec_high_level", "privilege_type"]:
        out[f"html_has_{key}"] = key in text
    save("04_html_meta.json", out)
    if playinfo is not None:
        save("04_html_playinfo.json", playinfo)
    # save small html snippet around playinfo
    idx = text.find("__playinfo__")
    if idx >= 0:
        (OUT / "04_html_snippet.txt").write_text(text[max(0, idx - 100) : idx + 500], encoding="utf-8")
    print("html", out.get("playinfo_summary"), "urls", out["bilivideo_url_count"])
    return out


def get_preview_url() -> str:
    headers = {
        "User-Agent": UA_WEB,
        "Referer": f"https://www.bilibili.com/video/{BVID}",
    }
    data = get_json(
        "https://api.bilibili.com/x/player/playurl",
        {"bvid": BVID, "cid": CID, "qn": 64, "fnval": 0, "platform": "web"},
        headers,
    )
    return ((data.get("data") or {}).get("durl") or [{}])[0].get("url") or ""


def probe_cdn_path_guess(preview_url: str) -> list[dict]:
    """Guess sibling CDN objects (other qn / full) from preview path."""
    if not preview_url:
        return []
    u = urlparse(preview_url)
    path = u.path  # e.g. /upgcxcode/52/00/38167710052/38167710052-1-64.mp4
    headers_ok = {
        "User-Agent": UA_WEB,
        "Referer": f"https://www.bilibili.com/video/{BVID}",
    }
    headers_bad = {"User-Agent": UA_WEB}

    candidates = []
    # mutate quality suffix
    for qn in [16, 32, 64, 80, 112, 116, 120, 125, 126, 127]:
        for codec in ["", "-1", "-1-"]:
            pass
        # common patterns: {cid}-1-{qn}.mp4 / {cid}-{qn}.mp4 / {cid}-1-{qn}.m4s
        base_dir = path.rsplit("/", 1)[0]
        stem = path.rsplit("/", 1)[-1]
        # replace trailing -N.ext
        stem_no_ext = stem.rsplit(".", 1)[0]
        ext = stem.rsplit(".", 1)[-1] if "." in stem else "mp4"
        # strip last -<num>
        core = re.sub(r"-\d+$", "", stem_no_ext)
        for e in ["mp4", "m4s", "flv"]:
            candidates.append(f"{base_dir}/{core}-{qn}.{e}")
            candidates.append(f"{base_dir}/{core}-1-{qn}.{e}")
            candidates.append(f"{base_dir}/{CID}-{qn}.{e}")
            candidates.append(f"{base_dir}/{CID}-1-{qn}.{e}")

    # unique preserve order
    seen = set()
    uniq = []
    for p in candidates:
        if p not in seen:
            seen.add(p)
            uniq.append(p)

    rows = []
    # always probe original first
    for label, headers in [("orig_ref", headers_ok), ("orig_noref", headers_bad)]:
        try:
            r = requests.get(preview_url, headers=headers, timeout=15, stream=True)
            chunk = next(r.iter_content(16), b"")
            rows.append(
                {
                    "label": label,
                    "path": u.path,
                    "status": r.status_code,
                    "clen": r.headers.get("content-length"),
                    "ctype": r.headers.get("content-type"),
                    "magic": chunk[:12].hex(),
                }
            )
            r.close()
        except Exception as e:
            rows.append({"label": label, "error": str(e)})

    # probe guesses with same query string (signed) and without
    qs = u.query
    sample = uniq[:40]  # limit
    for p in sample:
        for mode, q in [("with_qs", qs), ("no_qs", "")]:
            url = urlunparse((u.scheme, u.netloc, p, "", q, ""))
            try:
                r = requests.get(url, headers=headers_ok, timeout=10, stream=True)
                chunk = next(r.iter_content(16), b"")
                status = r.status_code
                clen = r.headers.get("content-length")
                ctype = r.headers.get("content-type")
                r.close()
            except Exception as e:
                status, clen, ctype, chunk = -1, None, None, b""
                err = str(e)
            else:
                err = None
            interesting = status == 200 and ctype and "video" in ctype
            if interesting or status not in (403, 404, -1):
                rows.append(
                    {
                        "path": p,
                        "mode": mode,
                        "status": status,
                        "clen": clen,
                        "ctype": ctype,
                        "magic": chunk[:12].hex() if chunk else "",
                        "interesting": interesting,
                        "error": err,
                    }
                )
                if interesting:
                    print("CDN HIT", p, clen, mode)
    save("05_cdn_guess.json", rows)
    print("cdn probe rows", len(rows))
    return rows


def probe_range_and_duration(preview_url: str) -> dict:
    """Confirm whether CDN object is truly short or truncated full file."""
    headers = {
        "User-Agent": UA_WEB,
        "Referer": f"https://www.bilibili.com/video/{BVID}",
    }
    out: dict[str, Any] = {"url_host": urlparse(preview_url).netloc if preview_url else ""}
    if not preview_url:
        save("06_range.json", out)
        return out

    # full GET content-length
    r = requests.get(preview_url, headers=headers, timeout=30, stream=True)
    out["status"] = r.status_code
    out["content_length"] = r.headers.get("content-length")
    out["accept_ranges"] = r.headers.get("accept-ranges")
    # download to temp and inspect with mutagen-less: just size + ftyp
    blob = b""
    for chunk in r.iter_content(256 * 1024):
        blob += chunk
        if len(blob) > 2 * 1024 * 1024:
            break
    r.close()
    # if small file, get all
    clen = int(out["content_length"] or 0)
    if clen and clen < 5 * 1024 * 1024:
        r2 = requests.get(preview_url, headers=headers, timeout=60)
        blob = r2.content
        out["downloaded_bytes"] = len(blob)
        out["magic"] = blob[:12].hex()
        # crude mp4 duration via mvhd timescale if present
        out["mp4_duration_sec_guess"] = parse_mp4_duration(blob)
        # save sample for local inspection
        (OUT / "06_preview.bin").write_bytes(blob[: min(len(blob), clen or len(blob))])
    else:
        out["downloaded_bytes"] = len(blob)
        out["magic"] = blob[:12].hex()
        out["mp4_duration_sec_guess"] = parse_mp4_duration(blob)

    # Range near end of advertised size
    if clen:
        start = max(0, clen - 64)
        hr = dict(headers)
        hr["Range"] = f"bytes={start}-{clen-1}"
        rr = requests.get(preview_url, headers=hr, timeout=15)
        out["range_end_status"] = rr.status_code
        out["range_end_len"] = len(rr.content)
        # request past end
        hr2 = dict(headers)
        hr2["Range"] = f"bytes={clen}-{clen+1024}"
        rr2 = requests.get(preview_url, headers=hr2, timeout=15)
        out["range_past_status"] = rr2.status_code
        out["range_past_len"] = len(rr2.content)
        # request far beyond (would succeed if full file hidden)
        hr3 = dict(headers)
        hr3["Range"] = f"bytes={clen*10}-{clen*10+1023}"
        rr3 = requests.get(preview_url, headers=hr3, timeout=15)
        out["range_far_status"] = rr3.status_code
        out["range_far_clen"] = rr3.headers.get("content-length")
        out["range_far_body_len"] = len(rr3.content)

    save("06_range.json", out)
    print("range", out)
    return out


def parse_mp4_duration(data: bytes) -> float | None:
    """Best-effort parse mvhd duration from first moov if present."""
    # search 'mvhd'
    idx = data.find(b"mvhd")
    if idx < 0:
        return None
    try:
        # atom starts 4 bytes before 'mvhd' size+type; version at mvhd+4
        version = data[idx + 4]
        if version == 0:
            timescale = int.from_bytes(data[idx + 16 : idx + 20], "big")
            duration = int.from_bytes(data[idx + 20 : idx + 24], "big")
        else:
            timescale = int.from_bytes(data[idx + 24 : idx + 28], "big")
            duration = int.from_bytes(data[idx + 28 : idx + 36], "big")
        if timescale:
            return round(duration / timescale, 3)
    except Exception:
        return None
    return None


def probe_fake_auth_headers() -> list[dict]:
    """Identity spoof headers / cookies that must not work if server-side."""
    base_params = {
        "bvid": BVID,
        "cid": CID,
        "qn": 80,
        "fnval": 4048,
        "platform": "web",
        "fnver": 0,
    }
    trials = [
        {"Cookie": "SESSDATA=AAAA; DedeUserID=1; bili_jct=1"},
        {"Cookie": f"SESSDATA=fake; DedeUserID={UP_MID}"},
        {"x-bili-mid": str(UP_MID)},
        {"x-bili-aurora-eid": "fake"},
        {"Authorization": "Bearer fake"},
        {"Cookie": "LIVE_BUVID=AUTO; sid=fake"},
    ]
    rows = []
    for i, extra in enumerate(trials):
        headers = {
            "User-Agent": UA_WEB,
            "Referer": f"https://www.bilibili.com/video/{BVID}",
            **extra,
        }
        data = get_json("https://api.bilibili.com/x/player/playurl", base_params, headers)
        row = {"trial": i, "extra_keys": list(extra.keys()), **summarize_playurl(data)}
        rows.append(row)
        print("fake", i, row["verdict"], row.get("stream_ms"))
    save("07_fake_auth.json", rows)
    return rows


def main() -> None:
    results = {
        "bvid": BVID,
        "meta_ms": META_MS,
        "goal": "find FULL stream without charged cookie",
    }
    print("=== 1 web param injection ===")
    r1 = probe_web_param_injection()
    print("=== 2 alternate hosts ===")
    r2 = probe_alternate_hosts()
    print("=== 3 appkey signed ===")
    r3 = probe_appkey_signed()
    print("=== 4 html leak ===")
    r4 = probe_html_page_leak()
    print("=== 5/6 cdn ===")
    preview = get_preview_url()
    results["preview_url_path"] = urlparse(preview).path if preview else ""
    r5 = probe_cdn_path_guess(preview)
    r6 = probe_range_and_duration(preview)
    print("=== 7 fake auth ===")
    r7 = probe_fake_auth_headers()

    def any_full(rows: list[dict]) -> list[dict]:
        return [x for x in rows if x.get("verdict") == "FULL"]

    results["full_hits"] = {
        "web_params": any_full(r1),
        "hosts": any_full(r2),
        "appkey": any_full(r3),
        "fake_auth": any_full(r7),
        "html": r4.get("playinfo_summary"),
        "cdn_interesting": [x for x in r5 if x.get("interesting")],
        "mp4_duration": r6.get("mp4_duration_sec_guess"),
        "content_length": r6.get("content_length"),
    }
    results["preview_only_confirmed"] = (
        not results["full_hits"]["web_params"]
        and not results["full_hits"]["hosts"]
        and not results["full_hits"]["appkey"]
        and not results["full_hits"]["fake_auth"]
        and not results["full_hits"]["cdn_interesting"]
        and (results["full_hits"]["mp4_duration"] or 0) < 30
    )
    save("00_bypass_verdict.json", results)
    print("\n=== VERDICT ===")
    print(json.dumps(results, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()