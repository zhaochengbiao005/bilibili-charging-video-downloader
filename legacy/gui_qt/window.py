# gui_qt/window.py
from PyQt6.QtWidgets import QMainWindow, QWidget, QHBoxLayout, QVBoxLayout, QStackedWidget
from PyQt6.QtCore import Qt
from gui_qt.titlebar import TitleBar
from gui_qt.sidebar import Sidebar
from gui_qt.pages.home import HomePage
from gui_qt.pages.history import HistoryPage
from gui_qt.pages.settings import SettingsPage
from gui_qt.pages.about import AboutDialog


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Bilibili Downloader")
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint)
        self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
        self.setFixedSize(1100, 760)

        # Root widget — solid bg matching the glass design
        central = QWidget()
        central.setObjectName("window-root")
        central.setStyleSheet("""
            #window-root {
                background: #f0f2f5;
                border: 1px solid rgba(255, 255, 255, 0.4);
                border-radius: 24px;
            }
        """)
        self.setCentralWidget(central)

        layout = QVBoxLayout(central)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        # Titlebar
        self.titlebar = TitleBar()
        layout.addWidget(self.titlebar)

        # Body: sidebar + content
        body = QWidget()
        body.setObjectName("window-body")
        body.setStyleSheet("background: transparent;")
        body_layout = QHBoxLayout(body)
        body_layout.setContentsMargins(0, 0, 0, 0)
        body_layout.setSpacing(0)

        self.sidebar = Sidebar()
        body_layout.addWidget(self.sidebar)

        self.stack = QStackedWidget()
        self.stack.setStyleSheet("background: transparent;")
        self.stack.addWidget(HomePage())
        self.stack.addWidget(HistoryPage())
        self.stack.addWidget(SettingsPage())
        self.stack.addWidget(QWidget())
        body_layout.addWidget(self.stack, 1)

        layout.addWidget(body, 1)

        self.sidebar.nav_list.currentItemChanged.connect(self._on_nav_changed)

    def _on_nav_changed(self, current, previous):
        if current and current.text() == "ℹ️ 关于":
            dlg = AboutDialog(self)
            dlg.exec()
            if previous:
                self.sidebar.nav_list.setCurrentItem(previous)
