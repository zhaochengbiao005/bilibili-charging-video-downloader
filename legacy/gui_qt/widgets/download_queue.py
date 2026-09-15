from PyQt6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QProgressBar, QScrollArea, QFrame
from PyQt6.QtCore import Qt


class DownloadTaskWidget(QWidget):
    def __init__(self, title: str, quality: str, fmt: str, parent=None):
        super().__init__(parent)
        self.setStyleSheet("""
            background: rgba(255,255,255,0.4);
            border: 1px solid rgba(255,255,255,0.6);
            border-radius: 12px;
        """)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(16, 12, 16, 12)
        layout.setSpacing(6)

        # Title row
        row = QWidget()
        rl = QHBoxLayout(row)
        rl.setContentsMargins(0, 0, 0, 0)

        self.title_label = QLabel(title)
        self.title_label.setStyleSheet("font-weight: 700; font-size: 13px; color: #444;")
        self.title_label.setWordWrap(True)
        rl.addWidget(self.title_label, 1)

        self.pct_label = QLabel("0%")
        self.pct_label.setStyleSheet("font-weight: 700; font-size: 12px; color: #fb7299;")
        rl.addWidget(self.pct_label)

        layout.addWidget(row)

        # Progress bar
        self.progress = QProgressBar()
        self.progress.setRange(0, 100)
        self.progress.setValue(0)
        self.progress.setFixedHeight(6)
        self.progress.setTextVisible(False)
        layout.addWidget(self.progress)

        # Meta row
        meta = QLabel(f"{quality} · {fmt.upper()}")
        meta.setStyleSheet("font-size: 10px; color: #999;")
        layout.addWidget(meta)

    def set_progress(self, value: int):
        self.progress.setValue(value)
        self.pct_label.setText(f"{value}%")


class DownloadQueue(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(8)

        header = QLabel("\U0001f4e5 下载队列")
        header.setStyleSheet("font-size: 16px; font-weight: 900; color: #333;")
        layout.addWidget(header)

        self.scroll = QScrollArea()
        self.scroll.setWidgetResizable(True)
        self.scroll.setFrameShape(QFrame.Shape.NoFrame)
        self.scroll.setStyleSheet("background: transparent;")

        self.container = QWidget()
        self.container.setStyleSheet("background: transparent;")
        self.container_layout = QVBoxLayout(self.container)
        self.container_layout.setContentsMargins(0, 0, 0, 0)
        self.container_layout.setSpacing(8)
        self.container_layout.addStretch()

        self.scroll.setWidget(self.container)
        layout.addWidget(self.scroll, 1)

    def add_task(self, title: str, quality: str, fmt: str) -> DownloadTaskWidget:
        widget = DownloadTaskWidget(title, quality, fmt)
        # Insert before the stretch
        self.container_layout.insertWidget(self.container_layout.count() - 1, widget)
        return widget
