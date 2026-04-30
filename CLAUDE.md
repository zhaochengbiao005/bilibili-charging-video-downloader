# Bilibili Charging Video Downloader — CLAUDE.md

## Quick Start
```bash
pip install -r requirements.txt
python gui.py                    # 启动 GUI（ttkbootstrap，当前可用版本）
python main.py info BV1xx411c7mD # CLI: 查看视频流信息
python main.py dl BV1xx411c7mD -c cookies.json # CLI: 下载
```

## Project Structure
```
破解B站充电视频测试/
├── gui.py                    # ttkbootstrap GUI（当前可用，后续替换为 PyQt6）
├── main.py                   # CLI 入口（info / dl 子命令）
├── login_capture.py          # 扫码登录脚本（由 GUI 子进程调用）
├── requirements.txt          # Python 依赖
├── CLAUDE.md
├── src/                      # Python 后端模块
│   ├── bilibili_api.py       # Bilibili REST API 客户端
│   ├── downloader.py         # 多线程分片下载引擎
│   ├── merger.py             # FFmpeg 合并/转换/自动下载
│   └── __init__.py
├── data/                     # 程序数据（Cookie/config/history，自动创建）
├── downloads/                # 默认下载目录
├── plans/                    # 技术分析文档
└── bilibili-video-downloader/ # React 前端源码（Google AI Studio 设计）
    ├── src/
    │   ├── App.tsx           # 路由 + 模态框
    │   ├── index.css         # Tailwind + 毛玻璃样式
    │   ├── bridge.ts         # HTTP API 桥接层（Electron/Flask 方案已废弃）
    │   ├── types.ts
    │   ├── components/
    │   │   ├── Sidebar.tsx       # 侧边栏
    │   │   ├── UrlInput.tsx      # 链接 + Cookie 输入
    │   │   ├── VideoInfo.tsx     # 视频信息卡片
    │   │   ├── DownloadOptions.tsx # 下载设置
    │   │   └── DownloadQueue.tsx # 下载队列
    │   └── pages/
    │       ├── Home.tsx      # 首页（解析 + 下载）
    │       ├── History.tsx   # 下载历史
    │       ├── Settings.tsx  # 设置
    │       └── About.tsx     # 关于（模态框）
    ├── package.json
    └── vite.config.ts
```

## Backend Architecture

**核心模块职责：**
- `bilibili_api.py` — REST API 客户端，处理视频信息获取、playurl 解析、DASH/DURL 提取、VIP/充电状态检测、Cookie 验证
- `downloader.py` — 多线程分片下载，支持 Range 请求、连接池、重试
- `merger.py` — FFmpeg 包装：音视频合并（DASH）、concat 合并、MP3 转换、自动下载 FFmpeg

**数据流：**
```
BVID → BilibiliAPI.get_video_info() → 获取分P列表 (cid list)
  → 对每个分P: get_playurl(bvid, cid, qn, fnval=4048)
    → DASH: {video_tracks, audio_tracks} (多码率)
    → DURL: [FLV segments] (旧格式)
  → VideoDownloader.download()（并发下载分片/Range）
  → FFmpegMerger.merge_video_audio()（合并为 MP4）
```

**关键常量：**
- `QUALITY_MAP`: 360P=16, 480P=32, 720P=64, 1080P=80, 1080P60=116, 4K=120, HDR=125
- `fnval=4048`: 启用 DASH + 全格式
- Cookie 格式支持：JSON 对象、JSON 数组（Cookie-Editor）、Netscape、KEY=VALUE

## PyQt6 重构计划

当前 `gui.py`（ttkbootstrap）将被 PyQt6 版本替代。

**前端设计参考：** `bilibili-video-downloader/src/`（Google AI Studio 生成的 React 设计）
- 设计风格：毛玻璃（glassmorphism）+ B站粉 (#fb7299) + 暗色/浅色混合
- 布局结构：侧边栏导航 + 主内容区（多页面切换）
- 页面：首页（输入+解析+下载）、历史记录、设置、关于

**需要复刻的组件：**
1. 主窗口框架 — 无边框、圆角、阴影、半透明背景
2. 侧边栏 — Logo、导航项、底部支持按钮
3. 首页 — 搜索栏、视频信息卡片、下载配置面板、下载队列
4. 历史记录 — 列表、搜索、删除
5. 设置 — 默认配置、FFmpeg 安装、路径管理
6. 关于 — 版本信息、开源说明

**PyQt6 可实现的效果清单**
| 设计元素 | PyQt6 实现方案 |
|----------|---------------|
| 毛玻璃半透明背景 | `setAttribute(Qt.WA_TranslucentBackground)` + QSS rgba |
| 圆角边框 | `border-radius` QSS |
| 阴影 | `QGraphicsDropShadowEffect` |
| 侧边栏导航 | `QListWidget` + `QStackedWidget` |
| 粉色主题 | 全局 QSS 变量 + 调色板 |
| 自定义标题栏 | `setWindowFlags(Qt.FramelessWindowHint)` + 自定义 drag |
| 下载队列进度条 | `QProgressBar` QSS 样式 |
| 滚动画质选择 | `QListWidget` radio 样式 |
