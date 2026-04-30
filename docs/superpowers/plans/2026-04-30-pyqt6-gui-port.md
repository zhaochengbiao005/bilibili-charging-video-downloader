# PyQt6 GUI Port — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `gui.py` (ttkbootstrap) with a PyQt6 GUI that visually matches the Google AI Studio React design (glassmorphism + Bilibili pink theme).

**Architecture:** Single-window PyQt6 app with frameless window, custom titlebar, sidebar navigation (`QListWidget` + `QStackedWidget`), and reusable glass-panel widgets. Same Python backend modules (`src/`) reused without modification. Background downloads run in `QThread`.

**Tech Stack:** PyQt6, Python 3.13+, QSS styling, existing `src/` modules

---

## File Structure

```
gui_qt/                          ← NEW directory
├── __init__.py                  # Package init, exports QApplication setup
├── app.py                       # QApplication, global QSS theme, entry point
├── window.py                    # MainWindow (frameless, shadow, rounded corners)
├── sidebar.py                   # Sidebar widget (nav list, logo, support button)
├── titlebar.py                  # Custom titlebar (drag + min/max/close)
├── glass_panel.py               # Reusable frosted-glass panel widget base
├── pages/
│   ├── __init__.py
│   ├── home.py                  # Home page: search, video info, download settings, queue
│   ├── history.py               # Download history page
│   ├── settings.py              # Settings page
│   └── about.py                 # About dialog (modal)
├── widgets/
│   ├── __init__.py
│   ├── url_input.py             # URL input row (text field + parse button)
│   ├── cookie_input.py          # Cookie input row (path + browse + QR login)
│   ├── video_info.py            # Video info card (thumbnail, metadata, tags)
│   ├── download_options.py      # Format/quality selection panel
│   ├── download_queue.py        # Download task list with progress bars
│   └── log_panel.py             # Collapsible log output
├── resources/
│   ├── styles.qss               # Global QSS stylesheet
│   └── icons/                   # SVG icons (optional, can use unicode)
└── main.py                      # Entry point: `python -m gui_qt.main`
```

**Modified files:**
- `requirements.txt` — add PyQt6 dependency

**Unchanged files:**
- `src/bilibili_api.py` — reused as-is
- `src/downloader.py` — reused as-is
- `src/merger.py` — reused as-is
- `login_capture.py` — reused as-is
- `main.py` — CLI, unchanged

---

## Phase 1: Scaffold — Window + Navigation

### Task 1: Create gui_qt package structure

**Files:**
- Create: `gui_qt/__init__.py`
- Create: `gui_qt/app.py`
- Create: `gui_qt/main.py`
- Modify: `requirements.txt`

- [ ] **1a: Create package init**

```python
# gui_qt/__init__.py
"""PyQt6 GUI for Bilibili Downloader"""
```

- [ ] **1b: Create main entry point**

```python
# gui_qt/main.py
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))
from gui_qt.app import run

if __name__ == '__main__':
    run()
```

- [ ] **1c: Create app.py with QApplication bootstrap**

```python
# gui_qt/app.py
import sys, os
from PyQt6.QtWidgets import QApplication
from PyQt6.QtGui import QFont
from gui_qt.window import MainWindow

QSS_PATH = os.path.join(os.path.dirname(__file__), 'resources', 'styles.qss')

def run():
    app = QApplication(sys.argv)
    app.setFont(QFont("-apple-system", "Microsoft YaHei UI", 10))

    # Load QSS
    if os.path.exists(QSS_PATH):
        with open(QSS_PATH, encoding='utf-8') as f:
            app.setStyleSheet(f.read())

    window = MainWindow()
    window.show()
    sys.exit(app.exec())
```

- [ ] **1d: Add PyQt6 to requirements.txt**

```
requests>=2.31.0
PyQt6>=6.8.0
```

- [ ] **1e: Verify import works**

Run: `python -c "from gui_qt.main import run; print('OK')"`
Expected: `OK`

---

### Task 2: Resources — Global QSS theme

**Files:**
- Create: `gui_qt/resources/`
- Create: `gui_qt/resources/styles.qss`

- [ ] **2a: Create resources directory**

```bash
mkdir -p gui_qt/resources
```

- [ ] **2b: Write base QSS theme**

```css
/* gui_qt/resources/styles.qss */
/* Bilibili Pink Theme — PyQt6 Stylesheet */

/* ── Color Variables ── */
/* Primary: #fb7299 (B站粉) */
/* Secondary: #00aeec (B站蓝) */
/* BG: #f4f5f7 (浅灰蓝) */
/* Card: rgba(255,255,255,0.6) */
/* Text: #222222 */
/* Text secondary: #99a2aa */

/* ── Global ── */
QWidget {
    font-family: -apple-system, "Microsoft YaHei UI", "PingFang SC", sans-serif;
    color: #222222;
}

/* ── Scrollbar ── */
QScrollBar:vertical {
    width: 8px;
    background: transparent;
    margin: 8px 0;
}
QScrollBar::handle:vertical {
    background: rgba(0, 0, 0, 0.12);
    border-radius: 4px;
    min-height: 30px;
}
QScrollBar::handle:vertical:hover {
    background: rgba(0, 0, 0, 0.22);
}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
    height: 0;
}

/* ── Sidebar ── */
#sidebar {
    background: rgba(255, 255, 255, 0.6);
    border-right: 1px solid rgba(255, 255, 255, 0.8);
    backdrop-filter: blur(24px);
}
#sidebar-nav QListWidget {
    background: transparent;
    border: none;
    outline: none;
    padding: 8px;
}
#sidebar-nav QListWidget::item {
    padding: 12px 16px;
    border-radius: 16px;
    margin: 2px 0;
    font-weight: 700;
    font-size: 14px;
    color: #666;
}
#sidebar-nav QListWidget::item:hover {
    background: rgba(255, 255, 255, 0.4);
}
#sidebar-nav QListWidget::item:selected {
    background: rgba(255, 255, 255, 0.7);
    color: #fb7299;
    border: 1px solid rgba(255, 255, 255, 0.5);
}

/* ── Titlebar ── */
#titlebar {
    background: rgba(255, 255, 255, 0.3);
    border-bottom: 1px solid rgba(255, 255, 255, 0.5);
}
#titlebar-btn {
    border: none;
    background: transparent;
    color: #999;
    border-radius: 8px;
    padding: 4px 10px;
    font-size: 14px;
}
#titlebar-btn:hover {
    background: rgba(0, 0, 0, 0.06);
    color: #666;
}
#titlebar-btn.close:hover {
    background: rgba(224, 67, 74, 0.15);
    color: #e0434a;
}

/* ── Glass Panel (card) ── */
.glass-panel {
    background: rgba(255, 255, 255, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.8);
    border-radius: 24px;
    padding: 20px;
}

/* ── Buttons ── */
.btn-pink {
    background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
        stop:0 #fb7299, stop:1 #ff85a8);
    color: white;
    border: none;
    border-radius: 10px;
    padding: 8px 20px;
    font-weight: 700;
    font-size: 13px;
}
.btn-pink:hover {
    background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
        stop:0 #ff85a8, stop:1 #ff95b5);
}
.btn-pink:pressed {
    padding-top: 9px;
    padding-bottom: 7px;
}

.btn-blue {
    background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
        stop:0 #00a1d6, stop:1 #40c5f1);
    color: white;
    border: none;
    border-radius: 10px;
    padding: 8px 20px;
    font-weight: 700;
    font-size: 13px;
}

.btn-ghost {
    background: rgba(255, 255, 255, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.6);
    border-radius: 10px;
    padding: 8px 16px;
    color: #888;
    font-weight: 600;
    font-size: 12px;
}
.btn-ghost:hover {
    background: rgba(255, 255, 255, 0.6);
    color: #fb7299;
}

/* ── Inputs ── */
.input-field {
    background: rgba(255, 255, 255, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.6);
    border-radius: 10px;
    padding: 8px 14px;
    font-size: 13px;
    color: #444;
}
.input-field:focus {
    border: 2px solid #fb7299;
    background: rgba(255, 255, 255, 0.7);
}

/* ── Progress Bar ── */
QProgressBar {
    background: rgba(0, 0, 0, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.3);
    border-radius: 4px;
    height: 8px;
    text-align: center;
    font-size: 10px;
}
QProgressBar::chunk {
    background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
        stop:0 #fb7299, stop:0.5 #00a1d6, stop:1 #9664ff);
    border-radius: 4px;
}

/* ── ComboBox / Select ── */
QComboBox {
    background: rgba(255, 255, 255, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.6);
    border-radius: 10px;
    padding: 8px 14px;
    font-size: 12px;
    color: #444;
    min-width: 100px;
}
QComboBox:focus {
    border: 2px solid #fb7299;
}
QComboBox::drop-down {
    border: none;
    width: 30px;
}
QComboBox QAbstractItemView {
    background: white;
    border: 1px solid #eee;
    border-radius: 12px;
    padding: 4px;
    selection-background-color: rgba(251, 114, 153, 0.1);
    selection-color: #fb7299;
}

/* ── Radio Button (styled as quality selector) ── */
QRadioButton {
    spacing: 8px;
    font-weight: 700;
    font-size: 13px;
    color: #444;
}
QRadioButton::indicator {
    width: 20px;
    height: 20px;
    border-radius: 10px;
    border: 2px solid #ddd;
}
QRadioButton::indicator:checked {
    border: 2px solid #fb7299;
    background: #fb7299;
}
```

- [ ] **2c: Verify QSS file is loadable**

Run: `python -c "open('gui_qt/resources/styles.qss').read(); print('OK')"`
Expected: `OK`

---

### Task 3: GlassPanel base widget

**Files:**
- Create: `gui_qt/glass_panel.py`

- [ ] **3a: Implement GlassPanel**

```python
# gui_qt/glass_panel.py
from PyQt6.QtWidgets import QFrame, QVBoxLayout
from PyQt6.QtCore import Qt

class GlassPanel(QFrame):
    """Rounded card with frosted-glass styling."""
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setProperty("class", "glass-panel")
        self.setStyleSheet("""
            background: rgba(255, 255, 255, 0.6);
            border: 1px solid rgba(255, 255, 255, 0.8);
            border-radius: 24px;
        """)
        self._layout = QVBoxLayout(self)
        self._layout.setContentsMargins(24, 24, 24, 24)
        self._layout.setSpacing(16)

    def layout(self):
        return self._layout
```

- [ ] **3b: Verify import**

Run: `python -c "from gui_qt.glass_panel import GlassPanel; print('OK')"`
Expected: `OK`

---

### Task 4: Custom Titlebar

**Files:**
- Create: `gui_qt/titlebar.py`

- [ ] **4a: Implement TitleBar widget**

```python
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
```

- [ ] **4b: Verify import**

Run: `python -c "from gui_qt.titlebar import TitleBar; print('OK')"`
Expected: `OK`

---

### Task 5: MainWindow (frameless frame)

**Files:**
- Create: `gui_qt/window.py`

- [ ] **5a: Install PyQt6 if needed**

```bash
pip install PyQt6
```

- [ ] **5b: Implement MainWindow**

```python
# gui_qt/window.py
from PyQt6.QtWidgets import QMainWindow, QWidget, QHBoxLayout, QVBoxLayout, QStackedWidget
from PyQt6.QtCore import Qt, QPropertyAnimation, QEasingCurve
from PyQt6.QtGui import QPainter, QPainterPath, QBrush, QColor, QPen
from gui_qt.titlebar import TitleBar
from gui_qt.sidebar import Sidebar
from gui_qt.pages.home import HomePage
from gui_qt.pages.history import HistoryPage
from gui_qt.pages.settings import SettingsPage

class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Bilibili Downloader")
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setFixedSize(1100, 760)

        # Shadow effect for the entire window
        from PyQt6.QtGui import QPainter, QColor, QLinearGradient
        self._shadow_margin = 20

        # Central widget
        central = QWidget()
        central.setObjectName("window-container")
        central.setStyleSheet("""
            #window-container {
                background: rgba(244, 245, 247, 0.6);
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
        body_layout = QHBoxLayout(body)
        body_layout.setContentsMargins(0, 0, 0, 0)
        body_layout.setSpacing(0)

        self.sidebar = Sidebar()
        body_layout.addWidget(self.sidebar)

        self.stack = QStackedWidget()
        self.stack.addWidget(HomePage())
        self.stack.addWidget(HistoryPage())
        self.stack.addWidget(SettingsPage())
        body_layout.addWidget(self.stack, 1)

        layout.addWidget(body, 1)

        # Connect sidebar navigation
        self.sidebar.nav_list.currentRowChanged.connect(self.stack.setCurrentIndex)

    def paintEvent(self, event):
        """Draw window shadow (simulated with gradient edges)."""
        super().paintEvent(event)
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        path = QPainterPath()
        rect = self.rect().adjusted(2, 2, -2, -2)
        path.addRoundedRect(rect, 24, 24)
        painter.setPen(QPen(QColor(255, 255, 255, 100), 1))
        painter.setBrush(QBrush(QColor(244, 245, 247, int(0.6 * 255))))
        painter.drawPath(path)
```

- [ ] **5c: Verify window can be instantiated**

Run: `python -c "from gui_qt.window import MainWindow; print('OK')"`
Expected: `OK`

---

### Task 6: Sidebar navigation

**Files:**
- Create: `gui_qt/sidebar.py`

- [ ] **6a: Implement Sidebar**

```python
# gui_qt/sidebar.py
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QListWidget, QPushButton
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
        self.nav_list.setFrameShape(QListWidget.FrameShape.NoFrame)
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
```

- [ ] **6b: Verify import**

Run: `python -c "from gui_qt.sidebar import Sidebar; print('OK')"`
Expected: `OK`

---

### Task 7: Verify Phase 1 scaffold runs without error

- [ ] **7a: Quick smoke test**

```bash
cd "F:\本地模型性能测试\破解B站充电视频测试"
python -c "
import sys, os
sys.path.insert(0, '.')
from gui_qt.app import run
# Just verify imports, don't actually show window
print('Phase 1 scaffold complete')
"
```

Expected: `Phase 1 scaffold complete`

---

## Phase 2: Home Page

### Task 8: UrlInput + CookieInput widgets

**Files:**
- Create: `gui_qt/widgets/__init__.py`
- Create: `gui_qt/widgets/url_input.py`
- Create: `gui_qt/widgets/cookie_input.py`

- [ ] **8a: Create widgets package**

```bash
mkdir -p gui_qt/widgets
touch gui_qt/widgets/__init__.py
```

- [ ] **8b: Implement UrlInput**

```python
# gui_qt/widgets/url_input.py
from PyQt6.QtWidgets import QWidget, QHBoxLayout, QLineEdit, QPushButton

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
```

- [ ] **8c: Implement CookieInput**

```python
# gui_qt/widgets/cookie_input.py
from PyQt6.QtWidgets import QWidget, QHBoxLayout, QLineEdit, QPushButton

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

    def _qr_login(self):
        from PyQt6.QtWidgets import QMessageBox
        QMessageBox.information(self, "扫码登录", "浏览器窗口将打开，请用 B站 App 扫码")

    def get_cookie_path(self) -> str:
        return self.input.text().strip()
```

- [ ] **8d: Verify imports**

Run: `python -c "from gui_qt.widgets.url_input import UrlInput; from gui_qt.widgets.cookie_input import CookieInput; print('OK')"`
Expected: `OK`

---

### Task 9: VideoInfo card widget

**Files:**
- Create: `gui_qt/widgets/video_info.py`

- [ ] **9a: Implement VideoInfo**

```python
# gui_qt/widgets/video_info.py
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QFrame
from PyQt6.QtCore import Qt
from gui_qt.glass_panel import GlassPanel

class VideoInfo(GlassPanel):
    """Video information card with thumbnail, metadata, tags."""
    def __init__(self, parent=None):
        super().__init__(parent)

        # Thumbnail placeholder
        self.thumb = QLabel()
        self.thumb.setFixedHeight(240)
        self.thumb.setStyleSheet("""
            background: #e0e0e0;
            border-radius: 16px;
            font-size: 14px;
            color: #999;
        """)
        self.thumb.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.thumb.setText("暂无缩略图")
        self.layout().addWidget(self.thumb)

        # Title
        self.title = QLabel()
        self.title.setWordWrap(True)
        self.title.setStyleSheet("font-size: 22px; font-weight: 900; color: #222;")
        self.layout().addWidget(self.title)

        # Tags row
        tags = QWidget()
        tags_layout = QHBoxLayout(tags)
        tags_layout.setContentsMargins(0, 0, 0, 0)
        tags_layout.setSpacing(8)

        self.author_label = QLabel()
        self.author_label.setStyleSheet("font-weight: 600; color: #555; font-size: 13px;")
        tags_layout.addWidget(self.author_label)

        self.views_label = QLabel()
        self.views_label.setStyleSheet("color: #999; font-size: 12px;")
        tags_layout.addWidget(self.views_label)

        self.pages_label = QLabel()
        self.pages_label.setStyleSheet("color: #999; font-size: 12px;")
        tags_layout.addWidget(self.pages_label)

        self.vip_tag = QLabel()
        self.vip_tag.setStyleSheet("""
            background: rgba(251,114,153,0.12);
            color: #fb7299;
            border-radius: 4px;
            padding: 2px 8px;
            font-size: 11px;
            font-weight: 700;
        """)
        tags_layout.addWidget(self.vip_tag)

        self.charge_tag = QLabel()
        self.charge_tag.setStyleSheet("""
            background: rgba(255,138,43,0.12);
            color: #ff8a2b;
            border-radius: 4px;
            padding: 2px 8px;
            font-size: 11px;
            font-weight: 700;
        """)
        tags_layout.addWidget(self.charge_tag)

        tags_layout.addStretch()
        self.layout().addWidget(tags)

        # Description
        self.desc = QLabel()
        self.desc.setWordWrap(True)
        self.desc.setStyleSheet("color: #888; font-size: 12px; line-height: 1.6;")
        self.desc.setMaximumHeight(60)
        self.layout().addWidget(self.desc)

        # Hide optional tags initially
        self.vip_tag.hide()
        self.charge_tag.hide()

    def set_data(self, data: dict):
        """Populate from API response dict."""
        self.title.setText(data.get("title", ""))
        self.author_label.setText(f"👤 {data.get('author', '')}")
        self.views_label.setText(f"👁 {data.get('views', '0')} 次播放")
        pages = data.get("pages", [])
        self.pages_label.setText(f"📑 {len(pages)} 个分P")

        if data.get("is_vip"):
            self.vip_tag.setText("👑 大会员")
            self.vip_tag.show()
        else:
            self.vip_tag.hide()

        if data.get("is_charging"):
            self.charge_tag.setText("⚡ 充电专属")
            self.charge_tag.show()
        else:
            self.charge_tag.hide()

        desc = data.get("desc", "")
        self.desc.setText(desc[:200] + ("..." if len(desc) > 200 else ""))
```

- [ ] **9b: Verify import**

Run: `python -c "from gui_qt.widgets.video_info import VideoInfo; print('OK')"`
Expected: `OK`

---

### Task 10: DownloadOptions widget

**Files:**
- Create: `gui_qt/widgets/download_options.py`

- [ ] **10a: Implement DownloadOptions**

```python
# gui_qt/widgets/download_options.py
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QPushButton, QButtonGroup, QRadioButton, QHBoxLayout
from PyQt6.QtCore import Qt, pyqtSignal
from gui_qt.glass_panel import GlassPanel

QUALITY_LABELS = ["360P", "480P", "720P", "1080P", "1080P60", "4K", "HDR"]

class DownloadOptions(GlassPanel):
    download_clicked = pyqtSignal()

    def __init__(self, parent=None):
        super().__init__(parent)

        # Header
        header = QLabel("⚙️ 下载设置")
        header.setStyleSheet("font-size: 20px; font-weight: 900; color: #222;")
        self.layout().addWidget(header)

        # Format toggle
        fmt_label = QLabel("格式")
        fmt_label.setStyleSheet("font-size: 13px; font-weight: 700; color: #888;")
        self.layout().addWidget(fmt_label)

        fmt_row = QHBoxLayout()
        self.video_btn = QPushButton("🎬 视频 (MP4)")
        self.video_btn.setCheckable(True)
        self.video_btn.setChecked(True)
        self.video_btn.setStyleSheet(self._fmt_btn_style(True))

        self.audio_btn = QPushButton("🎵 音频 (MP3)")
        self.audio_btn.setCheckable(True)
        self.audio_btn.setStyleSheet(self._fmt_btn_style(False))

        self.video_btn.clicked.connect(lambda: self._switch_fmt("video"))
        self.audio_btn.clicked.connect(lambda: self._switch_fmt("audio"))

        fmt_row.addWidget(self.video_btn)
        fmt_row.addWidget(self.audio_btn)
        self.layout().addLayout(fmt_row)

        # Quality list
        ql = QLabel("画质")
        ql.setStyleSheet("font-size: 13px; font-weight: 700; color: #888; margin-top: 8px;")
        self.layout().addWidget(ql)

        self.quality_group = QButtonGroup(self)
        for q in QUALITY_LABELS:
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

        # Select first quality
        if self.quality_group.buttons():
            self.quality_group.buttons()[3].setChecked(True)  # 1080P

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
            QPushButton:pressed { padding-top: 17px; padding-bottom: 15px; }
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
```

- [ ] **10b: Verify import**

Run: `python -c "from gui_qt.widgets.download_options import DownloadOptions; print('OK')"`
Expected: `OK`

---

### Task 11: DownloadQueue widget

**Files:**
- Create: `gui_qt/widgets/download_queue.py`

- [ ] **11a: Implement DownloadQueue**

```python
# gui_qt/widgets/download_queue.py
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QPushButton, QProgressBar, QScrollArea
from PyQt6.QtCore import Qt

class DownloadTaskWidget(QWidget):
    """Single download task item with progress bar."""
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
    """Scrollable list of download tasks."""
    def __init__(self, parent=None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(8)

        header = QLabel("📥 下载队列")
        header.setStyleSheet("font-size: 16px; font-weight: 900; color: #333;")
        layout.addWidget(header)

        self.scroll = QScrollArea()
        self.scroll.setWidgetResizable(True)
        self.scroll.setFrameShape(QScrollArea.FrameShape.NoFrame)
        self.scroll.setStyleSheet("background: transparent;")

        self.container = QWidget()
        self.container_layout = QVBoxLayout(self.container)
        self.container_layout.setContentsMargins(0, 0, 0, 0)
        self.container_layout.setSpacing(8)
        self.container_layout.addStretch()

        self.scroll.setWidget(self.container)
        layout.addWidget(self.scroll, 1)

    def add_task(self, title: str, quality: str, fmt: str):
        widget = DownloadTaskWidget(title, quality, fmt)
        self.container_layout.insertWidget(self.container_layout.count() - 1, widget)
        return widget
```

- [ ] **11b: Verify import**

Run: `python -c "from gui_qt.widgets.download_queue import DownloadQueue, DownloadTaskWidget; print('OK')"`
Expected: `OK`

---

### Task 12: HomePage — assemble all widgets

**Files:**
- Create: `gui_qt/pages/__init__.py`
- Create: `gui_qt/pages/home.py`

- [ ] **12a: Create pages package**

```bash
mkdir -p gui_qt/pages
touch gui_qt/pages/__init__.py
```

- [ ] **12b: Implement HomePage**

```python
# gui_qt/pages/home.py
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QLabel, QScrollArea
from PyQt6.QtCore import Qt
from gui_qt.glass_panel import GlassPanel
from gui_qt.widgets.url_input import UrlInput
from gui_qt.widgets.cookie_input import CookieInput
from gui_qt.widgets.video_info import VideoInfo
from gui_qt.widgets.download_options import DownloadOptions
from gui_qt.widgets.download_queue import DownloadQueue

class HomePage(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)

        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QScrollArea.FrameShape.NoFrame)
        scroll.setStyleSheet("background: transparent;")

        content = QWidget()
        layout = QVBoxLayout(content)
        layout.setContentsMargins(40, 40, 40, 40)
        layout.setSpacing(24)

        # Header
        header = QWidget()
        hl = QVBoxLayout(header)
        hl.setContentsMargins(0, 0, 0, 0)
        hl.setSpacing(8)

        title = QLabel("下载你喜欢的视频")
        title.setStyleSheet("font-size: 36px; font-weight: 900; color: #222;")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        hl.addWidget(title)

        subtitle = QLabel("粘贴 B站视频链接，快速解析并下载高清视频与音频")
        subtitle.setStyleSheet("font-size: 16px; color: #888; font-weight: 500;")
        subtitle.setAlignment(Qt.AlignmentFlag.AlignCenter)
        hl.addWidget(subtitle)

        layout.addWidget(header)

        # Search bar (glass panel)
        search_panel = GlassPanel()
        search_panel.layout().setContentsMargins(16, 8, 16, 8)
        search_panel.setMaximumWidth(800)
        search_panel.setStyleSheet(search_panel.styleSheet() + "border-radius: 48px;")

        self.url_input = UrlInput()
        search_panel.layout().addWidget(self.url_input)

        self.cookie_input = CookieInput()
        search_panel.layout().addWidget(self.cookie_input)

        # Center the search bar
        search_wrap = QWidget()
        swl = QHBoxLayout(search_wrap)
        swl.setContentsMargins(0, 0, 0, 0)
        swl.addStretch()
        swl.addWidget(search_panel)
        swl.addStretch()
        layout.addWidget(search_wrap)

        # Content area: video info + download options side by side
        content_row = QWidget()
        crl = QHBoxLayout(content_row)
        crl.setContentsMargins(0, 0, 0, 0)
        crl.setSpacing(24)

        self.video_info = VideoInfo()
        crl.addWidget(self.video_info, 1)

        right_col = QWidget()
        rcl = QVBoxLayout(right_col)
        rcl.setContentsMargins(0, 0, 0, 0)
        rcl.setSpacing(16)

        self.dl_options = DownloadOptions()
        rcl.addWidget(self.dl_options)

        self.dl_queue = DownloadQueue()
        rcl.addWidget(self.dl_queue, 1)

        crl.addWidget(right_col, 1)
        layout.addWidget(content_row, 1)

        scroll.setWidget(content)

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.addWidget(scroll)
```

- [ ] **12c: Verify import**

Run: `python -c "from gui_qt.pages.home import HomePage; print('OK')"`
Expected: `OK`

---

## Phase 3: History + Settings Pages

### Task 13: HistoryPage

**Files:**
- Create: `gui_qt/pages/history.py`

- [ ] **13a: Implement HistoryPage**

```python
# gui_qt/pages/history.py
from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QPushButton, QLineEdit, QListWidget, QListWidgetItem
from PyQt6.QtCore import Qt
from gui_qt.glass_panel import GlassPanel

class HistoryPage(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        layout = QVBoxLayout(self)
        layout.setContentsMargins(40, 40, 40, 40)
        layout.setSpacing(20)

        # Header
        header = QLabel("📋 下载历史")
        header.setStyleSheet("font-size: 28px; font-weight: 900; color: #222;")
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

        # List
        self.list_widget = QListWidget()
        self.list_widget.setFrameShape(QListWidget.FrameShape.NoFrame)
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
        """)
        layout.addWidget(self.list_widget, 1)

        # Empty state
        self.empty = QLabel("✨ 暂无下载记录")
        self.empty.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.empty.setStyleSheet("font-size: 16px; color: #bbb; font-weight: 700; padding: 60px;")
        layout.addWidget(self.empty)
        self.empty.hide()
```

- [ ] **13b: Verify import**

Run: `python -c "from gui_qt.pages.history import HistoryPage; print('OK')"`
Expected: `OK`

---

### Task 14: SettingsPage

**Files:**
- Create: `gui_qt/pages/settings.py`

- [ ] **14a: Implement SettingsPage**

```python
# gui_qt/pages/settings.py
from PyQt6.QtWidgets import (QWidget, QVBoxLayout, QLabel, QPushButton, QComboBox,
                             QLineEdit, QCheckBox, QHBoxLayout, QScrollArea)
from PyQt6.QtCore import Qt
from gui_qt.glass_panel import GlassPanel

class SettingsPage(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)

        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QScrollArea.FrameShape.NoFrame)
        scroll.setStyleSheet("background: transparent;")

        content = QWidget()
        layout = QVBoxLayout(content)
        layout.setContentsMargins(40, 40, 40, 40)
        layout.setSpacing(24)

        # Header
        h = QLabel("⚙️ 设置")
        h.setStyleSheet("font-size: 28px; font-weight: 900; color: #222;")
        layout.addWidget(h)

        # FFmpeg status
        ffmpeg_panel = GlassPanel()
        ffmpeg_panel.layout().addWidget(QLabel("📦 FFmpeg 状态"))
        self.ffmpeg_status = QLabel("检查中...")
        ffmpeg_panel.layout().addWidget(self.ffmpeg_status)
        self.ffmpeg_btn = QPushButton("安装 FFmpeg")
        ffmpeg_panel.layout().addWidget(self.ffmpeg_btn)
        layout.addWidget(ffmpeg_panel)

        # Download defaults
        defaults = GlassPanel()
        defaults.layout().addWidget(QLabel("⬇ 默认下载设置"))

        # Quality
        defaults.layout().addWidget(QLabel("默认画质"))
        self.quality_combo = QComboBox()
        self.quality_combo.addItems(["360P", "480P", "720P", "1080P", "1080P60", "4K", "HDR"])
        defaults.layout().addWidget(self.quality_combo)

        # Speed
        defaults.layout().addWidget(QLabel("下载速度"))
        self.speed_combo = QComboBox()
        self.speed_combo.addItems(["慢速 (4线程)", "标准 (8线程)", "快速 (16线程)", "极速 (32线程)"])
        defaults.layout().addWidget(self.speed_combo)

        # Output dir
        defaults.layout().addWidget(QLabel("输出目录"))
        dir_row = QHBoxLayout()
        self.outdir = QLineEdit("downloads")
        dir_row.addWidget(self.outdir)
        browse = QPushButton("浏览")
        dir_row.addWidget(browse)
        defaults.layout().addLayout(dir_row)

        # Auto merge
        self.auto_merge = QCheckBox("自动合并音视频（需要 FFmpeg）")
        defaults.layout().addWidget(self.auto_merge)

        # Save button
        save = QPushButton("保存设置")
        save.setStyleSheet("""
            QPushButton {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #fb7299, stop:1 #ff85a8);
                color: white;
                border: none;
                border-radius: 16px;
                padding: 12px 32px;
                font-weight: 700;
                font-size: 14px;
            }
            QPushButton:hover { opacity: 0.8; }
        """)
        save.setFixedWidth(160)
        save_row = QHBoxLayout()
        save_row.addStretch()
        save_row.addWidget(save)
        defaults.layout().addLayout(save_row)
        layout.addWidget(defaults)

        # Data info
        info = GlassPanel()
        info.setStyleSheet(info.styleSheet() + """
            background: rgba(240, 245, 255, 0.6);
            border: 1px solid rgba(200, 215, 240, 0.5);
            border-radius: 16px;
        """)
        info.layout().addWidget(QLabel("📁 数据存储说明"))
        info.layout().addWidget(QLabel(
            "程序配置、Cookie 和下载历史保存在 data/ 目录中。\n"
            "下载的视频保存在输出目录，卸载程序时视频文件不会被删除。"
        ))
        layout.addWidget(info)

        layout.addStretch()
        scroll.setWidget(content)

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.addWidget(scroll)
```

- [ ] **14b: Verify import**

Run: `python -c "from gui_qt.pages.settings import SettingsPage; print('OK')"`
Expected: `OK`

---

### Task 15: AboutPage (modal dialog)

**Files:**
- Create: `gui_qt/pages/about.py`

- [ ] **15a: Implement AboutPage**

```python
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
        self.setStyleSheet("""
            background: rgba(255,255,255,0.85);
            border: 1px solid white;
            border-radius: 32px;
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
```

- [ ] **15b: Verify import**

Run: `python -c "from gui_qt.pages.about import AboutDialog; print('OK')"`
Expected: `OK`

---

## Phase 4: Backend Integration

### Task 16: Wire HomePage to real API

**Files:**
- Modify: `gui_qt/pages/home.py`
- Modify: `gui_qt/widgets/video_info.py`
- Modify: `gui_qt/widgets/download_options.py`

- [ ] **16a: Add API call when parse button clicked**

Connect `UrlInput.parse_btn.clicked` to call `BilibiliAPI`:

```python
# In home.py, add:
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..'))
from src.bilibili_api import BilibiliAPI, APIError

# Connect parse button
self.url_input.parse_btn.clicked.connect(self._fetch_info)

def _fetch_info(self):
    bvid = self._extract_bvid(self.url_input.get_url())
    if not bvid:
        return
    self.url_input.parse_btn.setEnabled(False)
    self.url_input.parse_btn.setText("解析中...")

    # Run in background
    from PyQt6.QtCore import QThread
    class FetchThread(QThread):
        def run(self):
            try:
                cookie = self._parent.cookie_input.get_cookie_path()
                api = BilibiliAPI()
                if cookie:
                    from src.bilibili_api import BilibiliAPI as API
                    # Parse cookies
                    ...
                info = api.get_video_info(bvid)
                self._parent.video_info.set_data(info)
            except Exception as e:
                print(f"Error: {e}")
            finally:
                self._parent.url_input.parse_btn.setEnabled(True)
                self._parent.url_input.parse_btn.setText("解析 →")

    # Simplified: direct call for now (blocking, but OK for initial impl)
    import threading
    def fetch():
        try:
            api = BilibiliAPI()
            info = api.get_video_info(bvid)
            self.video_info.set_data(info)
        except APIError as e:
            print(f"API Error: {e}")
        finally:
            self.url_input.parse_btn.setEnabled(True)
            self.url_input.parse_btn.setText("解析 →")

    threading.Thread(target=fetch, daemon=True).start()
```

- [ ] **16b: Add BVID extraction helper**

```python
import re
@staticmethod
def _extract_bvid(text):
    m = re.search(r"BV\w{10,}", text.strip() or "")
    return m.group(0) if m else None
```

---

### Task 17: Wire download button to Downloader

**Files:**
- Modify: `gui_qt/pages/home.py`

- [ ] **17a: Connect download button**

```python
self.dl_options.download_clicked.connect(self._start_download)

def _start_download(self):
    bvid = self._extract_bvid(self.url_input.get_url())
    if not bvid:
        return
    # Get selected quality
    selected = self.dl_options.quality_group.checkedButton()
    quality = selected.text() if selected else "1080P"

    widget = self.dl_queue.add_task(f"正在处理 {bvid}...", quality, "video")

    import threading
    def dl():
        try:
            api = BilibiliAPI()
            info = api.get_video_info(bvid)
            pages = info.get("pages", [])
            if pages:
                cid = pages[0]["cid"]
                qn = {"360P":16,"480P":32,"720P":64,"1080P":80,
                      "1080P60":116,"4K":120,"HDR":125}.get(quality, 80)
                playurl = api.get_playurl(bvid, cid, qn=qn)
                dash = api.extract_dash_urls(playurl)
                if dash["video"] and dash["audio"]:
                    from src.downloader import VideoDownloader
                    bv = max(dash["video"], key=lambda v: v.get("bandwidth", 0))
                    ba = max(dash["audio"], key=lambda v: v.get("bandwidth", 0))
                    dl = VideoDownloader(max_workers=4)
                    v_out = os.path.join("downloads", f"{bvid}_video.m4s")
                    a_out = os.path.join("downloads", f"{bvid}_audio.m4s")
                    dl.download(bv["base_url"], v_out)
                    widget.set_progress(50)
                    dl.download(ba["base_url"], a_out)
                    widget.set_progress(100)
        except Exception as e:
            print(f"Download error: {e}")

    threading.Thread(target=dl, daemon=True).start()
```

---

### Task 18: Wire Settings page to config

**Files:**
- Modify: `gui_qt/pages/settings.py`

- [ ] **18a: Load/save config from JSON**

```python
from src.merger import FFmpegMerger
import json, os

DATA_DIR = os.path.join(os.path.dirname(__file__), '..', '..', 'data')
os.makedirs(DATA_DIR, exist_ok=True)

def load_config():
    p = os.path.join(DATA_DIR, 'config.json')
    try:
        with open(p) as f: return json.load(f)
    except: return {}

def save_config(cfg):
    p = os.path.join(DATA_DIR, 'config.json')
    with open(p, 'w') as f: json.dump(cfg, f, indent=2)
```

- [ ] **18b: Wire FFmpeg check button**

```python
self.ffmpeg_btn.clicked.connect(self._install_ffmpeg)

def _install_ffmpeg(self):
    import threading
    def task():
        try:
            merger = FFmpegMerger()
            path = merger.auto_download()
            self.ffmpeg_status.setText(f"✅ FFmpeg 已安装: {path}")
        except Exception as e:
            self.ffmpeg_status.setText(f"❌ 安装失败: {e}")
    threading.Thread(target=task, daemon=True).start()
```

---

## Phase 5: Build & Polish

### Task 19: PyInstaller packaging

- [ ] **19a: Create spec file for PyInstaller**

```bash
pyinstaller --onefile --windowed --add-data "gui_qt;gui_qt" --add-data "src;src" --add-data "gui_qt/resources;gui_qt/resources" gui_qt/main.py
```

- [ ] **19b: Verify exe runs without console**

The packaged exe should:
1. Show the main window immediately
2. Have the custom frameless design with rounded corners
3. All navigation pages work
4. API calls to Bilibili work
5. Downloads work in background thread

---

## Key Design Decisions

1. **Frameless window**: `Qt.FramelessWindowHint` removes native titlebar. Custom `TitleBar` widget handles dragging and min/max/close. Semi-transparent background with rounded corners.

2. **Thread safety**: All network calls (API, downloads) run in `threading.Thread` (daemon) to keep UI responsive. Progress bars updated via `widget.set_progress()` calls from worker threads.

3. **Glass panel effect**: Achieved via semi-transparent QSS backgrounds (`rgba(255,255,255,0.6)`) with `border-radius`. No real blur needed — the semi-transparency gives a convincing frosted-glass look.

4. **Sidebar navigation**: `QListWidget` + `QStackedWidget` pattern. `currentRowChanged` signal drives page switching. Clean and matches the React design.

5. **QSS over widget-specific styles**: Global stylesheet for consistent theming, inline `setStyleSheet()` for component-specific overrides where the global QSS can't target nested widgets easily.

## Task Dependency Graph

```
Task 1 ─→ Task 2 ─→ Task 3 ─→ Task 4 ─→ Task 5 ─→ Task 7
                                  ↓
                              Task 6 ──────→ Task 5
                                                 ↓
                           Task 8 ─→ Task 9 ─→ Task 12
                           Task 10 ──────────→ Task 12
                           Task 11 ──────────→ Task 12
                                                 ↓
                          Task 13 (independent)  │
                          Task 14 (independent)  │
                          Task 15 (independent)  │
                                                 ↓
                          Task 16 ─→ Task 17 ─→ Task 18
                                                 ↓
                                              Task 19
```

Tasks 8-11 (widgets) can be built in parallel. Tasks 13-15 (pages) can be built in parallel. Tasks 16-18 (backend integration) depend on the pages existing.
