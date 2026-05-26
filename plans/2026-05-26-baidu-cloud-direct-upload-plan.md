# 百度网盘云端直传实施计划

日期：2026-05-26  
目标项目：`rust-bilibili`  
核心目标：下载 B站视频时，视频大数据不落本地硬盘，直接通过内存缓冲上传到百度网盘。  

## 0. 当前状态快照

更新时间：2026-05-26

- 百度网盘 OAuth 已改为 `oob` 手动授权码模式，使用开放平台应用详情中的 `AppKey` 作为 `client_id`、`Secretkey` 作为 `client_secret`。
- 设置页已支持保存百度网盘配置、打开授权页、粘贴授权码完成授权、退出授权、上传测试文件。
- 首页已接入“保存位置：本地 / 百度网盘”，未授权时会禁用云盘保存入口并给出中文提示。
- 百度 Raw DASH 直传链路已实现并经人工验证可上传，但输出为 `.m4s + .m4a`，不能作为单个 MP4 直接播放。
- 视频云盘保存已改为 MP4 管道直传：第一遍通过 FFmpeg stdout 生成 fragment MP4 并计算分片 MD5，第二遍重新生成同样字节流并上传百度网盘，不写本地大视频文件。
- MP4 管道直传已通过本地 FFmpeg 双遍稳定性自检；仍需用真实百度网盘视频任务验证云盘中出现 `.mp4` 且播放器可直接播放。
- 云盘上传会保存小型 session 状态；应用重启后重新发起同名云盘任务时，会复用匹配的上传会话并跳过已上传分片。
- 弹幕在云盘模式下仍以外挂 `.ass` 小文件上传；烧录弹幕后直传 MP4 暂不默认支持。

## 1. 目标与边界

### 1.1 必须实现

- [x] 新增统一云盘上传接口，后续可以扩展 123 云盘、坚果云等国内云盘。
- [x] 第一版只接百度网盘。
- [x] 新增“保存到百度网盘”下载模式。
- [x] 云盘模式下不生成本地 `.m4s`、`.m4a`、`.mp4` 大文件。
- [x] 允许写入小型本地状态文件，例如百度授权 token、云盘配置、上传任务状态、历史记录。
- [x] 支持取消任务、失败提示、上传进度展示。
- [x] 本地下载模式保持现有行为不变。

### 1.2 第一阶段不做

- [x] 不做外国云盘。
- [x] 不做百度网盘以外的云盘实现。
- [x] 不做“下载到本地后自动上传”的过渡模式。
- [x] 不承诺一开始就支持云盘内单文件 MP4。（已在 Raw DASH 稳定后补充实验管道）
- [x] 不承诺一开始就支持烧录弹幕后直传 MP4。

### 1.3 允许的本地写入

云端直传模式允许写入：

- `cloud_config.json`
- `baidu_token.json`
- `cloud_upload_sessions.json`
- `history.json`
- 日志文件

云端直传模式不允许写入：

- 大体积视频文件
- 大体积音频文件
- 大体积合并中间文件
- 大体积 FFmpeg 输出文件

## 2. 技术事实与风险

### 2.1 百度网盘开放平台约束

百度网盘上传链路通常包含：

- OAuth 授权，常见 scope 包含 `basic`、`netdisk`
- 预上传 `precreate`
- 分片上传 `superfile2`
- 创建文件 `create`
- 文件目录通常落在应用授权目录，例如 `/apps/<应用名>/`

重要风险：

- 百度网盘上传权限可能需要申请或审核。
- 开放平台接口、权限范围、目录限制可能会变化，最终以官方文档和实际申请权限为准。
- 预上传可能需要完整文件大小和分片 MD5 列表，因此真正“单遍边下边传”不一定可行。

参考资料：

- 百度 OAuth 授权说明：https://developer.baidu.com/question/detail.html?id=177
- 百度文件上传流程说明：https://developer.baidu.com/question/detail.html?id=179
- 百度上传权限说明：https://developer.baidu.com/article/details/293412

### 2.2 当前项目约束

当前下载管线：

- `DownloadClient` 以文件为目标下载，Range 分片会写 `.part` 临时文件。
- `run_download_task` 会先下载 DASH 视频流和音频流，再用 FFmpeg 合并成 MP4。
- 弹幕外挂会生成 ASS 文件；烧录弹幕会依赖 FFmpeg 写出新视频文件。
- 历史记录目前以本地路径为核心。

因此云端直传不能直接复用现有 `download_file` 主路径，必须新增独立流式管线。

## 3. 总体架构

### 3.1 新增模块

建议新增后端模块：

```text
rust-bilibili/src-tauri/src/cloud/
  mod.rs
  uploader.rs
  baidu.rs
  mock.rs
  session.rs

rust-bilibili/src-tauri/src/streaming/
  mod.rs
  bili_stream.rs
  md5_plan.rs
```

建议新增模型：

```text
rust-bilibili/src-tauri/src/models/cloud.rs
```

### 3.2 统一接口草案

```rust
#[async_trait]
pub trait CloudUploader: Send + Sync {
    async fn auth_status(&self) -> AppResult<CloudAuthStatus>;
    async fn precreate(&self, plan: CloudFilePlan) -> AppResult<CloudUploadSession>;
    async fn upload_part(
        &self,
        session: &CloudUploadSession,
        part_index: usize,
        bytes: bytes::Bytes,
    ) -> AppResult<CloudPartResult>;
    async fn finish(&self, session: CloudUploadSession) -> AppResult<CloudFileResult>;
}
```

核心数据结构：

```rust
pub enum CloudProvider {
    BaiduNetdisk,
}

pub enum CloudUploadMode {
    RawDash,
    Mp4PipeExperimental,
}

pub struct CloudFilePlan {
    pub provider: CloudProvider,
    pub remote_path: String,
    pub size_bytes: u64,
    pub part_size: u64,
    pub block_md5: Vec<String>,
}
```

### 3.3 云端直传两条路径

#### 路径 A：Raw DASH 直传，已实现的低风险路径

输出到百度网盘：

- `视频标题-video-<清晰度>.m4s`
- `视频标题-audio.m4a`
- 可选：`视频标题.ass`

当前状态：

- 已实现。
- 已经通过真实百度授权链路验证可上传。
- 当前作为音频云盘保存和必要时的低风险回退方案保留。

优点：

- 实现风险最低。
- 不写本地大文件。
- 不需要 FFmpeg 合并。
- 更接近“边拉流边上传”。

缺点：

- 云盘里不是单个 MP4。
- 播放器需要自行处理或用户后续下载后再合并。

#### 路径 B：MP4 管道直传，当前视频云盘默认路径

流程：

```text
B站视频流 + B站音频流 -> FFmpeg pipe -> 内存分片 -> 百度网盘
```

风险：

- 百度预上传若强制要求 size 和分片 MD5，需要先跑一次 FFmpeg 输出扫描，再跑第二次上传。
- FFmpeg stdout 的 MP4 需要适合流式输出的参数，例如 fragment MP4。
- 烧录弹幕会增加 CPU、内存和失败恢复复杂度。

当前状态：

- 已实现。
- 视频云盘保存默认走此路径，目标输出为单个 `.mp4`。
- 已通过本地 FFmpeg 双遍稳定性自检。
- 真实百度网盘中的 `.mp4` 上传和播放仍待人工验收。

因此 MP4 直传只在 Raw DASH 稳定后推进。当前已接入两遍 FFmpeg stdout 管道：第一遍计算分片 MD5，第二遍按相同参数上传 MP4 字节流，不写本地大视频文件。

## 4. 阶段拆解

## Phase 0：官方权限与可行性验证

目标：确认百度网盘开放平台权限能跑通，不写业务代码。

任务：

- [x] 注册或确认百度网盘开放平台应用。
- [x] 确认 OAuth 回调方式：`oob` 手动复制授权码。
- [x] 确认应用是否具备文件上传权限。
- [x] 确认应用目录路径规则。
- [x] 用应用内“上传测试文件”完成一个小文本文件上传。

验收：

- [x] 能拿到 `access_token`。
- [x] 能在百度网盘应用目录创建文件。
- [x] 记录实际可用接口、scope、目录限制和错误码。

阻塞条件：

- 无法获得上传权限。
- 应用审核未通过。
- 百度接口要求与公开资料不一致且无法绕过。

## Phase 1：云盘抽象层与 Mock Uploader

目标：先把云盘能力做成可测试接口，不绑定百度。

任务：

- [x] 新增 `models/cloud.rs`。
- [x] 新增 `cloud/uploader.rs` trait。
- [x] 新增 `cloud/mock.rs`，用于不联网的单元测试。
- [x] 新增云盘上传进度事件。
- [x] 新增云盘错误类型映射。
- [x] 新增测试：mock uploader 完整跑通 `precreate -> upload_part -> finish`。

验收：

```powershell
cd rust-bilibili/src-tauri
cargo test cloud
```

完成标准：

- [x] 不影响现有本地下载。
- [x] mock 分片上传测试通过。
- [x] 云盘接口没有直接依赖 UI。

## Phase 2：百度网盘授权与配置

目标：让应用能登录百度网盘，并持久化 token。

任务：

- [x] 新增 `cloud/baidu.rs`。
- [x] 新增 `BaiduTokenStore`，保存 `access_token`、`refresh_token`、过期时间。
- [x] 新增命令：
  - `baidu_auth_start`
  - `baidu_auth_finish`
  - `baidu_auth_status`
  - `baidu_logout`
- [x] 设置页新增“百度网盘”区域。
- [x] 支持打开系统浏览器授权。
- [x] 支持 `oob` 手动输入授权码。
- [x] token 过期时自动刷新或提示重新授权。

验收：

- [x] 设置页能显示百度网盘登录状态。
- [x] 重启应用后仍能读取授权状态。
- [x] 未登录时云盘下载按钮不可用或给出清晰提示。

命令：

```powershell
cd rust-bilibili
npm run lint
npm run build
cd src-tauri
cargo test
```

## Phase 3：B站流式读取器

目标：新增不落盘的 B站字节读取能力。

任务：

- [x] 新增 `streaming/bili_stream.rs`。
- [x] 支持 HEAD 获取大小。
- [x] 支持 Range 读取指定区间。
- [x] 支持普通流式读取。
- [x] 支持取消任务。
- [x] 支持进度回调。
- [x] 保留 B站 Referer、Cookie 请求头。

验收：

- [x] 能读取视频流前 4MB 到内存。
- [x] 能读取音频流前 4MB 到内存。
- [x] 取消任务后不会继续请求。
- [x] 不创建本地视频临时文件。

## Phase 4：分片 MD5 规划

目标：满足百度预上传可能需要的 block list。

任务：

- [x] 新增 `streaming/md5_plan.rs`。
- [x] 从 B站流按百度分片大小读取。
- [x] 只在内存里计算每个分片 MD5。
- [x] 保存小型上传计划到 `cloud_upload_sessions.json`。
- [x] 支持失败后重新计算或继续。

验收：

- [x] 对一个公开视频的视频流生成完整 block MD5。
- [x] 对音频流生成完整 block MD5。
- [x] 本地没有产生大体积临时文件。
- [x] 内存占用受控，单块缓冲不超过配置值。

风险说明：

如果百度 `precreate` 不强制要求完整 block MD5，则可以跳过预扫描，直接进入上传；计划中保留该阶段是为了兼容更严格的接口要求。

## Phase 5：百度 Raw DASH 直传

目标：实现第一条真正可用的“不落本地大文件”链路。当前已作为低风险路径实现；视频云盘默认路径已转向 Phase 8 的 MP4 管道直传。

任务：

- [x] 新增 `start_cloud_upload` 命令，参数复用当前下载参数并增加云盘字段。
- [x] 从 `playurl` 选择视频流和音频流。
- [x] 为视频流生成上传计划。
- [x] 上传视频 `.m4s` 到百度网盘。
- [x] 为音频流生成上传计划。
- [x] 上传音频 `.m4a` 到百度网盘。
- [x] 如果选择外挂弹幕，生成 ASS 文本并上传为小文件。
- [x] 写入历史记录，路径显示为百度网盘远程路径。

验收：

- [x] 输入公开视频链接，选择百度网盘保存。
- [x] 下载任务完成后，百度网盘应用目录出现 `.m4s` 和 `.m4a`。
- [x] 勾选外挂弹幕时出现 `.ass`。
- [x] 本地下载目录没有对应大视频文件。
- [x] 任务取消时百度网盘不会创建损坏完成文件，或能明确标记失败状态。

## Phase 6：前端云盘下载 UI

目标：让用户可以清晰选择“本地下载”或“百度网盘直传”。

任务：

- [x] 下载设置新增“保存位置”分段控件：
  - 本地
  - 百度网盘
- [x] 百度网盘模式下隐藏本地保存目录。
- [x] 百度网盘模式下显示远程目录。
- [x] 百度网盘未登录时显示登录引导。
- [x] 下载队列支持云盘阶段文案：
  - 解析中
  - 计算分片
  - 上传视频
  - 上传音频
  - 上传弹幕
  - 上传完成
- [x] 历史页支持云盘记录展示。

验收：

- [x] UI 全中文。
- [x] 本地模式原功能不变。
- [x] 百度网盘模式不会显示本地目录输入框。
- [x] 云盘任务进度可见。

## Phase 7：失败恢复与限速重试

目标：提升大文件上传稳定性。

任务：

- [x] 记录每个文件的上传 session。
- [x] 记录已完成分片。
- [x] 网络错误自动重试。
- [x] 百度限速或 429/5xx 错误退避重试。
- [x] 应用重启后能提示“存在未完成云盘任务”。
- [x] 应用重启后支持从已完成分片继续上传。

验收：

- [x] 断网后任务失败信息清晰。
- [x] 短暂网络失败可自动重试。
- [x] 用户取消后状态正确。
- [x] 重启后不会丢失小型任务状态。
- [x] 重启后可以继续未完成云盘任务。

## Phase 8：MP4 云端直传实验模式

目标：在 Raw DASH 稳定后，输出单个 MP4 到百度网盘。当前已接入为视频云盘保存默认路径，但真实百度网盘播放效果仍需人工验收。

任务：

- [x] 调研 FFmpeg stdout 输出 fragment MP4 参数。
- [x] 支持从 B站流输入 FFmpeg。
- [x] 第一遍扫描 FFmpeg 输出，计算总大小和 block MD5。
- [x] 第二遍重复输出并分片上传百度网盘。
- [x] UI 标记为 MP4 云端直传。
- [x] 烧录弹幕暂不默认开启。
- [x] 本地 FFmpeg stdout 双遍输出稳定性自检。

验收：

- [ ] 百度网盘中出现单个 `.mp4` 文件。
- [ ] 本地没有 `.m4s/.m4a/.mp4` 大文件。
- [ ] 播放器能正常播放下载后的 MP4。

退出条件：

如果 FFmpeg 管道输出无法稳定复现相同分片 MD5，或百度上传必须严格绑定预扫描结果，则 MP4 云端直传保持实验状态，不作为默认功能。

## 5. 数据与配置变更

新增配置：

```json
{
  "default_provider": "baidu_netdisk",
  "default_remote_dir": "/apps/B站充电视频下载器",
  "default_save_mode": "local",
  "part_size_mb": 4,
  "baidu": {
    "client_id": "<AppKey>",
    "client_secret": "<Secretkey>",
    "redirect_uri": "oob",
    "scope": "basic,netdisk"
  }
}
```

当前历史记录方式：

```json
{
  "format": "cloud_video | cloud_audio",
  "output_path": "/apps/B站充电视频下载器/xxx.mp4",
  "status": "completed"
}
```

后续如要做更完整的历史页展示，再补充 `storage`、`remote_path`、`cloud_provider` 等结构化字段。

## 6. 验证矩阵

每个阶段都必须跑：

```powershell
cd rust-bilibili
npm run lint
npm run build
cd src-tauri
cargo test
```

关键人工验收：

- [ ] 本地下载模式仍能下载 MP4。
- [x] 百度网盘未登录时提示清楚。
- [x] 百度网盘登录成功后可选择云盘保存。
- [x] 云盘模式不写大视频文件到下载目录。
- [x] 云盘里能看到上传结果。
- [x] 取消任务不会留下错误的完成历史。
- [ ] 云盘视频模式上传后，百度网盘中出现可直接播放的单个 `.mp4`。

## 7. 推荐推进顺序

严格串行：

1. Phase 0
2. Phase 1
3. Phase 2
4. Phase 3
5. Phase 4
6. Phase 5
7. Phase 6
8. Phase 7
9. Phase 8

可并行部分：

- Phase 2 的前端登录 UI 可以和 Phase 3 的 B站流式读取器并行。
- Phase 6 的视觉设计可以在 Phase 5 后端接口稳定后再并行补齐。
- Phase 8 必须等 Phase 5 稳定后再做。

## 8. 完成定义

第一版完成标准：

- [x] 用户可以登录百度网盘。
- [x] 用户可以选择保存到百度网盘。
- [x] B站视频流和音频流可通过 Raw DASH 方式上传到百度网盘。
- [x] 视频大数据不落本地硬盘。
- [x] 任务进度和失败原因可见。
- [x] 本地下载模式不受影响。

最终增强标准：

- [x] 支持单文件 MP4 云端直传实验模式。
- [x] 支持断点恢复。
- [ ] 支持更多国内云盘接入。
