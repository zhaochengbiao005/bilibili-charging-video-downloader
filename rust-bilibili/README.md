# B站充电视频下载器

Rust/Tauri 全栈版 B站视频下载工具，支持登录、解析、画质选择、并发下载、MP4 合并、MP3 转换、历史记录和本地设置。

## 环境

- Windows 10/11
- Node.js 18+
- Rust stable
- Tauri Windows 打包依赖

## 开发运行

```powershell
npm install
npm run tauri:dev
```

## 打包

```powershell
npm install
npm run tauri:build
```

打包前会自动执行：

```powershell
npm run prepare:ffmpeg
npm run build
```

`prepare:ffmpeg` 会把构建期 `ffmpeg-static` 提供的 `ffmpeg.exe` 和对应许可说明复制到 `src-tauri/resources/ffmpeg/`。生成的二进制和许可拷贝不进入 git，但会被 Tauri 打进安装包资源中。

常见产物路径：

- 安装包：`src-tauri/target/release/bundle/nsis/`
- Release 可执行文件：`src-tauri/target/release/BilibiliChargingDownloader104.exe`

## 本地数据

应用数据默认保存到：

```text
<程序所在目录>/
```

包含：

- `config.json`：设置
- `cookies.json`：登录 Cookie
- `history.json`：下载历史
- `downloads/`：默认下载目录
- `ffmpeg/ffmpeg.exe`：设置页一键安装的兜底 FFmpeg
- `logs/`：运行日志和 `panic.log` 崩溃记录

开发模式下该目录会跟随当前启动的 Tauri 可执行文件；安装包模式下会落在应用安装目录，避免配置、历史和下载文件分散到多个系统目录。

## 验收

1. 运行 `npm run tauri:build`，确认 NSIS 安装包生成。
2. 安装后启动应用，窗口标题应为“B站充电视频下载器”。
3. 打开设置页，FFmpeg 状态应显示“可用，可直接合并”，并能看到资源路径或缓存路径。
4. 使用公开视频链接解析，解析成功后才出现下载设置。
5. 下载视频格式，完成后输出应为 `.mp4`，不应只留下 `.m4s` 和 `.m4a`。
6. 切到下载历史，确认新记录出现，打开目录按钮可打开输出文件所在目录。
7. 断网或使用错误链接时，界面应显示可理解的错误提示。

## 许可提醒

打包内置 FFmpeg 会带来 FFmpeg 对应许可义务。当前准备脚本会复制 `ffmpeg.exe.LICENSE` 和 `ffmpeg.exe.README` 到资源目录；正式分发时请随安装包或发布说明一并保留。
