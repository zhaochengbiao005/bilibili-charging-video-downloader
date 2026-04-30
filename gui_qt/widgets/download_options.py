from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QPushButton, QButtonGroup, QRadioButton, QHBoxLayout
from PyQt6.QtCore import Qt, pyqtSignal
from gui_qt.glass_panel import GlassPanel

QUALITY_LABELS = ["360P", "480P", "720P", "1080P", "1080P60", "4K", "HDR"]


class DownloadOptions(GlassPanel):
    download_clicked = pyqtSignal()

    def __init__(self, parent=None):
        super().__init__(parent)

        header = QLabel("⚙️ 下载设置")
        header.setStyleSheet("font-size: 20px; font-weight: 900; color: #222;")
        self.layout().addWidget(header)

        # Format toggle
        fmt_label = QLabel("格式")
        fmt_label.setStyleSheet("font-size: 13px; font-weight: 700; color: #888;")
        self.layout().addWidget(fmt_label)

        fmt_row = QHBoxLayout()
        self.video_btn = QPushButton("\U0001f3ac 视频 (MP4)")
        self.video_btn.setCheckable(True)
        self.video_btn.setChecked(True)
        self.video_btn.setStyleSheet(self._fmt_btn_style(True))

        self.audio_btn = QPushButton("\U0001f3b5 音频 (MP3)")
        self.audio_btn.setCheckable(True)
        self.audio_btn.setStyleSheet(self._fmt_btn_style(False))

        self.video_btn.clicked.connect(lambda: self._switch_fmt("video"))
        self.audio_btn.clicked.connect(lambda: self._switch_fmt("audio"))

        fmt_row.addWidget(self.video_btn)
        fmt_row.addWidget(self.audio_btn)
        self.layout().addLayout(fmt_row)

        # Quality list
        ql = QLabel("画质")
        ql.setStyleSheet(
            "font-size: 13px; font-weight: 700; color: #888; margin-top: 8px;"
        )
        self.layout().addWidget(ql)

        self.quality_group = QButtonGroup(self)
        for i, q in enumerate(QUALITY_LABELS):
            rb = QRadioButton(q)
            rb.setStyleSheet("""
                QRadioButton {
                    spacing: 8px;
                    font-weight: 700;
                    font-size: 13px;
                    color: #444;
                    padding: 12px 16px;
                    background: rgba(255,255,255,0.4);
                    border: 2px solid rgba(255,255,255,0.6);
                    border-radius: 16px;
                }
                QRadioButton:checked {
                    border: 2px solid #fb7299;
                    background: rgba(251,114,153,0.06);
                    color: #fb7299;
                }
                QRadioButton:hover {
                    border: 2px solid rgba(251,114,153,0.3);
                }
            """)
            self.quality_group.addButton(rb)
            self.layout().addWidget(rb)

        # Select 1080P by default (index 3)
        if self.quality_group.buttons():
            self.quality_group.buttons()[3].setChecked(True)

        # Download button
        self.dl_btn = QPushButton("⬇ 开始下载")
        self.dl_btn.setStyleSheet("""
            QPushButton {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #fb7299, stop:1 #ff85a8);
                color: white;
                border: none;
                border-radius: 16px;
                padding: 16px;
                font-weight: 900;
                font-size: 16px;
                margin-top: 16px;
            }
            QPushButton:hover {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #ff85a8, stop:1 #ff95b5);
            }
        """)
        self.dl_btn.clicked.connect(self.download_clicked.emit)
        self.layout().addWidget(self.dl_btn)

    def _fmt_btn_style(self, active: bool) -> str:
        if active:
            return """
                QPushButton {
                    background: rgba(251,114,153,0.06);
                    border: 2px solid #fb7299;
                    border-radius: 16px;
                    padding: 12px;
                    font-weight: 900;
                    font-size: 13px;
                    color: #fb7299;
                }
            """
        return """
            QPushButton {
                background: rgba(255,255,255,0.5);
                border: 2px solid rgba(255,255,255,0.8);
                border-radius: 16px;
                padding: 12px;
                font-weight: 900;
                font-size: 13px;
                color: #888;
            }
            QPushButton:hover { border: 2px solid rgba(251,114,153,0.3); }
        """

    def _switch_fmt(self, fmt: str):
        active = fmt == "video"
        self.video_btn.setChecked(active)
        self.audio_btn.setChecked(not active)
        self.video_btn.setStyleSheet(self._fmt_btn_style(active))
        self.audio_btn.setStyleSheet(self._fmt_btn_style(not active))

    def get_selected_quality(self) -> str:
        btn = self.quality_group.checkedButton()
        return btn.text() if btn else "1080P"
