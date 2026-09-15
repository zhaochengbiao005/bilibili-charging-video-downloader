from PyQt6.QtWidgets import QWidget, QHBoxLayout, QLineEdit, QPushButton
from PyQt6.QtCore import Qt


class UrlInput(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)

        self.input = QLineEdit()
        self.input.setPlaceholderText("https://www.bilibili.com/video/BV...")
        self.input.setStyleSheet("""
            QLineEdit {
                background: transparent;
                border: none;
                font-size: 15px;
                padding: 12px 8px;
                font-weight: 500;
                color: #444;
            }
            QLineEdit:focus { border: none; }
        """)

        self.parse_btn = QPushButton("解析 →")
        self.parse_btn.setStyleSheet("""
            QPushButton {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #fb7299, stop:1 #ff85a8);
                color: white;
                border: none;
                border-radius: 24px;
                padding: 12px 28px;
                font-weight: 700;
                font-size: 14px;
            }
            QPushButton:hover {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #ff85a8, stop:1 #ff95b5);
            }
            QPushButton:disabled { opacity: 0.6; }
        """)
        self.parse_btn.setCursor(Qt.CursorShape.PointingHandCursor)

        layout.addWidget(self.input)
        layout.addWidget(self.parse_btn)

    def get_url(self) -> str:
        return self.input.text().strip()
