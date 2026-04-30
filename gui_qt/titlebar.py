# gui_qt/titlebar.py
from PyQt6.QtWidgets import QWidget, QHBoxLayout, QLabel, QPushButton
from PyQt6.QtCore import Qt, QPoint

class TitleBar(QWidget):
    """Custom frameless titlebar with drag + min/max/close."""
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("titlebar")
        self.setFixedHeight(44)
        self._drag_pos = QPoint()

        layout = QHBoxLayout(self)
        layout.setContentsMargins(16, 0, 12, 0)

        self.label = QLabel("BiliDownloader")
        self.label.setStyleSheet("font-size: 12px; font-weight: 700; color: #999; letter-spacing: 1px;")

        layout.addWidget(self.label)
        layout.addStretch()

        for text, obj, slot in [
            ("─", "min-btn", self._minimize),
            ("□", "max-btn", self._maximize),
            ("✕", "close-btn", self._close),
        ]:
            btn = QPushButton(text)
            btn.setObjectName(obj)
            btn.setFixedSize(36, 36)
            btn.setCursor(Qt.CursorShape.ArrowCursor)
            btn.clicked.connect(slot)
            layout.addWidget(btn)

    def _minimize(self):
        self.window().showMinimized()

    def _maximize(self):
        w = self.window()
        if w.isMaximized():
            w.showNormal()
        else:
            w.showMaximized()

    def _close(self):
        self.window().close()

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self._drag_pos = event.globalPosition().toPoint()
            self._drag_start = self.window().pos()
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        if event.buttons() & Qt.MouseButton.LeftButton:
            delta = event.globalPosition().toPoint() - self._drag_pos
            self.window().move(self._drag_start + delta)
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event):
        self._drag_pos = QPoint()
        super().mouseReleaseEvent(event)
