from PyQt6.QtWidgets import QWidget, QVBoxLayout, QLabel, QHBoxLayout
from PyQt6.QtCore import Qt
from gui_qt.glass_panel import GlassPanel


class VideoInfo(GlassPanel):
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
