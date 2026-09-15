from PyQt6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QScrollArea, QFrame
from PyQt6.QtCore import Qt
from gui_qt.glass_panel import GlassPanel
from gui_qt.widgets.url_input import UrlInput
from gui_qt.widgets.cookie_input import CookieInput
from gui_qt.widgets.video_info import VideoInfo
from gui_qt.widgets.download_options import DownloadOptions
from gui_qt.widgets.download_queue import DownloadQueue
import sys
import os
import re
import threading

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", ".."))
from src.bilibili_api import BilibiliAPI, APIError
from src.downloader import VideoDownloader, DownloadError


class HomePage(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)

        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QFrame.Shape.NoFrame)
        scroll.setStyleSheet("background: transparent;")
        scroll.viewport().setStyleSheet("background: transparent;")

        content = QWidget()
        content.setStyleSheet("background: transparent;")
        layout = QVBoxLayout(content)
        layout.setContentsMargins(40, 40, 40, 40)
        layout.setSpacing(24)

        # Header
        header = QWidget()
        hl = QVBoxLayout(header)
        hl.setContentsMargins(0, 0, 0, 0)
        hl.setSpacing(8)

        title = QLabel("下载你喜欢的视频")
        title.setStyleSheet("font-size: 36px; font-weight: 900; color: #222;")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        hl.addWidget(title)

        subtitle = QLabel("粘贴 B站视频链接，快速解析并下载高清视频与音频")
        subtitle.setStyleSheet("font-size: 16px; color: #888; font-weight: 500;")
        subtitle.setAlignment(Qt.AlignmentFlag.AlignCenter)
        hl.addWidget(subtitle)

        layout.addWidget(header)

        # Search bar (glass panel, pill-shaped)
        search_panel = GlassPanel()
        search_panel.layout().setContentsMargins(16, 8, 16, 8)
        search_panel.setMaximumWidth(800)
        search_panel.setStyleSheet(search_panel.styleSheet() + "border-radius: 48px;")

        self.url_input = UrlInput()
        search_panel.layout().addWidget(self.url_input)

        self.cookie_input = CookieInput()
        search_panel.layout().addWidget(self.cookie_input)

        # Center the search bar with stretch on both sides
        search_wrap = QWidget()
        search_wrap.setStyleSheet("background: transparent;")
        swl = QHBoxLayout(search_wrap)
        swl.setContentsMargins(0, 0, 0, 0)
        swl.addStretch()
        swl.addWidget(search_panel)
        swl.addStretch()
        layout.addWidget(search_wrap)

        # Content area: video info + download options side by side
        content_row = QWidget()
        content_row.setStyleSheet("background: transparent;")
        crl = QHBoxLayout(content_row)
        crl.setContentsMargins(0, 0, 0, 0)
        crl.setSpacing(24)

        self.video_info = VideoInfo()
        crl.addWidget(self.video_info, 1)

        right_col = QWidget()
        right_col.setStyleSheet("background: transparent;")
        rcl = QVBoxLayout(right_col)
        rcl.setContentsMargins(0, 0, 0, 0)
        rcl.setSpacing(16)

        self.dl_options = DownloadOptions()
        rcl.addWidget(self.dl_options)

        self.dl_queue = DownloadQueue()
        self.url_input.parse_btn.clicked.connect(self._fetch_info)
        self.dl_options.download_clicked.connect(self._start_download)
        rcl.addWidget(self.dl_queue, 1)

        crl.addWidget(right_col, 1)
        layout.addWidget(content_row, 1)

        scroll.setWidget(content)

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.addWidget(scroll)

    def _extract_bvid(self, text: str) -> str | None:
        m = re.search(r"BV\w{10,}", text.strip() or "")
        return m.group(0) if m else None

    def _fetch_info(self):
        bvid = self._extract_bvid(self.url_input.get_url())
        if not bvid:
            return
        self.url_input.parse_btn.setEnabled(False)
        self.url_input.parse_btn.setText("解析中...")

        def fetch():
            try:
                api = BilibiliAPI()
                info = api.get_video_info(bvid)
                self.video_info.set_data({
                    "id": bvid,
                    "title": info.get("title", ""),
                    "author": info.get("owner", {}).get("name", ""),
                    "thumbnail": info.get("pic", ""),
                    "views": str(info.get("stat", {}).get("view", 0)),
                    "duration": str(info.get("duration", 0)),
                    "duration_sec": info.get("duration", 0),
                    "pages": [
                        {"cid": p["cid"], "page": p["page"], "part": p.get("part", "")}
                        for p in info.get("pages", [])
                    ],
                    "qualities": ["360P", "480P", "720P", "1080P"],
                    "is_charging": info.get("rights", {}).get("elec_high", 0) == 1
                    or info.get("elec", 0) == 1,
                    "is_vip": False,
                    "vip_type": 0,
                    "is_login": False,
                    "desc": info.get("desc", "")[:300],
                })
            except APIError as e:
                print(f"API Error: {e}")
            except Exception as e:
                print(f"Error: {e}")
            finally:
                self.url_input.parse_btn.setEnabled(True)
                self.url_input.parse_btn.setText("解析 →")

        threading.Thread(target=fetch, daemon=True).start()

    def _start_download(self):
        bvid = self._extract_bvid(self.url_input.get_url())
        if not bvid:
            return
        quality = self.dl_options.get_selected_quality()
        qn = {
            "360P": 16,
            "480P": 32,
            "720P": 64,
            "1080P": 80,
            "1080P60": 116,
            "4K": 120,
            "HDR": 125,
        }.get(quality, 80)
        outdir = "downloads"

        widget = self.dl_queue.add_task(f"正在处理 {bvid}...", quality, "video")

        def dl():
            try:
                api = BilibiliAPI()
                info = api.get_video_info(bvid)
                pages = info.get("pages", [])
                if not pages:
                    return
                cid = pages[0]["cid"]
                playurl = api.get_playurl(bvid, cid, qn=qn)
                dash = api.extract_dash_urls(playurl)

                if dash["video"] and dash["audio"]:
                    bv = max(dash["video"], key=lambda v: v.get("bandwidth", 0))
                    ba = max(dash["audio"], key=lambda v: v.get("bandwidth", 0))
                    dl_engine = VideoDownloader(max_workers=4)

                    v_out = os.path.join(outdir, f"{bvid}_video.m4s")
                    a_out = os.path.join(outdir, f"{bvid}_audio.m4s")
                    os.makedirs(outdir, exist_ok=True)

                    dl_engine.download(bv["base_url"], v_out)
                    widget.set_progress(50)
                    dl_engine.download(ba["base_url"], a_out)
                    widget.set_progress(100)

                    import shutil
                    from src.merger import FFmpegMerger

                    merger = FFmpegMerger()
                    if merger.check_available():
                        mp4_path = os.path.join(outdir, f"{bvid}.mp4")
                        merger.merge_video_audio(v_out, a_out, mp4_path)
                        os.remove(v_out)
                        os.remove(a_out)
            except Exception as e:
                print(f"Download error: {e}")

        threading.Thread(target=dl, daemon=True).start()
