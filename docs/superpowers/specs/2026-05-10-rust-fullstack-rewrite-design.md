# React + Tauri + Rust 一站式重写计划

**日期**: 2026-05-10
**调整日期**: 2026-05-21
**范围**: 现有 Python/PyQt 原型 + `bilibili-video-downloader` React 前端 → Tauri 2 + React + Rust 后端
**核心原则**: 前端设计不重做，后端高性能重写，最终交付仍是一站式桌面下载器。

## 1. 项目定位

本项目的目标不是做一个“只会下载的命令行工具”，而是做一个完整的一站式 B站视频下载桌面应用：

- 用户可以在同一个应用里完成登录、解析、选择画质、下载、合并、查看历史、配置 FFmpeg 和输出目录。
- React 前端以 `bilibili-video-downloader/src/` 中 Google AI Studio 已生成的设计为基础，只做工程化接入和必要体验修正。
- Rust 后端替代 Python 原型，负责网络请求、Cookie、分片下载、任务调度、FFmpeg 调用、历史记录和本地配置。
- Tauri 负责桌面壳、前后端 IPC、窗口能力、文件系统权限和最终打包分发。

最终交付物应该是一个普通用户可以直接打开使用的桌面程序，而不是需要用户手动安装 Python、手动启动服务、手动配置脚本的开发者工具。

## 2. 路线决策

选择 **Rust + Tauri + React**，不再继续扩展 PyQt 主线，也不优先采用 C/C++ 重写。

理由：

- 现有 React 前端已经成型，Tauri 可以自然承载该前端。
- 下载器主要瓶颈是网络 I/O、文件 I/O、并发调度和外部 FFmpeg 进程，不是手写 C/C++ 算法。
- Rust 的 `tokio`、`reqwest`、`serde`、`thiserror` 更适合快速搭建可靠后端。
- Tauri 的 command/event 模型可以直接替换当前 `bridge.ts` 中的假 HTTP API。
- Rust 比 C/C++ 更容易控制并发取消、任务生命周期、错误边界和资源释放。

C/C++ 只保留为未来可选方向：如果后续需要把下载核心嵌入其他 C++ 项目，可以再把稳定后的 Rust 核心通过 FFI 或单独库形式暴露。

## 3. 现有资产分工

| 资产 | 去留 | 用途 |
|---|---|---|
| `bilibili-video-downloader/src/` | 保留并接入 | 作为主 UI 和交互稿 |
| `src/*.py` | 参考后迁移 | 提取 API、下载、FFmpeg 语义 |
| `main.py` | 参考后迁移 | CLI 行为和参数设计参考 |
| `gui_qt/` | 冻结，不继续扩展 | 作为旧版可运行参考 |
| `login_capture.py` | 参考后替换 | 扫码登录能力迁移到 Rust/Tauri |
| `rust-bilibili/` | 修复并作为新主线 | Tauri + Rust 工程目录 |

## 4. 总体架构

```text
React Frontend
  - Home / History / Settings / About
  - UrlInput / VideoInfo / DownloadOptions / DownloadQueue
  - 只通过 bridge.ts 调用后端
        |
        | invoke(command) / listen(event)
        v
Tauri IPC Layer
  - commands: 请求-响应
  - events: 下载进度、日志、任务完成、登录状态
        |
        v
Rust Backend
  - api: B站 REST API、playurl、DASH/DURL 解析
  - auth: Cookie 文件、登录状态、扫码登录
  - downloader: 高性能并发下载、Range、重试、取消、限速
  - task: 任务队列、状态机、事件推送
  - ffmpeg: 检查、自动安装、合并、转码
  - storage: 配置、历史记录、Cookie、本地路径
  - cli: 可选 CLI 入口
```

关键约束：

- 前端不得直接访问本地文件系统，统一走 Tauri command。
- 前端不得自己生成最终下载任务 ID，任务 ID 由后端创建并返回。
- 下载进度以事件流推送，前端只订阅并渲染。
- 所有错误都必须变成结构化错误，不把 Rust panic 或原始 stderr 直接透给 UI。

## 5. 前端接入计划

### 5.1 保留的设计

继续使用现有 Google AI Studio 设计：

- 侧边栏导航：`Sidebar`
- 首页解析与下载：`Home`
- URL/Cookie 输入：`UrlInput`
- 视频信息卡片：`VideoInfo`
- 下载选项：`DownloadOptions`
- 下载队列：`DownloadQueue`
- 历史记录：`History`
- 设置页：`Settings`
- 关于页：`About`

视觉方向保留 B站粉、磨砂白、圆角卡片和轻量动效，但实现时要收敛以下问题：

- 不再把所有交互都做成本地 mock。
- 减少纯装饰性状态，所有状态必须能对应真实后端事件。
- 小按钮、隐藏按钮、hover 才显示的关键动作要补可访问性标签。
- 文件夹选择、打开目录、取消下载、扫码登录都必须接真实 command。

### 5.2 `bridge.ts` 改造

当前 `bridge.ts` 使用本地 HTTP `/api/*` 形式。Tauri 重写后改成唯一前后端入口：

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
```

前端只保留以下 API：

```ts
export async function fetchInfo(input: FetchInfoRequest): Promise<VideoInfoResponse>;
export async function startDownload(input: StartDownloadRequest): Promise<StartDownloadResponse>;
export async function cancelDownload(taskId: string): Promise<void>;
export async function pauseDownload(taskId: string): Promise<void>;
export async function resumeDownload(taskId: string): Promise<void>;

export async function getHistory(): Promise<HistoryItem[]>;
export async function deleteHistoryItem(id: string): Promise<void>;
export async function clearHistory(): Promise<void>;

export async function getConfig(): Promise<AppConfig>;
export async function saveConfig(config: AppConfig): Promise<void>;
export async function chooseOutputDir(): Promise<string | null>;
export async function openPath(path: string): Promise<void>;

export async function checkFfmpeg(): Promise<FfmpegStatus>;
export async function installFfmpeg(): Promise<StartTaskResponse>;

export async function checkLogin(): Promise<LoginStatus>;
export async function startQrLogin(): Promise<QrLoginStartResponse>;
export async function logout(): Promise<void>;
```

事件：

```ts
download://progress
download://log
download://completed
download://failed
auth://qr-updated
auth://login-updated
ffmpeg://install-progress
```

### 5.3 前端状态模型

`types.ts` 需要从展示模型扩展为后端契约模型。

```ts
export type DownloadStage =
  | 'queued'
  | 'resolving'
  | 'downloading_video'
  | 'downloading_audio'
  | 'downloading_segments'
  | 'merging'
  | 'converting_audio'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'paused';

export interface DownloadProgressEvent {
  taskId: string;
  stage: DownloadStage;
  percent: number;
  bytesDone: number;
  bytesTotal: number;
  speedBytesPerSec: number;
  etaSeconds?: number;
  message?: string;
}
```

视频信息也要包含真实可下载流，而不只是字符串画质：

```ts
export interface StreamOption {
  id: string;
  qn: number;
  label: string;
  codec?: string;
  width?: number;
  height?: number;
  frameRate?: string;
  bandwidth?: number;
  requiresLogin: boolean;
  requiresVip: boolean;
  available: boolean;
  unavailableReason?: string;
}
```

## 6. Rust 后端模块设计

### 6.1 `src-tauri/src/lib.rs`

库根，注册模块和 command。

职责：

- 初始化 `AppState`
- 注册 Tauri commands
- 注册事件推送工具
- 统一错误转换

### 6.2 `state.rs`

全局应用状态。

```rust
pub struct AppState {
    pub client: BilibiliClient,
    pub task_manager: TaskManager,
    pub config_store: ConfigStore,
    pub history_store: HistoryStore,
    pub cookie_store: CookieStore,
    pub ffmpeg: FfmpegManager,
}
```

要求：

- 使用 `Arc` 包装共享状态。
- 对任务表使用 `tokio::sync::RwLock` 或 `DashMap`。
- 不在 command 中持有锁执行长时间 I/O。

### 6.3 `api.rs`

B站 REST API 客户端。

迁移来源：`src/bilibili_api.py`

职责：

- `check_login`
- `video_info`
- `playurl`
- `check_video_access`
- `extract_dash_streams`
- `extract_durl_segments`

性能和稳定性要求：

- 使用一个长期复用的 `reqwest::Client`。
- 配置 cookie jar、默认 UA、Referer、Origin。
- 所有请求设置超时。
- API 响应使用 `serde` 强类型，不使用无约束 `serde_json::Value` 贯穿业务。
- 对 B站错误码保留原始 `code` 和 `message`。

### 6.4 `auth.rs`

登录和 Cookie 管理。

职责：

- 解析 JSON / Cookie-Editor / Netscape cookie 文件。
- 保存和读取默认 Cookie。
- 检查登录状态。
- 启动扫码登录。
- 登录成功后更新 `CookieStore` 并通知前端。

扫码登录建议：

- 优先使用 B站公开 QR 登录接口。
- 前端显示二维码图片或二维码 URL。
- 后端轮询扫码状态。
- 不再依赖 Playwright 作为默认路径。
- Playwright 只作为未来 fallback，不进入 MVP。

### 6.5 `downloader.rs`

高性能下载核心。

职责：

- DASH 单文件下载。
- DURL 多段下载。
- Range 分块下载。
- 自适应并发。
- 重试和指数退避。
- 取消、暂停、恢复。
- 实时进度统计。

关键优化：

- 大文件分块写入临时文件，不把整个响应读进内存。
- Range 请求只接受 `206 Partial Content`；如果返回 `200 OK`，立即回退单线程流式下载。
- 每个 chunk 写入独立 `.part` 文件或使用随机写入并校验长度。
- 下载完成后原子 rename，避免半成品被当成成品。
- 进度事件限频，例如每 100ms 或每 1MB 推送一次，避免 UI 被刷爆。
- 并发数按文件大小、网络错误率和用户设置动态调整。
- 所有临时文件放入任务专属目录，失败后可清理或恢复。

### 6.6 `task.rs`

下载任务状态机。

职责：

- 创建任务 ID。
- 维护任务状态。
- 管理取消 token。
- 向前端推送进度事件。
- 写入历史记录。

状态流：

```text
queued
  -> resolving
  -> downloading_video / downloading_audio / downloading_segments
  -> merging / converting_audio
  -> completed

任意运行状态
  -> paused
  -> cancelled
  -> failed
```

要求：

- 后端任务 ID 是唯一真实 ID。
- 前端创建占位任务后必须用后端返回 ID 替换。
- 取消必须能中断网络请求和 FFmpeg 子进程。
- 任务失败必须保留错误码、错误阶段和用户可读信息。

### 6.7 `ffmpeg.rs`

FFmpeg 管理和合并。

职责：

- 检测系统 PATH。
- 检测应用缓存目录。
- 自动下载 FFmpeg。
- 合并 DASH 音视频。
- 合并 DURL/FLV。
- 音频转 MP3。

安全和可靠性：

- 下载 FFmpeg 后校验 zip 文件结构。
- 只提取 `ffmpeg.exe`，防止 zip slip。
- FFmpeg 参数使用数组传参，不拼 shell 字符串。
- 捕获 stderr，转换为结构化错误。
- 合并输出先写临时文件，成功后 rename 到最终路径。

### 6.8 `storage.rs`

本地持久化。

目录建议：

```text
<程序所在目录>/
  config.json
  history.json
  cookies.json
  ffmpeg/ffmpeg.exe
  downloads/
  logs/app.log
  tasks/
```

职责：

- 配置读写。
- 历史记录读写。
- Cookie 存储。
- 日志文件。
- 任务临时目录。

要求：

- JSON 写入使用临时文件 + 原子替换。
- 历史记录限制最大条数。
- Cookie 文件不写入项目目录。

### 6.9 `commands.rs`

Tauri command 层。

命令命名：

```rust
#[tauri::command]
async fn fetch_info(input: FetchInfoRequest, state: State<'_, AppState>) -> Result<VideoInfoResponse, AppError>;

#[tauri::command]
async fn start_download(input: StartDownloadRequest, app: AppHandle, state: State<'_, AppState>) -> Result<StartDownloadResponse, AppError>;

#[tauri::command]
async fn cancel_download(task_id: String, state: State<'_, AppState>) -> Result<(), AppError>;
```

规则：

- command 只做参数校验、调用服务、返回结果。
- 长任务必须 spawn 到后台。
- command 不直接操作 UI，不阻塞 WebView。

### 6.10 `cli.rs`

保留 CLI，但不要让 CLI 主导架构。

命令：

```bash
bilibili-dl info <BVID> [--cookie path]
bilibili-dl dl <BVID> [--quality 80] [--output downloads] [--cookie path] [--audio-only]
bilibili-dl login
bilibili-dl ffmpeg check
```

CLI 复用同一套 Rust 后端模块，不复制业务逻辑。

## 7. 高性能目标

### 7.1 下载性能

目标：

- 支持单文件 Range 多分块下载。
- 支持多 DURL segment 并发下载。
- 支持 DASH video/audio 并发下载。
- 支持用户选择 4/8/16/32 并发档位。
- 支持失败 chunk 单独重试。

验收：

- 同一公开测试视频，Rust 下载结果和 Python 原型输出大小一致。
- Range 不支持时不会生成重复拼接文件。
- 取消下载后 2 秒内停止网络和磁盘写入。
- 大文件下载过程中内存不随文件大小线性增长。

### 7.2 UI 事件性能

目标：

- 下载期间 UI 可滚动、可切页、可取消。
- 进度刷新平滑但不刷爆事件队列。

验收：

- 同时 3 个任务下载时，前端不卡死。
- 进度事件频率限制在每任务每秒 5-10 次。
- 任务日志不造成 React 大量重渲染。

### 7.3 启动和打包

目标：

- 发布包不依赖 Python。
- 首次启动可以自动创建配置目录。
- 没有 FFmpeg 时能在设置页一键安装。

验收：

- 全新 Windows 环境双击可启动。
- `ffmpeg` 不存在时，设置页能正确显示“未安装”。
- 自动安装完成后无需重启即可合并视频。

## 8. 一站式服务能力

MVP 必须覆盖：

- 粘贴 BVID 或 B站 URL。
- 自动解析视频标题、封面、UP 主、分P、可用画质。
- 支持选择画质和视频/音频格式。
- 支持 Cookie 文件导入。
- 支持扫码登录。
- 支持 FFmpeg 检测和一键安装。
- 支持下载队列、进度、速度、取消。
- 支持下载历史、打开目录、删除记录。
- 支持设置默认画质、默认并发、默认输出目录、自动合并。

后续增强：

- 批量分P选择。
- 断点续传。
- 下载限速。
- 代理设置。
- 任务失败后重试。
- 多账号 Cookie 管理。
- 日志导出。

## 9. 错误模型

Rust 后端统一返回：

```rust
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    #[error("B站 API 错误 {code}: {message}")]
    Api { code: i32, message: String },

    #[error("网络请求失败: {message}")]
    Network { message: String },

    #[error("登录状态无效，请重新登录")]
    AuthRequired,

    #[error("当前账号无权访问该视频")]
    PermissionDenied { reason: String },

    #[error("下载失败: {message}")]
    Download { task_id: Option<String>, message: String },

    #[error("FFmpeg 未安装")]
    FfmpegNotFound,

    #[error("合并失败: {message}")]
    Merge { message: String },

    #[error("本地文件错误: {message}")]
    Io { message: String },
}
```

前端展示原则：

- 用户看到中文可理解错误。
- 详情日志可展开。
- 权限类错误要提示登录或 Cookie。
- FFmpeg 类错误要引导去设置页安装。

## 10. 项目结构

建议把 `rust-bilibili` 作为新主线目录，结构如下：

```text
rust-bilibili/
  package.json
  vite.config.ts
  tsconfig.json
  index.html
  src/
    App.tsx
    bridge.ts
    types.ts
    components/
    pages/
    styles/
  src-tauri/
    Cargo.toml
    tauri.conf.json
    build.rs
    icons/
    src/
      main.rs
      lib.rs
      state.rs
      commands.rs
      error.rs
      api.rs
      auth.rs
      downloader.rs
      task.rs
      ffmpeg.rs
      storage.rs
      cli.rs
      models/
        mod.rs
        video.rs
        download.rs
        config.rs
        history.rs
```

迁移时可以先把 `bilibili-video-downloader/src/` 复制到 `rust-bilibili/src/`，再逐步把 `bridge.ts` 替换成 Tauri 实现。

## 11. 依赖建议

### Rust

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"

tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["cookies", "json", "stream", "gzip", "brotli"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
futures-util = "0.3"
tokio-util = "0.7"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
dirs = "6"
sanitize-filename = "0.5"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### 前端

保留：

- `react`
- `react-dom`
- `react-router-dom`
- `lucide-react`
- `tailwindcss`
- `vite`

建议移除：

- `@google/genai`
- `express`
- `dotenv`
- 任何只服务 AI Studio 云端模板的依赖

## 12. 分阶段实施计划

### Phase 0: 工程修复和前端搬迁

- [x] 修复 `rust-bilibili/src-tauri` 基础 Tauri 工程，补齐 `src/lib.rs`、`src/main.rs`、`tauri.conf.json`。
- [x] 将 `bilibili-video-downloader/src/` 作为新前端主线迁入或链接到 `rust-bilibili/src/`。
- [x] 清理无关依赖，确保 `npm run build` 和 `cargo check` 通过。
- [x] 建立 `bridge.ts` 的 Tauri invoke/listen 封装。
- [x] 完成当前前端体验修正：全中文、解析前精简、非主页覆盖式弹窗、登录弹窗自适应、设置页不提前显示下载参数。

验收：

- [x] `npm run build` 成功。
- [x] `cargo check` 成功。
- [ ] Tauri 窗口能启动并显示现有 React 界面。

阶段记录（2026-05-21）：

- 已完成 `rust-bilibili` 前端迁移、Tauri 2 配置、基础 Rust command/state/storage/API/FFmpeg 模块骨架。
- 已验证 `npm run build`、`npm run lint`、`cargo check` 成功。
- 已用静态浏览器预览确认 React 主界面可渲染；Tauri 桌面窗口启动仍待 `npm run tauri:dev` 手工验收。
- 已根据当前产品目标修正首页骨架：界面保持全中文，解析前只显示链接输入与提示，解析成功后再展示视频信息和下载设置；除主页外的界面统一采用覆盖式弹窗，并让底层主页虚焦。
- 已新增本地 SVG 视觉资产：Q版 22/33 登录装饰、左上角阿卡林剪影登录头像、二维码登录模拟图；移除 Google 模板占位小人方向。
- 已用静态预览逐项点击验收：下载历史、设置、关于、登录均为覆盖式弹窗，关闭后回到首页。
- 已确认下载设置只在首页解析成功后出现；设置弹窗改为应用级配置，不再提前展示默认画质、下载速度等下载流参数。

### Phase 1: 前后端契约落地

- [x] 重写 `types.ts`，定义真实请求、响应、事件和错误类型。
- [x] 实现 `fetch_info` command。
- [x] 实现 `get_config` / `save_config` / `choose_output_dir` / `open_path`。
- [x] 前端 Home / Settings / History 不再调用假 HTTP `/api/*`。

验收：

- [x] 输入 BVID 后能通过 Rust 后端返回真实视频信息。
- [x] 设置页能读写本地配置。
- [x] 打开目录使用 Tauri command，不再用 `saveConfig` 伪装。

阶段记录（2026-05-21）：

- 已把前端 `types.ts` 扩展为真实契约模型：`FetchInfoRequest/Response`、`StreamOption`、下载事件、配置读写请求/响应、结构化错误载荷。
- `fetch_info` 现在返回 `FetchInfoResponse { video }`，Rust API 客户端走 B站 `/x/web-interface/view` 真实接口，并为前端生成可用 `streams/qualities`。
- `get_config` / `save_config` 改为 `ConfigResponse` 包裹，设置页读写落到本地 `config.json`；新增配置 roundtrip 单测。
- `choose_output_dir` 接入 Tauri dialog 插件，`open_path` 接入 Tauri opener 插件；前端不再通过假 HTTP 或 `saveConfig` 伪装目录操作。
- 验证通过：`npm run lint`、`npm run build`、`cargo test`、`BILI_LIVE_TEST_BVID=BV1xx411c7mD cargo test public_video_info_contract_returns_streams_when_enabled -- --nocapture`。

### Phase 2: 登录和权限

- [x] 实现 Cookie 文件解析。
- [x] 实现默认 Cookie 保存和读取。
- [x] 实现 `check_login`。
- [x] 实现扫码登录 MVP。
- [x] 前端显示登录状态、账号名、权限提示。

验收：

- [x] 无 Cookie 可解析公开视频。
- [ ] 有 Cookie 可解析需要登录权限的视频。
- [ ] Cookie 失效时前端显示明确提示。

阶段记录（2026-05-21）：

- 新增 Rust `auth.rs`，支持 JSON 对象、Cookie-Editor 数组、Netscape、`KEY=VALUE` Cookie 文本解析，并补齐单元测试。
- 新增默认 Cookie 存储：导入后的登录 Cookie 写入 `%APPDATA%/BilibiliDownloader/cookies.json`，不写入项目目录；配置/历史/Cookie 均走本地应用数据目录。
- `check_login` 接入 B站 `/x/web-interface/nav`，返回账号名、UID、等级、大会员类型和中文状态提示。
- `start_qr_login` / `poll_qr_login` 接入 B站公开二维码登录接口，前端展示真实二维码 SVG 并轮询扫码状态；扫码成功后自动保存 Cookie。
- 前端登录弹窗新增 Cookie 文件导入、退出登录、扫码二维码刷新；侧栏显示当前登录账号和等级；视频解析会携带默认 Cookie 并把登录状态写回视频信息。
- 下载设置对需要登录的高清画质显示 `需登录` 标签和权限提示。
- 验证通过：`npm run lint`、`npm run build`、`cargo test`、`BILI_LIVE_TEST_BVID=BV1xx411c7mD cargo test public_video_info_contract_returns_streams_when_enabled -- --nocapture`。
- 待真实账号验收：需要用户扫码或提供有效/失效 Cookie 后，确认受限视频解析和失效 Cookie 提示。

### Phase 3: 高性能下载核心

- [x] 实现 DASH video/audio 下载。
- [x] 实现 DURL segment 下载。
- [x] 实现 Range 分块下载。
- [x] 实现任务状态机和进度事件。
- [x] 实现取消下载。
- [x] 前端 DownloadQueue 使用后端 taskId 和真实进度。

验收：

- [ ] 下载公开测试视频成功。
- [x] 下载时 UI 不阻塞。
- [ ] 取消任务能停止下载。
- [x] Range 不支持时自动回退，不生成损坏文件。

阶段记录（2026-05-21）：

- 新增 `fetch_playurl` command 和强类型 `PlayUrlResponse`，接入 B站 `/x/player/playurl`，支持 DASH 与 DURL 元数据解析。
- 新增 `downloader.rs`，实现流式写入、Range 分块下载、Range 返回 `200 OK` 时自动回退普通流式下载、临时文件完成后原子 rename。
- `start_download` 从占位事件改为后台真实任务：解析第一个分P、下载 DASH 视频/音频原始流，或下载 DURL 分段并串接为 FLV；DASH 音视频合并仍按计划留到 Phase 4。
- 新增取消 token 表，`cancel_download` 可标记后台任务取消；任务结束后清理 token。
- 前端 DownloadQueue 使用后端返回的 `taskId`、真实 progress/completed/failed/cancelled 事件更新状态和错误信息。
- 验证通过：`npm run lint`、`npm run build`、`cargo test`、`BILI_LIVE_PLAYURL_BVID=BV1xx411c7mD cargo test public_playurl_contract_returns_downloadable_streams_when_enabled -- --nocapture`。
- 待桌面实机验收：公开视频完整下载、取消 2 秒内停止、DASH 原始流下载结果人工检查。

### Phase 4: FFmpeg 和一站式体验

- [x] 实现 FFmpeg 检测。
- [x] 实现一键安装 FFmpeg。
- [x] 实现 DASH 音视频合并。
- [x] 实现音频 MP3 转换。
- [x] 实现下载历史写入。

验收：

- [x] 无 FFmpeg 时设置页提示安装。
- [ ] 安装完成后可直接合并。
- [ ] 下载完成后历史页出现记录。
- [x] 历史页可打开输出目录。

阶段记录（2026-05-21）：

- `check_ffmpeg` 已按“打包内置资源 -> 程序所在目录缓存 -> 系统 PATH”的顺序检测 FFmpeg；设置页能显示可用状态和实际路径。
- `npm run prepare:ffmpeg` 会从构建期 `ffmpeg-static` 复制 `ffmpeg.exe` 与对应 LICENSE/README 到 `src-tauri/resources/ffmpeg/`；`tauri build` 前会自动执行，安装包可携带开箱即用的合并/转码能力。
- `install_ffmpeg` 从 gyan.dev 下载 Windows essentials ZIP，提取前只接受安全的 `bin/ffmpeg.exe` 条目，并通过下载进度事件驱动设置页进度条。
- 视频下载默认输出 MP4：DASH 视频/音频下载完成后调用 FFmpeg copy 合并，成功后清理 `.m4s/.m4a` 临时原始流。
- 音频下载默认输出 MP3：先拉取 DASH 音频流，再通过 FFmpeg 转码为 `.mp3`。
- 下载完成会写入程序所在目录的 `history.json`，默认下载目录为程序所在目录下的 `downloads/`，并按配置中的 `max_history` 截断；历史页可读取、删除、清空记录，打开按钮会打开输出文件所在目录。
- 合并临时文件名已改为 `*.tmp.mp4` / `*.tmp.mp3`，避免 FFmpeg 因 `*.mp4.tmp` 这类不可识别扩展报错。
- 验证通过：`npm run prepare:ffmpeg`、`npm run lint`、`cargo test`。安装完成后合并与历史出现记录仍需在真实 Tauri 窗口中用实际下载任务复验。

### Phase 5: 打包和发布质量

- [x] 配置 Tauri Windows 打包。
- [x] 图标、应用名、版本号、安装目录确认。
- [x] 日志文件和崩溃信息落盘。
- [x] 清理前端 AI Studio 残留依赖。
- [x] 编写最终 README。

验收：

- [ ] 全新 Windows 环境可安装/启动。
- [x] 不依赖 Python。
- [x] 常见错误有用户可理解提示。
- [x] 发布包包含完整一站式能力。

阶段记录（2026-05-21 / 2026-05-22）：

- Tauri 打包目标收敛为 Windows NSIS：`bundle.targets = ["nsis"]`，产品名和窗口标题改为“B站充电视频下载器”，安装模式为当前用户安装，安装器语言为简体中文。
- 打包图标使用 `src-tauri/icons/icon.ico`，版本号更新为 `1.0.4`，主程序名为 `BilibiliChargingDownloader104.exe`，安装包会包含 `resources/ffmpeg/` 中的 FFmpeg 资源。
- 新增 NSIS hook，安装完成后重写桌面/开始菜单快捷方式图标和卸载项 `DisplayIcon`，避免继续显示旧白底默认图标。
- 新增 `tauri-plugin-log`，运行日志写入程序所在目录的 `logs/`；README 中同步记录应用数据、日志、打包产物和验收步骤。
- 前端运行依赖保持为 Tauri API、React、路由、动画和图标库；AI Studio 云端模板依赖未进入 `rust-bilibili/package.json`。
- 已补齐真实二维码登录、登录成功后弹窗关闭、侧栏真实头像、视频 UP 主头像、Q版 22/33 看板娘、安装/卸载图标和快捷方式图标修正。
- 验收项中“全新 Windows 环境可安装/启动”已经进入实机验收；后续仍需继续覆盖更多真实受限视频和异常网络场景。

## 13. 测试策略

### Rust 单元测试

- Cookie 解析：JSON、Cookie-Editor、Netscape。
- BVID 提取。
- DASH/DURL 响应解析。
- 文件名清理。
- 错误转换。

### Rust 集成测试

- 使用录制的 B站 API JSON fixture 测试 `api.rs`。
- 使用本地 HTTP 测试服务器模拟 Range 206、Range 被忽略 200、断线重试。
- 测试取消 token 是否能中断下载。

### 前端测试

- `types.ts` 与 bridge 返回值对齐。
- 下载队列在 progress/completed/failed/cancelled 事件下正确更新。
- 设置页读写 config。

### 手工验收

- 公开视频解析和下载。
- 多分P视频解析。
- 无 FFmpeg 下载。
- 安装 FFmpeg 后合并。
- Cookie 失效。
- 取消下载。

## 14. 当前已知风险

- B站接口可能变化，需要保留原始响应日志开关。
- 充电/付费/大会员视频的权限判断不能只依赖一个字段，必须以 playurl 实际结果为准。
- FFmpeg 自动下载源可能不可用，需要允许用户手动指定路径。
- Tauri 文件系统权限要收敛，不能给整个磁盘无边界权限。
- 前端现有 UI 偏视觉稿，真实状态和错误态需要补齐。

## 15. 完成定义

该重写计划完成时，应满足：

- Google AI Studio 生成的 React 前端被保留为主界面。
- 前端所有核心动作都接入真实 Rust/Tauri command。
- 下载进度、取消、完成、失败都由后端事件驱动。
- Rust 后端可以独立支撑视频解析、下载、合并、历史、配置、登录。
- 发布包不依赖 Python。
- 用户可以在一个应用内完成从登录到下载完成的完整流程。
