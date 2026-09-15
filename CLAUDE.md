# Bilibili Charging Video Downloader — CLAUDE.md

主线是 **rust-bilibili/**（Tauri 2 + React + Rust）。上一代实现（Python CLI/PyQt GUI、React 设计稿 fork）已归档到 **legacy/**，只作语义参考，不再演进。领域词表与模块地图见 [CONTEXT.md](CONTEXT.md)。

## Quick Start
```bash
cd rust-bilibili
npm install

# 开发（Tauri 桌面）
npm run tauri dev

# 打包
npm run tauri build

# 验证（改动后必跑）
npm run lint          # tsc --noEmit
npm run build         # vite build
cd src-tauri && cargo test
```

## Project Structure
```
├── rust-bilibili/            # 主线：Tauri 2 + React + Rust
│   ├── src/                  # React 前端（bridge.ts 是唯一后端入口）
│   │   ├── bridge.ts         # typed invoke + 事件注册 + 错误归一化
│   │   ├── types.ts          # 前端契约（与 Rust models 手工同步）
│   │   ├── pages/            # Home / History / Settings / About
│   │   └── components/       # UrlInput / VideoInfo / DownloadOptions / DownloadQueue ...
│   └── src-tauri/src/        # Rust 后端
│       ├── commands.rs       # Tauri command 适配层：校验 + 委托，不含业务逻辑
│       ├── download/         # 下载域
│       │   ├── mod.rs        # 流解析策略（quality_to_qn / 选轨 / first_url / selected_page / ensure_not_cancelled）
│       │   ├── task.rs       # TaskOrchestrator：队列、取消表、事件收尾、两条下载管线
│       │   └── bili_http.rs  # B站 CDN 流原语（UA / 体积探测 / 分片规划 / MD5 / header 解析）
│       ├── cloud/            # 云盘直传（CloudUploader trait：baidu + mock 两个 adapter）
│       ├── streaming/        # B站流读取、上传计划（分片 MD5）
│       ├── models/           # 契约模型；download.rs 的 DownloadStage/TaskStatus 是双端唯一来源
│       └── state.rs          # AppState 组合根（tasks: Arc<TaskOrchestrator>）
├── legacy/                   # 已归档的上一代实现（Python CLI/GUI、React 设计稿、抓包分析产物）
├── plans/                    # 技术分析文档
└── docs/                     # 实施计划与设计 spec
```

## Backend Architecture（Rust）

**数据流：**
```
BVID → client.video_info() → 分P列表 (cid)
  → client.playurl(bvid, cid, qn, fnval) → DASH {video, audio} / DURL [FLV segments]
  → TaskOrchestrator.run_download_task()（selected_page → 选轨 → downloader.download_file）
  → ffmpeg.merge_video_audio() → MP4
  （云盘直传：streaming 读流 → md5_plan → CloudUploader.precreate/upload_part/finish，不落本地大文件）
```

**重要说明（课程视频/Cheese）：**
- 普通视频：`/x/player/playurl` + bvid/cid
- **课程视频**（如 ss21076 专区课程）：**必须**用专用接口 `/pugv/player/web/playurl`（ep_id + avid + cid），普通流程无效。

**关键常量：**
- 画质 qn：360P=16, 480P=32, 720P=64, 1080P=80, 720P60=74, 1080P+=112, 1080P60=116, 4K=120, HDR=125, 杜比=126, 8K=127（唯一实现：`download/mod.rs::quality_to_qn`）
- 音频档：30216=128kbps, 30232=192kbps, 30280=320kbps
- `fnval=4048`: DASH + 全格式
- Cookie 格式：JSON 对象、JSON 数组（Cookie-Editor）、Netscape、KEY=VALUE
- 云盘直传允许写的小文件：`cloud_config.json`、`baidu_token.json`、`cloud_upload_sessions.json`、`history.json`、日志；不允许写任何大体积视频/音频/中间文件

## 约定
- 取消是类型化错误 `AppError::Cancelled`，事件终态是 `TaskStatus`，**禁止**靠中文文案 substring 判断。
- 所有下载/直传管线从 `download/mod.rs`（流解析）和 `download/bili_http.rs`（CDN 原语）取共享实现，不要手抄副本。
- command 只做校验 + 委托；业务逻辑进 `download/task.rs` 或对应域模块。
