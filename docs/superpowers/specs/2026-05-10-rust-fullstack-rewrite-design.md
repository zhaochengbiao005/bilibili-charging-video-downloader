# Rust 全栈重写 — 设计文档

**日期**: 2026-05-10
**范围**: Python 后端 + CLI + PyQt6 GUI → tauri + React + Rust 全栈

## 目标

将 B站充电视频下载器从 Python/PyQt6 全栈迁移到 Rust/tauri/React，
彻底移除 Python 运行时依赖，实现单二进制分发，1:1 还原 B站粉磨砂玻璃设计。

## 架构

```
React Frontend (tauri webview)
  ├── Sidebar / Home / History / Settings / About
  ├── B站粉 (#fb7299) + 磨砂玻璃 (backdrop-filter: blur)
  └── invoke() ↔ emit() Tauri Bridge
Tauri Bridge
  ├── #[tauri::command] 命令调用（请求-响应）
  └── Window::emit() 事件推送（进度回调）
Rust Backend Library
  ├── api.rs     — B站 REST API 客户端 (reqwest + serde)
  ├── dl.rs      — 多分片并发下载引擎 (tokio)
  ├── merger.rs  — FFmpeg 子进程合并 (tokio::process)
  ├── auth.rs    — 扫码登录 (QR 码终端显示 + 轮询)
  ├── cli.rs     — CLI 入口 (clap derive: info / dl)
  └── error.rs   — 统一错误类型 (thiserror)
```

## 模块详设

### api.rs — B站 API 客户端

对应 `src/bilibili_api.py`（270行）。

- `BilibiliClient` struct 持有 `reqwest::Client`（带 cookie jar）
- 每个 API 端点返回强类型 serde struct，不再使用 `dict.get()` 链式访问
- 方法清单：`video_info`, `get_playurl`, `check_login`, `check_charging_status`, `check_video_access`
- 静态工具：`extract_dash_urls`, `extract_durl_urls`

### dl.rs — 下载引擎

对应 `src/downloader.py`（210行）。

- `Downloader` struct 使用 `tokio::sync::mpsc::Sender` 推送进度
- 分片下载：Range 请求 → 临时文件 → 按 order 拼接
- 通过 `mpsc::Receiver` 暴露进度流，tauri bridge 消费并 emit 到前端
- 支持重试（可配置次数）、DASH + DURL 两种格式

### merger.rs — FFmpeg 合并

对应 `src/merger.py`（165行）。

- `FFmpegMerger` struct
- `merge_video_audio(video, audio, output)` — DASH 音视频合并
- `concat_flv(segments, output)` — DURL FLV 拼接
- `check_ffmpeg()` — 检查系统中是否有 FFmpeg
- `auto_download_ffmpeg()` — 自动下载 FFmpeg 二进制

### auth.rs — 扫码登录

对应 `login_capture.py`（95行）。

- `QrCodeLogin` struct
- 调用 B站 QR API 获取登录 URL
- `qrcode` crate 在终端输出 QR 码
- 轮询扫码状态：Pending → Scanned → Confirmed
- 成功后提取 cookie 写入文件

### cli.rs — CLI 入口

对应 `main.py`（265行）。

- `clap` derive 宏定义子命令
- `bilibili-dl info <BVID>` — 视频信息查询
- `bilibili-dl dl <BVID> [OPTIONS]` — 下载（--quality, --output, --cookie）

### error.rs — 统一错误

```rust
#[derive(Error, Debug)]
pub enum AppError {
    #[error("API error {code}: {message}")]
    Api { code: i32, message: String },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Download failed: {0}")]
    Download(String),
    #[error("FFmpeg not found")]
    FFmpegNotFound,
    #[error("Merge failed: {0}")]
    Merge(String),
}
```

序列化为 `serde::Serialize`，通过 tauri command 返回给前端。

## Cargo 依赖

```toml
[package]
name = "bilibili-downloader"
edition = "2024"

[dependencies]
reqwest = { version = "0.12", features = ["cookies", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
thiserror = "2"
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
qrcode = "0.14"
chrono = "0.4"
```

## 项目结构

```
src-tauri/
├── Cargo.toml
├── tauri.conf.json
├── build.rs
├── icons/
├── src/
│   ├── main.rs          # tauri 入口 + CLI 分发逻辑
│   ├── lib.rs            # 库根
│   ├── api.rs            # B站 API 客户端
│   ├── dl.rs             # 下载引擎
│   ├── merger.rs         # FFmpeg 合并
│   ├── auth.rs           # 扫码登录
│   ├── cli.rs            # clap CLI
│   └── error.rs          # 统一错误类型
src/                       # React 前端
├── App.tsx
├── main.tsx
├── components/
│   ├── Sidebar.tsx
│   ├── TitleBar.tsx
│   ├── GlassPanel.tsx
│   ├── UrlInput.tsx
│   ├── VideoInfo.tsx
│   ├── DownloadOptions.tsx
│   ├── QualityCard.tsx
│   └── DownloadQueue.tsx
├── pages/
│   ├── Home.tsx
│   ├── History.tsx
│   ├── Settings.tsx
│   └── About.tsx
├── hooks/
│   ├── useDownload.ts
│   └── useHistory.ts
├── styles/
│   └── global.css
├── types/
│   └── index.ts
└── index.html
```

## 设计风格

- 配色：主色 `#fb7299`（B站粉），渐变 `#fb7299 → #ff85a8`
- 磨砂玻璃：`backdrop-filter: blur(20px)` + `rgba(255,255,255,0.6)` 半透明背景
- 卡片：圆角 `16px`，边框 `rgba(255,255,255,0.7)`，阴影分层
- 字体：系统默认中文字体，标题 `font-weight: 900`
- 动画：hover 缩放 `scale(1.02)`，按钮渐变过渡，侧边栏滑入
- 参考：`bilibili-video-downloader/src/` 中 Google AI Studio 生成的 React 组件

## 关键路径

1. **视频信息流**：用户输入 BVID → invoke("fetch_video", {bvid}) → api.rs → VideoInfo → JSON → React 渲染
2. **下载流**：用户点下载 → invoke("start_download", {bvid, cid, qn}) → dl.rs 并发下载 → emit progress → merger.rs 合并 → emit complete
3. **扫码登录**：用户触发 → auth.rs 生成 QR → 终端显示 → 轮询状态 → emit 登录成功 → 保存 cookie
4. **CLI 模式**：编译时 feature gate 或 cargo alias 直接运行为 CLI 工具