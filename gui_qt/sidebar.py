# gui_qt/sidebar.py
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QListWidget, QPushButton, QFrame
from PyQt6.QtCore import Qt

NAV_ITEMS = ["🏠 首页", "📋 下载历史", "⚙️ 设置", "ℹ️ 关于"]

class Sidebar(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("sidebar")
        self.setFixedWidth(240)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(16, 24, 16, 24)
        layout.setSpacing(8)

        # Logo area
        logo = QWidget()
        logo.setFixedHeight(64)
        logo_layout = QVBoxLayout(logo)
        logo_layout.setContentsMargins(8, 0, 8, 0)

        title = QLabel("BiliDownloader")
        title.setStyleSheet("font-size: 20px; font-weight: 900; color: #fb7299;")
        subtitle = QLabel("视频下载工具")
        subtitle.setStyleSheet("font-size: 11px; color: #999; font-weight: 500;")

        logo_layout.addWidget(title)
        logo_layout.addWidget(subtitle)
        layout.addWidget(logo)

        # Navigation list
        nav_container = QWidget()
        nav_container.setObjectName("sidebar-nav")
        nav_layout = QVBoxLayout(nav_container)
        nav_layout.setContentsMargins(0, 16, 0, 0)

        self.nav_list = QListWidget()
        self.nav_list.setFrameShape(QFrame.Shape.NoFrame)
        for item in NAV_ITEMS:
            self.nav_list.addItem(item)
        self.nav_list.setCurrentRow(0)
        nav_layout.addWidget(self.nav_list)

        layout.addWidget(nav_container, 1)

        # Support button at bottom
        support = QPushButton("❤️ 支持项目")
        support.setStyleSheet("""
            QPushButton {
                background: rgba(255, 242, 245, 0.5);
                border: 1px solid rgba(255, 200, 210, 0.5);
                border-radius: 16px;
                padding: 12px;
                font-weight: 700;
                font-size: 13px;
                color: #fb7299;
            }
            QPushButton:hover {
                background: rgba(255, 242, 245, 0.8);
            }
        """)
        support.setCursor(Qt.CursorShape.PointingHandCursor)
        layout.addWidget(support)
