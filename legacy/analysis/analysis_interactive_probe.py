# -*- coding: utf-8 -*-
"""Crawl Bilibili interactive (stein) video graph for BV1vb4y1r7cg."""
from __future__ import annotations

import json
import time
from collections import deque
from pathlib import Path

import requests

BVID = "BV1vb4y1r7cg"
FALLBACK_GV = 542689
FALLBACK_CID = 381841269

HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
        "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36"
    ),
    "Referer": f"https://www.bilibili.com/video/{BVID}",
    "Origin": "https://www.bilibili.com",
}
OUT = Path("analysis_BV1vb4y1r7cg")
OUT.mkdir(exist_ok=True)


def get_json(url: str, params: dict | None = None) -> dict:
    r = requests.get(url, params=params, headers=HEADERS, timeout=20)
    try:
        return r.json()
    except Exception:
        return {"_status": r.status_code, "_text": r.text[:400]}


def extract_choices(edges: dict) -> list[dict]:
    choices: list[dict] = []
    if not isinstance(edges, dict):
        return choices
    for c in edges.get("choices") or []:
        choices.append(c)
    for q in edges.get("questions") or []:
        for c in q.get("choices") or []:
            choices.append(c)
    return choices


def current_story(story_list: list) -> dict | None:
    if not story_list:
        return None
    for s in story_list:
        if s.get("cursor") == 1 or s.get("is_current"):
            return s
    return story_list[-1]


def main() -> None:
    view = get_json("https://api.bilibili.com/x/web-interface/view", {"bvid": BVID})
    (OUT / "01_view.json").write_text(
        json.dumps(view, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    view_data = view.get("data") or {}
    view_cid = ((view_data.get("pages") or [{}])[0]).get("cid") or FALLBACK_CID
    print(
        "title",
        view_data.get("title"),
        "stein",
        (view_data.get("rights") or {}).get("is_stein_gate"),
        "cid",
        view_cid,
        "duration",
        view_data.get("duration"),
        "pages",
        len(view_data.get("pages") or []),
    )

    pv = get_json(
        "https://api.bilibili.com/x/player/v2",
        {"bvid": BVID, "cid": view_cid},
    )
    (OUT / "02_player_v2.json").write_text(
        json.dumps(pv, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    interaction = (pv.get("data") or {}).get("interaction") or {}
    graph_version = interaction.get("graph_version") or FALLBACK_GV
    print("graph_version", graph_version, "msg", interaction.get("msg"))

    root = get_json(
        "https://api.bilibili.com/x/stein/edgeinfo_v2",
        {
            "bvid": BVID,
            "graph_version": graph_version,
            "portal": 0,
            "screen": 0,
        },
    )
    (OUT / "04_edgeinfo_v2.json").write_text(
        json.dumps(root, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    if root.get("code") != 0:
        raise SystemExit(f"edgeinfo root failed: {root}")

    data = root["data"]
    print("root title", data.get("title"), "edge_id", data.get("edge_id"))
    for s in (data.get("story_list") or [])[:8]:
        print(
            " story",
            {
                k: s.get(k)
                for k in ("node_id", "edge_id", "title", "cid", "cursor")
            },
        )
    edges = data.get("edges") or {}
    print("edges keys", list(edges.keys()) if isinstance(edges, dict) else edges)
    print("questions", json.dumps(edges.get("questions"), ensure_ascii=False)[:1200])
    print("preload", json.dumps(data.get("preload"), ensure_ascii=False)[:600])

    visited: set[int] = set()
    queue: deque[int | None] = deque([None])
    nodes: dict[int, dict] = {}
    raw_by_edge: dict[str, dict] = {}

    while queue and len(nodes) < 100:
        edge_id = queue.popleft()
        key = 0 if edge_id is None else int(edge_id)
        if key in visited:
            continue
        visited.add(key)

        params: dict = {
            "bvid": BVID,
            "graph_version": graph_version,
            "portal": 0,
            "screen": 0,
        }
        if edge_id is not None:
            params["edge_id"] = edge_id

        j = get_json("https://api.bilibili.com/x/stein/edgeinfo_v2", params)
        if j.get("code") != 0:
            print("fail", edge_id, j.get("code"), j.get("message"))
            continue

        d = j["data"]
        eid = int(d.get("edge_id") or key)
        story = d.get("story_list") or []
        cur = current_story(story)
        cid = (cur or {}).get("cid")
        node_id = (cur or {}).get("node_id")
        choices_raw = extract_choices(d.get("edges") or {})
        choice_rows = []
        for c in choices_raw:
            next_id = c.get("id")
            choice_rows.append(
                {
                    "id": next_id,
                    "option": c.get("option") or c.get("x") or c.get("text"),
                    "cid": c.get("cid"),
                    "condition": c.get("condition"),
                }
            )
            if next_id is not None and int(next_id) not in visited:
                queue.append(int(next_id))

        # also collect cids from preload video list
        preload_cids = []
        preload = d.get("preload") or {}
        for item in preload.get("video") or []:
            if item.get("cid"):
                preload_cids.append(item.get("cid"))

        nodes[eid] = {
            "edge_id": eid,
            "title": d.get("title"),
            "cid": cid,
            "node_id": node_id,
            "is_leaf": d.get("is_leaf"),
            "story_n": len(story),
            "preload_cids": preload_cids,
            "choices": choice_rows,
        }
        raw_by_edge[str(eid)] = {
            "title": d.get("title"),
            "edge_id": eid,
            "is_leaf": d.get("is_leaf"),
            "story_list": story,
            "edges": d.get("edges"),
            "preload": d.get("preload"),
            "hidden_vars": d.get("hidden_vars"),
        }
        print(
            f"edge={eid} title={d.get('title')!r} cid={cid} "
            f"leaf={d.get('is_leaf')} choices={len(choice_rows)} preload={preload_cids}"
        )
        time.sleep(0.12)

    unique_cids = sorted(
        {
            n["cid"]
            for n in nodes.values()
            if n.get("cid")
        }
        | {
            cid
            for n in nodes.values()
            for cid in (n.get("preload_cids") or [])
        }
    )
    print("TOTAL nodes", len(nodes), "unique cids", len(unique_cids))

    report = {
        "bvid": BVID,
        "aid": view_data.get("aid"),
        "graph_version": graph_version,
        "view_cid": view_cid,
        "node_count": len(nodes),
        "unique_cid_count": len(unique_cids),
        "unique_cids": unique_cids,
        "nodes": nodes,
    }
    (OUT / "05_graph_nodes.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    (OUT / "05_graph_raw.json").write_text(
        json.dumps(raw_by_edge, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    sample_cids = []
    if view_cid not in unique_cids:
        sample_cids.append(view_cid)
    sample_cids.extend(unique_cids[:8])
    play_summary = []
    for cid in sample_cids:
        pu = get_json(
            "https://api.bilibili.com/x/player/playurl",
            {
                "bvid": BVID,
                "cid": cid,
                "qn": 80,
                "fnval": 4048,
                "fnver": 0,
                "fourk": 1,
                "platform": "web",
            },
        )
        d = pu.get("data") or {}
        row = {
            "cid": cid,
            "code": pu.get("code"),
            "message": pu.get("message"),
            "quality": d.get("quality"),
            "timelength": d.get("timelength"),
            "has_dash": bool(d.get("dash")),
            "dash_v": len((d.get("dash") or {}).get("video") or []),
            "dash_a": len((d.get("dash") or {}).get("audio") or []),
            "durl_n": len(d.get("durl") or []),
            "durl_ms": sum(s.get("length", 0) for s in (d.get("durl") or [])),
        }
        play_summary.append(row)
        print("playurl", row)
        time.sleep(0.1)

    stein_try = get_json(
        "https://api.bilibili.com/x/stein/playurl",
        {
            "bvid": BVID,
            "cid": unique_cids[0] if unique_cids else view_cid,
            "qn": 80,
            "fnval": 16,
        },
    )
    print(
        "stein/playurl",
        stein_try.get("code"),
        stein_try.get("message"),
        list((stein_try.get("data") or {}).keys())[:10]
        if isinstance(stein_try.get("data"), dict)
        else None,
    )

    (OUT / "06_playurl_samples.json").write_text(
        json.dumps(
            {"samples": play_summary, "stein_playurl": stein_try},
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )

    strategy = {
        "type": "stein_gate_interactive",
        "sample": {
            "bvid": BVID,
            "title": view_data.get("title"),
            "graph_version": graph_version,
            "nodes_crawled": len(nodes),
            "unique_cids": len(unique_cids),
        },
        "detect": {
            "view.rights.is_stein_gate": (view_data.get("rights") or {}).get(
                "is_stein_gate"
            ),
            "player_v2.interaction.graph_version": graph_version,
            "view.pages_count": len(view_data.get("pages") or []),
            "view.duration_meta": view_data.get("duration"),
            "interaction.msg": interaction.get("msg"),
        },
        "apis": [
            "GET /x/web-interface/view?bvid= → rights.is_stein_gate, entry pages[0].cid",
            "GET /x/player/v2?bvid=&cid= → interaction.graph_version",
            "GET /x/stein/edgeinfo_v2?bvid=&graph_version=&edge_id= → story_list + choices",
            "GET /x/player/playurl?bvid=&cid=&fnval=4048 → segment media",
        ],
        "download_flow": [
            "detect stein",
            "read graph_version",
            "BFS edgeinfo_v2; choice.id = next edge_id",
            "collect unique cid from story_list / preload.video",
            "playurl each cid and download like normal video",
            "store as multi-file tree; optional path concat",
        ],
        "limitations": [
            "login may unlock full endings",
            "condition/hidden_vars may hide branches",
            "one BVID many cids; not single continuous file",
            "current app only downloads pages[0].cid intro",
        ],
    }
    (OUT / "00_interactive_report.json").write_text(
        json.dumps(strategy, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(strategy, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()