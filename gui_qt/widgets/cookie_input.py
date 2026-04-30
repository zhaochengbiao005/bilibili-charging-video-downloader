from PyQt6.QtWidgets import QWidget, QHBoxLayout, QLineEdit, QPushButton, QFileDialog
from PyQt6.QtCore import Qt


class CookieInput(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)

        self.input = QLineEdit()
        self.input.setPlaceholderText("Cookie 文件路径（可选）")
        self.input.setStyleSheet("""
            QLineEdit {
                background: transparent;
                border: 1px solid rgba(200,200,200,0.4);
                border-radius: 10px;
                padding: 8px 12px;
                font-size: 12px;
                color: #666;
            }
            QLineEdit:focus { border: 1px solid #fb7299; }
        """)

        browse_btn = QPushButton("浏览")
        browse_btn.setStyleSheet("""
            QPushButton {
                background: rgba(255,255,255,0.6);
                border: 1px solid rgba(255,255,255,0.8);
                border-radius: 10px;
                padding: 8px 16px;
                font-weight: 700;
                font-size: 12px;
                color: #888;
            }
            QPushButton:hover { color: #fb7299; }
        """)
        browse_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        browse_btn.clicked.connect(self._browse)

        qr_btn = QPushButton("扫码登录")
        qr_btn.setStyleSheet("""
            QPushButton {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #00a1d6, stop:1 #40c5f1);
                color: white;
                border: none;
                border-radius: 10px;
                padding: 8px 16px;
                font-weight: 700;
                font-size: 12px;
            }
            QPushButton:hover { opacity: 0.8; }
        """)
        qr_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        qr_btn.clicked.connect(self._qr_login)

        layout.addWidget(self.input)
        layout.addWidget(browse_btn)
        layout.addWidget(qr_btn)

    def _browse(self):
        path, _ = QFileDialog.getOpenFileName(
            self, "选择 Cookie 文件", "", "Cookie 文件 (*.txt *.json);;所有文件 (*.*)")
        if path:
            self.input.setText(path)

    def _qr_login(self):
        from PyQt6.QtWidgets import QMessageBox
        QMessageBox.information(self, "扫码登录", "浏览器窗口将打开，请用 B站 App 扫码")

    def get_cookie_path(self) -> str:
        return self.input.text().strip()
