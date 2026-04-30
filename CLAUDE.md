# Bilibili Charging Video Downloader — CLAUDE.md

## Quick Start
```bash
pip install -r requirements.txt

# PyQt6 GUI（推荐）
python -m gui_qt.main

# CLI
python main.py info BV1xx411c7mD
python main.py dl BV1xx411c7mD -c cookies.json

# 旧版 ttkbootstrap GUI（备用）
python gui.py
```

### PyInstaller 打包
```bash
pyinstaller --onefile --windowed ^
  --add-data "gui_qt;gui_qt" ^
  --add-data "src;src" ^
  --add-data "login_capture.py;." ^
  --name "BilibiliDownloader" ^
  gui_qt/main.py
```
输出: `dist/BilibiliDownloader.exe`

## Project Structure
```
├── gui_qt/                   # PyQt6 GUI（主界面）
│   ├── app.py                # QApplication + QSS 加载
│   ├── window.py             # 主窗口（frameless + 圆角 + 侧边栏）
│   ├── titlebar.py           # 自定义标题栏（拖拽 + min/max/close）
│   ├── sidebar.py            # 侧边栏导航
│   ├── glass_panel.py        # 磨砂面板基类
│   ├── pages/
│   │   ├── home.py           # 首页（搜索 + 视频信息 + 下载设置 + 队列）
│   │   ├── history.py        # 下载历史
│   │   ├── settings.py       # 设置（FFmpeg/默认画质/速度/目录）
│   │   └── about.py          # 关于对话框
│   ├── widgets/
│   │   ├── url_input.py      # URL 输入
│   │   ├── cookie_input.py   # Cookie + 扫码登录
│   │   ├── video_info.py     # 视频信息卡片
│   │   ├── download_options.py # 下载设置面板
│   │   └── download_queue.py # 下载队列 + 进度条
│   └── resources/styles.qss  # 全局 QSS 主题（B站粉 + 磨砂白）
├── src/                      # Python 后端模块
│   ├── bilibili_api.py       # Bilibili REST API 客户端
│   ├── downloader.py         # 多线程分片下载引擎
│   ├── merger.py             # FFmpeg 合并/转换/自动下载
│   └── __init__.py
├── gui.py                    # 旧版 ttkbootstrap GUI（备用）
├── main.py                   # CLI 入口（info / dl）
├── login_capture.py          # 扫码登录脚本
├── requirements.txt
├── plans/                    # 技术分析文档
├── docs/                     # 实施计划
└── bilibili-video-downloader/ # React 前端源码（Google AI Studio 设计参考）
```

## Backend Architecture

**模块职责：**
- `bilibili_api.py` — REST API 客户端：视频信息、playurl、DASH/DURL、VIP/充电检测、Cookie 验证
- `downloader.py` — 多线程分片下载，Range 请求，连接池，重试
- `merger.py` — FFmpeg 包装：音视频合并、concat、MP3 转换、自动下载

**数据流：**
```
BVID → BilibiliAPI.get_video_info() → 分P列表 (cid)
  → get_playurl(bvid, cid, qn, fnval=4048)
    → DASH: {video_tracks, audio_tracks} (多码率)
    → DURL: [FLV segments] (旧格式)
  → VideoDownloader.download()（并发分片/Range）
  → FFmpegMerger.merge_video_audio() → MP4
```

**关键常量：**
- `QUALITY_MAP`: 360P=16, 480P=32, 720P=64, 1080P=80, 1080P60=116, 4K=120, HDR=125
- `fnval=4048`: DASH + 全格式
- Cookie 格式：JSON 对象、JSON 数组（Cookie-Editor）、Netscape、KEY=VALUE

## PyQt6 GUI 说明

**设计风格：** B站粉 (#fb7299) + 磨砂白卡片 + 无边框圆角窗口

**技术要点：**
- `FramelessWindowHint` 无边框窗口，`TitleBar` 自定义拖拽
- `QListWidget` + `QStackedWidget` 侧边栏导航
- QSS 使用 `*[class="xxx"]` 属性选择器（Qt 不支持 `.class` CSS 语法）
- `QScrollArea` 需额外设置 `scroll.viewport().setStyleSheet()`
- 磨砂效果用实色 `#ffffff` 模拟（Qt 不支持 `backdrop-filter`）
- 后端调用在 `threading.Thread` 中执行，不阻塞 UI

**设计参考：** `bilibili-video-downloader/src/` — Google AI Studio 生成的 React 设计，含 Sidebar、UrlInput、VideoInfo、DownloadOptions、DownloadQueue 等组件
