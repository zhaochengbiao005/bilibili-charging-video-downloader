# gui_qt/pages/history.py
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QPushButton, QLineEdit, QListWidget, QListWidgetItem, QHBoxLayout, QFrame
from PyQt6.QtCore import Qt


class HistoryPage(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(40, 40, 40, 40)
        layout.setSpacing(20)

        # Header with clear button
        header = QWidget()
        hl = QHBoxLayout(header)
        hl.setContentsMargins(0, 0, 0, 0)

        title = QLabel("\U0001f4cb 下载历史")
        title.setStyleSheet("font-size: 28px; font-weight: 900; color: #222;")
        hl.addWidget(title)

        hl.addStretch()

        self.clear_btn = QPushButton("清空历史")
        self.clear_btn.setStyleSheet("""
            QPushButton {
                background: rgba(255,255,255,0.6);
                border: 1px solid rgba(255,255,255,0.8);
                border-radius: 16px;
                padding: 10px 20px;
                font-weight: 700;
                font-size: 13px;
                color: #888;
            }
            QPushButton:hover { color: #e0434a; }
        """)
        hl.addWidget(self.clear_btn)

        layout.addWidget(header)

        # Search
        self.search = QLineEdit()
        self.search.setPlaceholderText("搜索历史记录...")
        self.search.setStyleSheet("""
            QLineEdit {
                background: rgba(255,255,255,0.5);
                border: 1px solid rgba(255,255,255,0.8);
                border-radius: 16px;
                padding: 12px 16px;
                font-size: 14px;
            }
            QLineEdit:focus { border: 2px solid #fb7299; }
        """)
        layout.addWidget(self.search)

        # Empty state
        self.empty_label = QLabel("✨ 暂无下载记录")
        self.empty_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.empty_label.setStyleSheet("font-size: 16px; color: #bbb; font-weight: 700; padding: 60px;")
        layout.addWidget(self.empty_label)

        # List
        self.list_widget = QListWidget()
        self.list_widget.setFrameShape(QFrame.Shape.NoFrame)
        self.list_widget.setStyleSheet("""
            QListWidget {
                background: transparent;
                border: none;
            }
            QListWidget::item {
                background: rgba(255,255,255,0.6);
                border: 1px solid rgba(255,255,255,0.8);
                border-radius: 16px;
                padding: 16px;
                margin: 4px 0;
            }
            QListWidget::item:hover {
                background: rgba(255,255,255,0.8);
            }
        """)
        layout.addWidget(self.list_widget, 1)
        self.list_widget.hide()

    def set_items(self, items: list):
        """items: list of dicts with keys: title, bvid, quality, format, timestamp, output_path"""
        self.list_widget.clear()
        if not items:
            self.empty_label.show()
            self.list_widget.hide()
            return
        self.empty_label.hide()
        self.list_widget.show()
        for item in items:
            w = QWidget()
            w.setStyleSheet("background: transparent;")
            wl = QVBoxLayout(w)
            wl.setContentsMargins(0, 0, 0, 0)
            wl.setSpacing(4)

            title = QLabel(item.get("title", ""))
            title.setStyleSheet("font-weight: 700; font-size: 13px; color: #222;")
            wl.addWidget(title)

            meta = QLabel('{bvid} · {quality} · {fmt} · {ts}'.format(
                bvid=item.get("bvid", ""),
                quality=item.get("quality", ""),
                fmt=item.get("format", "").upper(),
                ts=item.get("timestamp", ""),
            ))
            meta.setStyleSheet("font-size: 11px; color: #999;")
            wl.addWidget(meta)

            qi = QListWidgetItem()
            qi.setSizeHint(w.sizeHint())
            self.list_widget.addItem(qi)
            self.list_widget.setItemWidget(qi, w)
