from PyQt6.QtWidgets import (QWidget, QVBoxLayout, QLabel, QPushButton, QComboBox,
                             QLineEdit, QCheckBox, QHBoxLayout, QScrollArea, QFrame)
from PyQt6.QtCore import Qt
from gui_qt.glass_panel import GlassPanel


import sys
import os
import json

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", ".."))
from src.merger import FFmpegMerger

DATA_DIR = os.path.join(os.path.dirname(__file__), "..", "..", "data")


def _load_config():
    p = os.path.join(DATA_DIR, "config.json")
    try:
        with open(p) as f:
            return json.load(f)
    except Exception:
        return {}


def _save_config(cfg):
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(os.path.join(DATA_DIR, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)


class SettingsPage(QWidget):
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
        h = QLabel("⚙️ 设置")
        h.setStyleSheet("font-size: 28px; font-weight: 900; color: #222;")
        layout.addWidget(h)

        # ── FFmpeg status ──
        ffmpeg_panel = GlassPanel()
        ffmpeg_header = QLabel("📦 FFmpeg 状态")
        ffmpeg_header.setStyleSheet("font-size: 18px; font-weight: 900; color: #333;")
        ffmpeg_panel.layout().addWidget(ffmpeg_header)

        self.ffmpeg_status = QLabel("检查中...")
        self.ffmpeg_status.setStyleSheet("font-size: 14px; color: #888;")
        ffmpeg_panel.layout().addWidget(self.ffmpeg_status)

        self.ffmpeg_btn = QPushButton("安装 FFmpeg")
        self.ffmpeg_btn.setStyleSheet("""
            QPushButton {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #fb7299, stop:1 #ff85a8);
                color: white; border: none; border-radius: 12px;
                padding: 10px 24px; font-weight: 700; font-size: 13px;
                max-width: 200px;
            }
            QPushButton:hover { opacity: 0.8; }
        """)
        ffmpeg_panel.layout().addWidget(self.ffmpeg_btn)

        layout.addWidget(ffmpeg_panel)

        # ── Download defaults ──
        defaults = GlassPanel()
        d_header = QLabel("⬇ 默认下载设置")
        d_header.setStyleSheet("font-size: 18px; font-weight: 900; color: #333;")
        defaults.layout().addWidget(d_header)

        defaults.layout().addWidget(QLabel("默认画质"))
        self.quality_combo = QComboBox()
        self.quality_combo.addItems(["360P", "480P", "720P", "1080P", "1080P60", "4K", "HDR"])
        defaults.layout().addWidget(self.quality_combo)

        defaults.layout().addWidget(QLabel("下载速度"))
        self.speed_combo = QComboBox()
        self.speed_combo.addItems(["慢速 (4线程)", "标准 (8线程)", "快速 (16线程)", "极速 (32线程)"])
        defaults.layout().addWidget(self.speed_combo)

        defaults.layout().addWidget(QLabel("输出目录"))
        dir_row = QHBoxLayout()
        self.outdir = QLineEdit("downloads")
        dir_row.addWidget(self.outdir)
        browse = QPushButton("浏览")
        browse.setStyleSheet("""
            QPushButton {
                background: rgba(255,255,255,0.6);
                border: 1px solid rgba(255,255,255,0.8);
                border-radius: 10px;
                padding: 8px 16px; font-weight: 700; font-size: 12px; color: #888;
            }
            QPushButton:hover { color: #fb7299; }
        """)
        dir_row.addWidget(browse)
        defaults.layout().addLayout(dir_row)

        self.auto_merge = QCheckBox("自动合并音视频（需要 FFmpeg）")
        self.auto_merge.setChecked(True)
        defaults.layout().addWidget(self.auto_merge)

        # Save button
        save = QPushButton("保存设置")
        save.setStyleSheet("""
            QPushButton {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #fb7299, stop:1 #ff85a8);
                color: white; border: none; border-radius: 16px;
                padding: 12px 32px; font-weight: 700; font-size: 14px;
            }
            QPushButton:hover { opacity: 0.8; }
        """)
        save.setFixedWidth(160)
        save_row = QHBoxLayout()
        save_row.addStretch()
        save_row.addWidget(save)
        defaults.layout().addLayout(save_row)

        layout.addWidget(defaults)

        # Load config
        cfg = _load_config()
        if cfg.get("default_quality"):
            idx = self.quality_combo.findText(cfg["default_quality"])
            if idx >= 0:
                self.quality_combo.setCurrentIndex(idx)
        if cfg.get("default_speed"):
            idx = self.speed_combo.findText(cfg["default_speed"])
            if idx >= 0:
                self.speed_combo.setCurrentIndex(idx)
        if cfg.get("default_outdir"):
            self.outdir.setText(cfg["default_outdir"])
        self.auto_merge.setChecked(cfg.get("auto_merge", True))

        # Check FFmpeg
        if FFmpegMerger().check_available():
            self.ffmpeg_status.setText("✅ FFmpeg 已安装")
            self.ffmpeg_btn.hide()
        else:
            self.ffmpeg_status.setText("❌ FFmpeg 未安装")

        # Connect buttons
        self.ffmpeg_btn.clicked.connect(self._install_ffmpeg)
        save.clicked.connect(self._save)

        # Browse button for output dir
        browse.clicked.connect(self._browse_outdir)

        # ── Data info ──
        info = GlassPanel()
        info.setStyleSheet("""
            background: rgba(240, 245, 255, 0.6);
            border: 1px solid rgba(200, 215, 240, 0.5);
            border-radius: 16px;
        """)
        i_header = QLabel("📁 数据存储说明")
        i_header.setStyleSheet("font-size: 18px; font-weight: 900; color: #333;")
        info.layout().addWidget(i_header)
        info_desc = QLabel(
            "程序配置、Cookie 和下载历史保存在 data/ 目录中。\n"
            "下载的视频保存在输出目录，卸载程序时视频文件不会被删除。"
        )
        info_desc.setStyleSheet("font-size: 13px; color: #667; line-height: 1.6;")
        info.layout().addWidget(info_desc)

        layout.addWidget(info)
        layout.addStretch()

        scroll.setWidget(content)

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.addWidget(scroll)

    def _install_ffmpeg(self):
        def task():
            try:
                merger = FFmpegMerger()
                merger.auto_download()
                self.ffmpeg_status.setText("✅ FFmpeg 已安装")
                self.ffmpeg_btn.hide()
            except Exception as e:
                self.ffmpeg_status.setText(f"❌ 安装失败: {e}")

        import threading

        threading.Thread(target=task, daemon=True).start()

    def _save(self):
        _save_config(
            {
                "default_quality": self.quality_combo.currentText(),
                "default_speed": self.speed_combo.currentText(),
                "default_outdir": self.outdir.text(),
                "auto_merge": self.auto_merge.isChecked(),
            }
        )

    def _browse_outdir(self):
        from PyQt6.QtWidgets import QFileDialog

        path = QFileDialog.getExistingDirectory(self, "选择输出目录")
        if path:
            self.outdir.setText(path)
