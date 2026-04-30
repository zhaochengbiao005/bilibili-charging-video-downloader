# gui_qt/pages/about.py
from PyQt6.QtWidgets import QDialog, QVBoxLayout, QLabel, QPushButton
from PyQt6.QtCore import Qt


class AboutDialog(QDialog):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("关于")
        self.setFixedSize(500, 400)
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.Dialog)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)

        # Semi-transparent glass background
        self.setStyleSheet("""
            AboutDialog {
                background: rgba(255,255,255,0.85);
                border: 1px solid white;
                border-radius: 32px;
            }
        """)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(40, 40, 40, 40)
        layout.setSpacing(16)
        layout.setAlignment(Qt.AlignmentFlag.AlignCenter)

        title = QLabel("BiliDownloader")
        title.setStyleSheet("font-size: 28px; font-weight: 900; color: #fb7299;")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(title)

        desc = QLabel("一个快速、安全、现代的 B站视频下载工具")
        desc.setWordWrap(True)
        desc.setStyleSheet("font-size: 14px; color: #666; font-weight: 500;")
        desc.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(desc)

        info = QLabel("v2.0.0 · PyQt6 · 仅供学习用途")
        info.setStyleSheet("font-size: 12px; color: #aaa;")
        info.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(info)

        layout.addStretch()

        close = QPushButton("关闭")
        close.setStyleSheet("""
            QPushButton {
                background: rgba(255,255,255,0.6);
                border: 1px solid rgba(255,255,255,0.8);
                border-radius: 16px;
                padding: 12px;
                font-weight: 700;
                font-size: 14px;
                color: #888;
            }
            QPushButton:hover { color: #fb7299; }
        """)
        close.clicked.connect(self.accept)
        layout.addWidget(close)
