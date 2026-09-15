# CONTEXT — B站充电视频下载器

领域词表 + 模块地图。架构词汇（module / interface / depth / seam / adapter / leverage / locality）与 `/codebase-design` 技能对齐。

## 领域词表

- **视频解析**（fetch_info）→ 视频信息、分 P（cid）、DASH / DURL 流、画质 qn、充电试看（preview stream）
- **下载任务**（task_id 前缀 `dl_` / `cloud_` / `ffmpeg_`）→ 编排器 `spawn_queued` → 队列（2 并发槽）、取消令牌表、进度/完成事件
- **云盘直传**（百度网盘）→ `precreate → upload_part → finish`；上传会话（`cloud_upload_sessions.json`）重启后复用、跳过已传分片
- **双遍 FFmpeg 管道**（MP4PipeExperimental）→ 第一遍 stdout 生成 fragment MP4 并算分片 MD5，第二遍同字节流上传
- **弹幕**（DanmakuMode）→ 外挂 `.ass`（Ass）/ 烧录进视频（Burn）/ 无（None）；取舍由后端 `effective_danmaku_mode` 统一判定
- **互动视频**（stein gate）→ 剧情图（segments/nodes/choices）、整图下载、路径下载、离线播放器
- **任务阶段**（DownloadStage）/ **任务终态**（TaskStatus）→ Rust `models/download.rs` 是双端唯一来源，serde 输出 snake_case

## 模块地图（rust-bilibili/src-tauri/src）

- `commands.rs` — Tauri command 适配层：校验 + 委托，不含业务逻辑
- `download/`
  - `mod.rs` — 流解析策略（deep module）：`quality_to_qn` / `select_video_track` / `select_audio_track` / `first_url` / `selected_page` / `ensure_not_cancelled`（取消是 `AppError::Cancelled`，不是散文）
  - `task.rs` — **TaskOrchestrator**（任务编排 module）：`spawn_queued` 统一队列/取消/事件收尾；`run_download_task` / `run_stein_download_task`
  - `bili_http.rs` — B站 CDN 流原语（deep module）：`BILIBILI_UA` / `probe_content_size` / `plan_ranges` / `md5_hex` / header 解析
- `cloud/` — **CloudUploader trait 是真实 seam**（两个 adapter：`baidu.rs` + `mock.rs`）；`direct.rs` 的 `run_cloud_upload_task` 接受注入；`session.rs` 上传会话复用
- `streaming/` — `bili_stream.rs` B站流读取（Range）；`md5_plan.rs` 上传计划
- `models/` — 契约模型；`download.rs` 的 `DownloadStage` / `TaskStatus` 双端唯一
- `state.rs` — AppState 组合根；`tasks: Arc<TaskOrchestrator>`
- 前端 `bridge.ts` — 唯一后端入口：typed invoke + 事件注册 + 错误归一化（`AppError` kind 只在此处拍平）

## 事件

- `download://progress` — DownloadProgressEvent（stage 是 DownloadStage）
- `download://completed` / `download://failed` — DownloadDoneEvent（status 是 TaskStatus；取消不是文案）

## 决策记录

- 2026-09-14 架构评审落地：CloudUploader 注入（direct.rs 不再内部 new 死 Baidu）、任务编排 module、typed 事件契约、bili_http 原语；上一代实现归档 `legacy/`。
- 暂不引入 ts-rs/specta 类型代码生成（新依赖 + 构建步骤）；以 `stage_serialization_matches_ts_union` 测试锁词表，后续可逆。
- 前端 bridge.ts 不再复写后端默认值/弹幕取舍逻辑——interface 收窄，pass-through 是 seam 的窄面，保留。
