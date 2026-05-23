# B站充电视频下载器

一个面向 Windows 桌面的 B站视频下载工具。项目当前主线是 `rust-bilibili`：前端使用 React + Tailwind，桌面端使用 Tauri，下载、解析、合并、弹幕处理和本地数据管理由 Rust 后端负责。

> 本项目仅用于个人学习、备份与技术研究。请遵守 B站用户协议、版权规则和内容创作者权益，不要用于未授权传播。

## 项目亮点

- 视频链接解析：支持普通 BV 链接、多个链接、合集/多 P 视频解析。
- 高清下载：支持 360P 到 1080P、1080P+、1080P60、4K、HDR、杜比视界、8K 等画质选项，实际可用画质取决于账号权限和视频源。
- 音频导出：支持音频下载与 MP3 转换。
- 弹幕处理：支持外挂 ASS 弹幕文件，也支持烧录弹幕到 MP4。
- 批量下载：支持合集内当前视频下载、全部下载和多任务队列。
- 本地登录：支持扫码登录，Cookie、配置和历史记录保存在本机。
- 开箱合并：打包时内置 FFmpeg 资源，用于视频/音频合并和弹幕烧录。
- 桌面体验：中文界面、下载历史、设置页、隐藏侧边栏、B站风格视觉。

## 当前主线

```text
rust-bilibili/
```

这是正在维护的 Rust/Tauri 桌面版本。

仓库中还保留了早期原型与迁移资料：

- `bilibili-video-downloader/`：早期 React 前端原型。
- `gui_qt/`、`src/`：早期 Python/PyQt 版本与下载逻辑。
- `docs/`、`plans/`：设计、迁移和实现计划文档。

## 开发环境

- Windows 10/11
- Node.js 18+
- Rust stable
- Tauri 2 相关 Windows 构建依赖

## 本地运行

```powershell
cd rust-bilibili
npm install
npm run tauri:dev
```

## 打包安装包

```powershell
cd rust-bilibili
npm install
npm run tauri:build
```

生成的 NSIS 安装包通常位于：

```text
rust-bilibili/src-tauri/target/release/bundle/nsis/
```

## 本地数据

安装版会优先把配置、历史、Cookie、日志和默认下载目录放在应用安装相关目录下，避免数据分散到多个系统目录。

常见数据包括：

- `config.json`：应用设置。
- `cookies.json`：登录 Cookie。
- `history.json`：下载历史。
- `downloads/`：默认下载目录。
- `logs/`：运行日志和崩溃日志。

## 链接

- 我的 B站主页：[https://space.bilibili.com/228533833](https://space.bilibili.com/228533833)
- 项目仓库：[https://github.com/zhaochengbiao005/bilibili-charging-video-downloader](https://github.com/zhaochengbiao005/bilibili-charging-video-downloader)

## 许可证与声明

仓库目前未声明开源许可证，默认保留全部权利。项目依赖 FFmpeg 完成合并、转码和弹幕烧录，正式分发时请保留 FFmpeg 相关许可说明。
