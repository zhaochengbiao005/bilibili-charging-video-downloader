#!/usr/bin/env python3
"""B站充电视频下载工具 - CLI 入口"""

import argparse
import json
import os
import sys
import time
from pathlib import Path

# 确保 src 在导入路径中
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "src"))

from bilibili_api import BilibiliAPI, APIError
from downloader import VideoDownloader, DownloadError
from merger import FFmpegMerger, FFmpegNotFoundError


def parse_cookie_file(path: str) -> dict:
    """解析 Netscape Cookie 文件或 JSON Cookie 文件"""
    if not os.path.exists(path):
        print(f"[!] Cookie file not found: {path}")
        sys.exit(1)

    cookies = {}

    with open(path, encoding="utf-8") as f:
        content = f.read().strip()

    # JSON 格式
    if content.startswith("{"):
        data = json.loads(content)
        cookies["SESSDATA"] = data.get("SESSDATA", "")
        cookies["bili_jct"] = data.get("bili_jct", "")
        cookies["DedeUserID"] = data.get("DedeUserID", "")
        return cookies

    # Netscape 格式 (每行: domain flag path secure expiry name value)
    for line in content.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) >= 7:
            name = parts[5]
            value = parts[6]
            if name in ("SESSDATA", "bili_jct", "DedeUserID"):
                cookies[name] = value

    return cookies


def download_video(
    bvid: str,
    output_dir: str,
    cookie_path: str = None,
    qn: int = 80,
    max_workers: int = 4,
    skip_merge: bool = False,
):
    """主下载流程"""
    # 初始化客户端
    cookies = {}
    if cookie_path:
        cookies = parse_cookie_file(cookie_path)

    api = BilibiliAPI(
        sessdata=cookies.get("SESSDATA", ""),
        bili_jct=cookies.get("bili_jct", ""),
        dedeuserid=cookies.get("DedeUserID", ""),
    )

    # 1. 获取视频信息
    print(f"[*] Fetching video info: {bvid}")
    try:
        info = api.get_video_info(bvid)
        title = info.get("title", bvid)
        print(f"    Title: {title}")
        pages = info.get("pages", [])
        print(f"    Pages: {len(pages)}")
    except APIError as e:
        print(f"[!] Failed to get video info: {e}")
        sys.exit(1)

    # 2. 检查充电状态
    charging_status = api.check_charging_status(bvid)
    if charging_status.get("is_charging"):
        print("    [!] This is a charging-only video. Cookie auth required.")
    elif charging_status.get("need_pay"):
        print("    [!] This is a paid video. Cookie auth required.")

    # 3. 创建输出目录
    safe_title = "".join(c if c.isalnum() or c in (" ", "_", "-") else "_" for c in title)
    video_dir = os.path.join(output_dir, f"{bvid}_{safe_title}")
    os.makedirs(video_dir, exist_ok=True)

    # 4. 对每个分P获取播放地址并下载
    downloader = VideoDownloader(max_workers=max_workers)

    for i, page in enumerate(pages):
        cid = page["cid"]
        part_title = page.get("part", f"P{page['page']}")
        print(f"\n[*] Processing P{page['page']}: {part_title} (cid={cid})")

        try:
            playurl_data = api.get_playurl(bvid, cid, qn=qn)
        except APIError as e:
            print(f"    [!] Failed to get playurl (cid={cid}): {e}")
            print("    [!] This video may require valid Cookie for charging video access")
            continue

        # 尝试 DASH 流
        dash = api.extract_dash_urls(playurl_data)
        if dash["video"] and dash["audio"]:
            print(f"    DASH streams available:")
            # 选择最高码率视频流
            best_video = max(dash["video"], key=lambda v: v.get("bandwidth", 0))
            best_audio = max(dash["audio"], key=lambda v: v.get("bandwidth", 0))
            print(f"      Video: {best_video['width']}x{best_video['height']} "
                  f"@{best_video['codecs']} ({best_video['bandwidth']}bps)")
            print(f"      Audio: {best_audio['codecs']} ({best_audio['bandwidth']}bps)")

            video_out = os.path.join(video_dir, f"P{page['page']}_video.m4s")
            audio_out = os.path.join(video_dir, f"P{page['page']}_audio.m4s")

            print("    Downloading video stream...")
            downloader.download(best_video["base_url"], video_out)
            print("    Downloading audio stream...")
            downloader.download(best_audio["base_url"], audio_out)

            if not skip_merge:
                merger = FFmpegMerger()
                if merger.check_available():
                    output_file = os.path.join(video_dir, f"P{page['page']}_{safe_title}.mp4")
                    print(f"    Merging to {output_file}...")
                    merger.merge_video_audio(video_out, audio_out, output_file)
                    print(f"    [OK] Saved to {output_file}")
                    # 清理临时文件
                    os.remove(video_out)
                    os.remove(audio_out)
                else:
                    print(f"    [OK] Video: {video_out}")
                    print(f"    [OK] Audio: {audio_out}")
                    print("    [*] Install FFmpeg to auto-merge: ffmpeg -i "
                          f"{os.path.basename(video_out)} -i {os.path.basename(audio_out)} "
                          f"-c copy P{page['page']}.mp4")

        # 回退到 DURL (FLV) 流
        durl = api.extract_durl_urls(playurl_data)
        if durl:
            print(f"    DURL segments: {len(durl)}")
            output_file = os.path.join(video_dir, f"P{page['page']}_{safe_title}.flv")
            downloader.download_with_concat(durl, output_file)
            print(f"    [OK] Saved to {output_file}")

        if not dash["video"] and not durl:
            print("    [!] No playable streams found (charging video without valid cookie?)")

    print(f"\n[*] All done. Files saved to: {video_dir}")


def list_streams(bvid: str, cookie_path: str = None, qn: int = 80):
    """列出视频的所有可用流信息（不下载）"""
    cookies = {}
    if cookie_path:
        cookies = parse_cookie_file(cookie_path)

    api = BilibiliAPI(
        sessdata=cookies.get("SESSDATA", ""),
        bili_jct=cookies.get("bili_jct", ""),
        dedeuserid=cookies.get("DedeUserID", ""),
    )

    info = api.get_video_info(bvid)
    print(f"\nVideo: {info.get('title', bvid)}")
    print(f"Description: {info.get('desc', '')[:200]}")
    print(f"Duration: {info.get('duration', 0)}s")
    print(f"Pages: {len(info.get('pages', []))}")

    pages = api.get_cid_by_bvid(bvid)
    for p in pages:
        print(f"\n  P{p['page']}: {p['title']} (cid={p['cid']}, {p['duration']}s)")
        try:
            playurl_data = api.get_playurl(bvid, p["cid"], qn=qn)
            dash = api.extract_dash_urls(playurl_data)
            for v in dash["video"]:
                print(f"    [V] {v['width']}x{v['height']} ~ {v['codecs']} {v['bandwidth']}bps")
            for a in dash["audio"]:
                print(f"    [A] {a['codecs']} {a['bandwidth']}bps")
            durl = api.extract_durl_urls(playurl_data)
            if durl:
                for s in durl:
                    print(f"    [S] Segment {s['order']}: {s['size']} bytes")
        except APIError as e:
            print(f"    [!] {e}")


def main():
    parser = argparse.ArgumentParser(
        description="B站视频下载工具",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # 查看视频可用的流
  python main.py info BV1xx411c7mD

  # 下载视频（需要 Cookie 文件）
  python main.py dl BV1xx411c7mD -c cookies.txt

  # 指定画质和并发数
  python main.py dl BV1xx411c7mD -c cookies.json -q 112 -w 8

  # 仅下载不合并
  python main.py dl BV1xx411c7mD -c cookies.txt --skip-merge
        """,
    )
    subparsers = parser.add_subparsers(dest="command", help="sub-command")

    # info 子命令
    info_parser = subparsers.add_parser("info", help="查看视频流信息")
    info_parser.add_argument("bvid", help="视频 BVID (BV号)")
    info_parser.add_argument("-c", "--cookie", help="Cookie 文件路径（JSON或Netscape格式）")
    info_parser.add_argument("-q", "--quality", type=int, default=80, help="画质 (default: 80=1080P)")

    # dl 子命令
    dl_parser = subparsers.add_parser("dl", help="下载视频")
    dl_parser.add_argument("bvid", help="视频 BVID (BV号)")
    dl_parser.add_argument("-c", "--cookie", help="Cookie 文件路径（JSON或Netscape格式）")
    dl_parser.add_argument("-o", "--output", default="downloads", help="输出目录 (default: downloads)")
    dl_parser.add_argument("-q", "--quality", type=int, default=80, help="画质 (default: 80=1080P)")
    dl_parser.add_argument("-w", "--workers", type=int, default=4, help="下载并发数 (default: 4)")
    dl_parser.add_argument("--skip-merge", action="store_true", help="不合并音视频，保留原始文件")

    args = parser.parse_args()

    if args.command == "info":
        list_streams(args.bvid, args.cookie, args.quality)
    elif args.command == "dl":
        download_video(
            bvid=args.bvid,
            output_dir=args.output,
            cookie_path=args.cookie,
            qn=args.quality,
            max_workers=args.workers,
            skip_merge=args.skip_merge,
        )
    else:
        parser.print_help()


def _is_frozen() -> bool:
    """判断是否运行在打包的 exe 中"""
    return getattr(sys, "frozen", False)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n[!] Interrupted by user.")
    except Exception as e:
        print(f"\n[!] Error: {e}")
    finally:
        if _is_frozen():
            input("\nPress Enter to exit...")
