# -*- coding: utf-8 -*-
"""Probe Bilibili upower/charging video APIs for BV159dABuEyd."""
import json
import time
from pathlib import Path
from urllib.parse import parse_qs, urlparse

import requests

BVID = "BV159dABuEyd"
CID = 38167710052
AID = 116533938360751
UP_MID = 11186360

HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/125.0.0.0 Safari/537.36"
    ),
    "Referer": f"https://www.bilibili.com/video/{BVID}",
    "Origin": "https://www.bilibili.com",
}

OUT = Path("analysis_BV159dABuEyd")
OUT.mkdir(exist_ok=True)


def save(name: str, obj) -> None:
    path = OUT / name
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=2), encoding="utf-8")
    print("saved", path)


def main() -> None:
    view = requests.get(
        "https://api.bilibili.com/x/web-interface/view",
        params={"bvid": BVID},
        headers=HEADERS,
        timeout=20,
    ).json()
    save("01_view.json", view)

    pv = requests.get(
        "https://api.bilibili.com/x/player/v2",
        params={"bvid": BVID, "cid": CID},
        headers=HEADERS,
        timeout=20,
    ).json()
    save("02_player_v2.json", pv)

    variants = [
        {"fnval": 0, "qn": 80, "platform": "web", "name": "fnval0_durl"},
        {"fnval": 16, "qn": 80, "platform": "web", "name": "fnval16_dash"},
        {"fnval": 4048, "qn": 80, "platform": "web", "name": "fnval4048"},
        {"fnval": 4048, "qn": 16, "platform": "html5", "name": "html5_fnval4048"},
        {"fnval": 16, "qn": 64, "platform": "html5", "name": "html5_fnval16"},
        {"fnval": 4048, "qn": 112, "platform": "web", "name": "fnval4048_qn112"},
        {"fnval": 80, "qn": 80, "platform": "pc", "name": "pc_fnval80"},
    ]
    summary = []
    for v in variants:
        params = {
            "bvid": BVID,
            "cid": CID,
            "qn": v["qn"],
            "platform": v["platform"],
            "fnval": v["fnval"],
            "fnver": 0,
            "fourk": 1,
            "force_host": 2,
            "client_ts": int(time.time()),
        }
        data = requests.get(
            "https://api.bilibili.com/x/player/playurl",
            params=params,
            headers=HEADERS,
            timeout=20,
        ).json()
        save(f"03_playurl_{v['name']}.json", data)
        d = data.get("data") or {}
        row = {
            "name": v["name"],
            "code": data.get("code"),
            "quality": d.get("quality"),
            "format": d.get("format"),
            "timelength": d.get("timelength"),
            "accept_quality": d.get("accept_quality"),
            "has_dash": bool(d.get("dash")),
            "has_durl": bool(d.get("durl")),
            "durl_ms": sum(s.get("length", 0) for s in (d.get("durl") or [])),
            "durl_size": sum(s.get("size", 0) for s in (d.get("durl") or [])),
            "dash_duration": (d.get("dash") or {}).get("duration"),
            "dash_v": len((d.get("dash") or {}).get("video") or []),
            "dash_a": len((d.get("dash") or {}).get("audio") or []),
        }
        if d.get("durl"):
            u = d["durl"][0].get("url", "")
            row["url_host"] = u.split("/")[2] if u.startswith("http") else ""
            qs = parse_qs(urlparse(u).query)
            row["url_qkeys"] = sorted(qs.keys())
            # keep only non-secret-looking sample values length
            row["url_q_meta"] = {
                k: (qs[k][0][:40] if k in ("deadline", "gen", "os", "oi", "trid", "nbs") else len(qs[k][0]))
                for k in qs
            }
        if d.get("dash") and (d["dash"].get("video") or []):
            vu = d["dash"]["video"][0].get("base_url", "")
            row["dash_host"] = vu.split("/")[2] if vu.startswith("http") else ""
            row["dash_ids"] = [t.get("id") for t in d["dash"]["video"][:8]]
        summary.append(row)
        print(json.dumps(row, ensure_ascii=False))

    save("00_summary.json", summary)

    endpoints = [
        ("https://api.bilibili.com/x/ugcpay-rank/elec/month/up", {"up_mid": UP_MID}),
        ("https://api.bilibili.com/x/web-interface/archive/relation", {"bvid": BVID}),
        (
            "https://api.bilibili.com/x/player/wbi/playurl",
            {
                "bvid": BVID,
                "cid": CID,
                "qn": 80,
                "fnval": 4048,
                "fnver": 0,
                "fourk": 1,
            },
        ),
        ("https://api.bilibili.com/x/web-interface/wbi/view", {"bvid": BVID}),
        (
            "https://api.bilibili.com/x/polymer/web-space/v2/upower/rank",
            {"mid": UP_MID},
        ),
        (
            "https://api.bilibili.com/x/vas/upower/archive/info",
            {"aid": AID, "bvid": BVID},
        ),
        (
            "https://api.bilibili.com/x/upower/v2/archive/info",
            {"aid": AID},
        ),
    ]
    for i, (url, params) in enumerate(endpoints):
        try:
            r = requests.get(url, params=params, headers=HEADERS, timeout=15)
            try:
                j = r.json()
            except Exception:
                j = {"status": r.status_code, "text": r.text[:800]}
            save(f"04_extra_{i}.json", {"url": url, "params": params, "resp": j})
            code = j.get("code") if isinstance(j, dict) else r.status_code
            print("extra", i, code, url)
        except Exception as e:
            print("extra fail", i, e)

    # CDN access control
    pu = requests.get(
        "https://api.bilibili.com/x/player/playurl",
        params={
            "bvid": BVID,
            "cid": CID,
            "qn": 64,
            "platform": "web",
            "fnval": 0,
            "fnver": 0,
        },
        headers=HEADERS,
        timeout=20,
    ).json()
    url = pu["data"]["durl"][0]["url"]
    for label, h in [
        ("no_referer", {"User-Agent": HEADERS["User-Agent"]}),
        ("with_referer", HEADERS),
        (
            "bad_referer",
            {
                "User-Agent": HEADERS["User-Agent"],
                "Referer": "https://example.com/",
            },
        ),
    ]:
        try:
            rr = requests.get(url, headers=h, timeout=20, stream=True)
            chunk = next(rr.iter_content(32), b"")
            print(
                label,
                "status",
                rr.status_code,
                "ctype",
                rr.headers.get("content-type"),
                "clen",
                rr.headers.get("content-length"),
                "magic",
                chunk[:12].hex(),
            )
            rr.close()
        except Exception as e:
            print(label, "err", e)

    d = view["data"]
    pvd = pv["data"]
    report = {
        "bvid": BVID,
        "title": d.get("title"),
        "owner": d.get("owner"),
        "duration_meta_sec": d.get("duration"),
        "dimension": (d.get("pages") or [{}])[0].get("dimension"),
        "rights": d.get("rights"),
        "upower_flags": {
            "is_upower_exclusive": d.get("is_upower_exclusive"),
            "is_upower_play": d.get("is_upower_play"),
            "is_upower_preview": d.get("is_upower_preview"),
            "is_upower_exclusive_with_qa": d.get("is_upower_exclusive_with_qa"),
        },
        "player_v2": {
            "is_upower_exclusive": pvd.get("is_upower_exclusive"),
            "is_upower_play": pvd.get("is_upower_play"),
            "is_ugc_pay_preview": pvd.get("is_ugc_pay_preview"),
            "preview_toast": pvd.get("preview_toast"),
            "elec_high_level": pvd.get("elec_high_level"),
            "options": pvd.get("options"),
        },
        "playurl_summary": summary,
        "detection_gap": {
            "rights_elec": (d.get("rights") or {}).get("elec"),
            "rights_elec_high_present": "elec_high" in (d.get("rights") or {}),
            "project_uses_rights_elec_high": True,
            "actual_signal": "is_upower_exclusive / is_upower_preview / playurl durl length",
        },
    }
    save("00_report.json", report)
    print("TITLE", d.get("title"))
    print("OWNER", d.get("owner", {}).get("name"))
    print("TOAST", pvd.get("preview_toast"))
    print("ELEC_HIGH", json.dumps(pvd.get("elec_high_level"), ensure_ascii=False))


if __name__ == "__main__":
    main()