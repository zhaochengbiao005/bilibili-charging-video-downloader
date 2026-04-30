from PyQt6.QtWidgets import QFrame, QVBoxLayout

class GlassPanel(QFrame):
    """Rounded card with frosted-glass styling."""
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setProperty("class", "glass-panel")
        self.setStyleSheet("""
            GlassPanel {
                background: #ffffff;
                border: 1px solid #eeeeff;
                border-radius: 24px;
            }
        """)
        self._layout = QVBoxLayout(self)
        self._layout.setContentsMargins(24, 24, 24, 24)
        self._layout.setSpacing(16)

    def layout(self):
        return self._layout
