#!/usr/bin/env python3
"""B站视频下载工具 - GUI 界面"""

import os
import re
import sys
import threading
import tkinter as tk
from tkinter import filedialog, ttk
from pathlib import Path

import ttkbootstrap as ttk
from ttkbootstrap.constants import *

# 确保 src 在导入路径中
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "src"))

# 崩溃日志写入文件（noconsole 模式下调试用）
_log_file = os.path.join(os.path.dirname(os.path.abspath(__file__)), "crash.log")
try:
    sys.stderr = open(_log_file, "w", encoding="utf-8")
except Exception:
    pass

from bilibili_api import BilibiliAPI, APIError
from downloader import VideoDownloader, DownloadError
from merger import FFmpegMerger


BVID_RE = re.compile(r"BV\w{10,}")


def _extract_bvid(text: str) -> str | None:
    """从 URL 或纯文本中提取 BVID（BV 号）"""
    if not text:
        return None
    m = BVID_RE.search(text.strip())
    return m.group(0) if m else None


def _sanitize_filename(name: str, max_len: int = 80) -> str:
    """将字符串转为安全的 Windows 文件名，过长的截断"""
    invalid = r'<>:"/\|?*'
    clean = "".join(c if c not in invalid and ord(c) >= 32 else "_" for c in name)
    clean = clean.strip(". ")
    if not clean:
        clean = "untitled"
    if len(clean) > max_len:
        clean = clean[:max_len].rstrip()
    return clean


QUALITY_MAP = {
    "360P": 16,
    "480P": 32,
    "720P": 64,
    "1080P": 80,
    "1080P+ 高码率": 80,
    "1080P60": 116,
    "4K": 120,
    "HDR": 125,
}

QUALITY_LABELS = list(QUALITY_MAP.keys())


class BilibiliDownloaderGUI:
    def __init__(self):
        self.root = ttk.Window(
            title="B站视频下载工具",
            themename="darkly",
            resizable=(True, True),
            size=(720, 620),
            minsize=(640, 520),
        )
        # state
        self.api: BilibiliAPI | None = None
        self._downloading = False

        self._build_ui()
        self._center_window()

    # ----------------------------------------------------------------
    # UI 构建
    # ----------------------------------------------------------------

    def _center_window(self):
        self.root.update_idletasks()
        w = self.root.winfo_width()
        h = self.root.winfo_height()
        sw = self.root.winfo_screenwidth()
        sh = self.root.winfo_screenheight()
        x = (sw - w) // 2
        y = (sh - h) // 2
        self.root.geometry(f"+{x}+{y}")

    def _build_ui(self):
        # 主容器
        main = ttk.Frame(self.root, padding=16)
        main.pack(fill=BOTH, expand=YES)

        # ---- 输入区域 ----
        input_frame = ttk.LabelFrame(main, text="视频信息")
        input_frame.pack(fill=X, pady=(0, 10))
        input_body = ttk.Frame(input_frame)
        input_body.pack(fill=BOTH, expand=YES, padx=12, pady=12)

        # BVID
        row1 = ttk.Frame(input_body)
        row1.pack(fill=X, pady=3)
        ttk.Label(row1, text="BVID:", width=10).pack(side=LEFT)
        self.bvid_var = tk.StringVar()
        ttk.Entry(row1, textvariable=self.bvid_var, font=("Consolas", 11)).pack(
            side=LEFT, fill=X, expand=YES, padx=(0, 6)
        )
        ttk.Button(row1, text="获取信息", command=self._fetch_info, bootstyle=INFO).pack(side=LEFT)

        # Cookie
        row2 = ttk.Frame(input_body)
        row2.pack(fill=X, pady=3)
        ttk.Label(row2, text="Cookie:", width=10).pack(side=LEFT)
        self.cookie_var = tk.StringVar()
        ttk.Entry(row2, textvariable=self.cookie_var).pack(
            side=LEFT, fill=X, expand=YES, padx=(0, 6)
        )
        ttk.Button(row2, text="浏览…", command=self._browse_cookie, width=8).pack(side=LEFT)
        ttk.Button(row2, text="扫码登录", command=self._qr_login, bootstyle=PRIMARY, width=10).pack(
            side=LEFT, padx=(6, 0))

        # 画质 + 输出目录
        row3 = ttk.Frame(input_body)
        row3.pack(fill=X, pady=3)
        ttk.Label(row3, text="画质:", width=10).pack(side=LEFT)
        self.quality_var = tk.StringVar(value="1080P")
        self.quality_cb = ttk.Combobox(
            row3,
            textvariable=self.quality_var,
            values=QUALITY_LABELS,
            state="readonly",
            width=16,
        )
        self.quality_cb.pack(side=LEFT, padx=(0, 20))

        ttk.Label(row3, text="输出目录:").pack(side=LEFT)
        self.outdir_var = tk.StringVar(value="downloads")
        ttk.Entry(row3, textvariable=self.outdir_var).pack(
            side=LEFT, fill=X, expand=YES, padx=(6, 6)
        )
        ttk.Button(row3, text="浏览…", command=self._browse_outdir, width=8).pack(side=LEFT)

        # 选项
        row4 = ttk.Frame(input_body)
        row4.pack(fill=X, pady=(6, 0))

        ttk.Label(row4, text="速度:").pack(side=LEFT)
        self.speed_var = tk.StringVar(value="标准 (8线程)")
        speed_cb = ttk.Combobox(
            row4, textvariable=self.speed_var,
            values=["慢速 (4线程)", "标准 (8线程)", "快速 (16线程)", "极速 (32线程)"],
            state="readonly", width=16,
        )
        speed_cb.pack(side=LEFT, padx=(4, 20))

        self.skip_merge_var = tk.BooleanVar(value=False)
        ttk.Checkbutton(row4, text="不合并音视频（保留原始文件）", variable=self.skip_merge_var).pack(
            side=LEFT, padx=(10, 20)
        )

        # ---- 视频信息展示（固定高度，内容过多可滚动） ----
        self.info_frame = ttk.LabelFrame(main, text="视频信息")
        self.info_frame.pack(fill=X, pady=(0, 10))
        info_body = ttk.Frame(self.info_frame)
        info_body.pack(fill=BOTH, expand=YES, padx=10, pady=10)
        self.info_text = tk.Text(
            info_body,
            height=4,
            wrap=WORD,
            font=("Microsoft YaHei UI", 9),
            state=tk.DISABLED,
            relief=tk.FLAT,
            borderwidth=0,
        )
        self.info_text.pack(side=LEFT, fill=BOTH, expand=YES)
        self.info_text_scroll = ttk.Scrollbar(info_body, orient=VERTICAL, command=self.info_text.yview)
        self.info_text_scroll.pack(side=RIGHT, fill=Y)
        self.info_text.configure(yscrollcommand=self.info_text_scroll.set)
        self._set_info_text("点击「获取信息」查看视频详情")

        # ---- 日志 ----
        log_frame = ttk.LabelFrame(main, text="日志")
        log_frame.pack(fill=BOTH, expand=YES, pady=(0, 8))
        log_body = ttk.Frame(log_frame)
        log_body.pack(fill=BOTH, expand=YES, padx=6, pady=6)

        self.log_text = tk.Text(
            log_body,
            height=12,
            wrap=WORD,
            font=("Consolas", 10),
            state=tk.DISABLED,
            bg="#1e1e1e",
            fg="#d4d4d4",
            relief=tk.FLAT,
            borderwidth=0,
        )
        self.log_text.pack(side=LEFT, fill=BOTH, expand=YES, padx=(0, 4))

        log_scroll = ttk.Scrollbar(log_body, orient=VERTICAL, command=self.log_text.yview)
        log_scroll.pack(side=RIGHT, fill=Y)
        self.log_text.configure(yscrollcommand=log_scroll.set)

        # ---- 进度条 ----
        progress_frame = ttk.Frame(main)
        progress_frame.pack(fill=X)
        self.progress = ttk.Progressbar(
            progress_frame, mode="determinate", value=0, bootstyle=SUCCESS
        )
        self.progress.pack(side=LEFT, fill=X, expand=YES, padx=(0, 8))
        self.progress_label = ttk.Label(progress_frame, text="就绪", width=12, anchor=E)
        self.progress_label.pack(side=LEFT)

        # ---- 底部按钮 ----
        btn_frame = ttk.Frame(main)
        btn_frame.pack(fill=X, pady=(6, 0))

        ttk.Button(
            btn_frame,
            text="帮助",
            command=self._show_help,
            bootstyle=INFO,
            width=10,
        ).pack(side=LEFT)

        ttk.Button(
            btn_frame,
            text="安装 FFmpeg",
            command=self._install_ffmpeg,
            bootstyle=INFO,
            width=16,
        ).pack(side=LEFT, padx=(6, 0))

        ttk.Button(
            btn_frame,
            text="清空日志",
            command=self._clear_log,
            bootstyle=SECONDARY,
        ).pack(side=LEFT, padx=(30, 0))

        ttk.Button(
            btn_frame,
            text="浏览器捕获下载",
            command=self._start_browser_capture,
            bootstyle=WARNING,
            width=20,
        ).pack(side=RIGHT, padx=(8, 0))

        ttk.Button(
            btn_frame,
            text="普通下载",
            command=self._start_download,
            bootstyle=SUCCESS,
            width=16,
        ).pack(side=RIGHT, padx=(8, 0))

    # ----------------------------------------------------------------
    # 日志 & 进度
    # ----------------------------------------------------------------

    def _log(self, msg: str):
        self.log_text.configure(state=tk.NORMAL)
        self.log_text.insert(tk.END, msg + "\n")
        self.log_text.see(tk.END)
        self.log_text.configure(state=tk.DISABLED)
        self.root.update_idletasks()

    def _set_progress(self, value: int, text: str = None):
        self.progress["value"] = value
        if text:
            self.progress_label.configure(text=text)
        self.root.update_idletasks()

    def _set_info_text(self, text: str):
        self.info_text.configure(state=tk.NORMAL)
        self.info_text.delete("1.0", tk.END)
        self.info_text.insert("1.0", text)
        self.info_text.configure(state=tk.DISABLED)

    def _clear_log(self):
        self.log_text.configure(state=tk.NORMAL)
        self.log_text.delete("1.0", tk.END)
        self.log_text.configure(state=tk.DISABLED)

    def _show_help(self):
        """显示帮助对话框"""
        help_text = """B站视频下载工具 - 使用帮助

━━━ 基本使用 ━━━
1. 输入 BVID（BV号）或粘贴完整视频链接
2. 选择 Cookie 文件（从浏览器导出登录信息）
3. 选择画质和输出目录
4. 点击「普通下载」开始下载

━━━ Cookie 获取 ━━━
• 点击「扫码登录」按钮 → 浏览器扫码 → 自动保存
• 或用 Cookie-Editor 扩展导出 JSON 后选择文件

━━━ 两种下载方式 ━━━
• 普通下载：通过 B站 API 直接下载（需有效 Cookie）
• 浏览器捕获：打开真实浏览器捕获视频流地址
  （可用于绕过部分反爬限制，需安装 Chrome）

━━━ 文件说明 ━━━
下载的原始文件为 .m4s 格式（视频/音频分离），
安装 FFmpeg 后会自动合并为单个 .mp4 文件。
点击「安装 FFmpeg」按钮自动下载配置。

━━━ 画质说明 ━━━
360P→16  480P→32  720P→64  1080P→80
1080P+→80  1080P60→116  4K→120  HDR→125
高画质需要大会员权限。"""

        from tkinter import messagebox
        messagebox.showinfo("帮助", help_text)

    def _install_ffmpeg(self):
        """后台下载 FFmpeg"""
        self._log("[*] 正在下载 FFmpeg（约 10MB，请稍候）...")
        self._set_progress(0, "下载 FFmpeg...")

        def task():
            try:
                merger = FFmpegMerger()
                path = merger.auto_download(progress_callback=lambda p, t: (
                    self._set_progress(p, t) if p < 100 else None
                ))
                self._set_progress(100, "FFmpeg 就绪")
                self._log(f"[OK] FFmpeg 已下载: {path}")
                self._log("[OK] 下次下载会自动合并成 .mp4 文件")
            except Exception as e:
                self._log(f"[!] 下载 FFmpeg 失败: {e}")
                self._log("[!] 手动下载: https://ffmpeg.org/download.html")
                self._set_progress(0, "下载失败")

        threading.Thread(target=task, daemon=True).start()

    # ----------------------------------------------------------------
    # 文件选择
    # ----------------------------------------------------------------

    def _browse_cookie(self):
        path = filedialog.askopenfilename(
            title="选择 Cookie 文件",
            filetypes=[
                ("Cookie 文件", "*.txt *.json"),
                ("所有文件", "*.*"),
            ],
        )
        if path:
            self.cookie_var.set(path)

    def _browse_outdir(self):
        path = filedialog.askdirectory(title="选择输出目录")
        if path:
            self.outdir_var.set(path)

    def _qr_login(self):
        """扫码登录 B站 — 打开浏览器让用户扫码，自动保存 Cookie"""
        import subprocess, tempfile, json as jmod

        self._log("[*] 正在启动扫码登录...")
        self._log("[*] 浏览器窗口会打开，请用 B站 App 扫码登录")
        self._set_progress(10, "启动登录浏览器...")

        python_exe = None
        for candidate in ["python", "python3", "py"]:
            try:
                r = subprocess.run([candidate, "--version"], capture_output=True, timeout=5, text=True)
                if "Python" in r.stdout or "Python" in r.stderr:
                    python_exe = candidate
                    break
            except Exception:
                continue

        if not python_exe:
            self._log("[!] 找不到系统 Python，无法启动登录")
            return

        exe_dir = os.path.dirname(os.path.abspath(sys.executable if getattr(sys, "frozen", False) else __file__))
        login_script = os.path.join(exe_dir, "login_capture.py")
        meipass = getattr(sys, "_MEIPASS", None)
        if not os.path.exists(login_script) and meipass:
            login_script = os.path.join(meipass, "login_capture.py")
        if not os.path.exists(login_script):
            login_script = os.path.join(os.path.dirname(exe_dir), "login_capture.py")

        if not os.path.exists(login_script):
            self._log("[!] 找不到登录脚本 (login_capture.py)")
            return

        result_file = os.path.join(tempfile.gettempdir(), "bilibili_login_cookies.json")

        def task():
            try:
                cmd = [python_exe, login_script, result_file]
                proc = subprocess.Popen(
                    cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                    creationflags=subprocess.CREATE_NO_WINDOW if hasattr(subprocess, "CREATE_NO_WINDOW") else 0,
                )
                enc = "gbk" if sys.platform == "win32" else "utf-8"
                for raw_line in iter(proc.stdout.readline, b""):
                    try:
                        line = raw_line.decode(enc, errors="replace").strip()
                    except Exception:
                        line = raw_line.decode("utf-8", errors="replace").strip()
                    if line:
                        self._log(f"  {line}")
                proc.wait()

                if proc.returncode == 0 and os.path.exists(result_file):
                    with open(result_file, encoding="utf-8") as f:
                        data = jmod.load(f)
                    if data.get("SESSDATA"):
                        # 保存 cookie 文件到程序目录
                        cookie_dir = Path.home() / ".bilibili_downloader"
                        cookie_dir.mkdir(parents=True, exist_ok=True)
                        cookie_path = cookie_dir / "cookies.json"
                        with open(cookie_path, "w", encoding="utf-8") as f:
                            jmod.dump(data, f, ensure_ascii=False)
                        self.cookie_var.set(str(cookie_path))
                        self._log(f"[OK] 登录成功! Cookie 已保存: {cookie_path}")
                        self._log(f"[OK] 账号: 已登录 (SESSDATA 长度={len(data['SESSDATA'])})")
                        self._set_progress(100, "登录成功")
                    else:
                        self._log("[!] 未能获取到完整 Cookie，请重试")
                        self._set_progress(0, "登录失败")
                else:
                    self._log("[!] 登录失败或超时")
                    self._set_progress(0, "登录失败")
            except Exception as e:
                self._log(f"[!] 登录过程出错: {e}")
                self._set_progress(0, "登录失败")

        threading.Thread(target=task, daemon=True).start()

    # ----------------------------------------------------------------
    # 获取视频信息
    # ----------------------------------------------------------------

    def _parse_cookies(self) -> dict:
        path = self.cookie_var.get().strip()
        if not path or not os.path.exists(path):
            return {}
        cookies = {}
        content = open(path, encoding="utf-8").read().strip()

        # JSON 格式: 对象 {"SESSDATA": "..."} 或 数组 [{name, value}, ...]
        if content.startswith("{") or content.startswith("["):
            import json
            data = json.loads(content)

            if isinstance(data, dict):
                root = data.get("cookies", data)
                for key in ("SESSDATA", "bili_jct", "DedeUserID"):
                    cookies[key] = root.get(key, data.get(key, ""))

            elif isinstance(data, list):
                # Cookie-Editor 或类似扩展的导出格式: [{name, value, domain, ...}, ...]
                for item in data:
                    name = item.get("name", "")
                    value = item.get("value", "")
                    if name in ("SESSDATA", "bili_jct", "DedeUserID"):
                        cookies[name] = value

        # Netscape / 文本格式
        else:
            for line in content.splitlines():
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                # 尝试 TAB 分隔 (Netscape)
                parts = line.split("\t")
                if len(parts) >= 7:
                    name, value = parts[5], parts[6]
                elif len(parts) >= 2:
                    # 也可能是 KEY=VALUE 格式但被 TAB 分开
                    if "=" in parts[0]:
                        n, v = parts[0].split("=", 1)
                        name, value = n.strip(), v.strip()
                    else:
                        name, value = parts[0], parts[1]
                elif "=" in line:
                    name, value = line.split("=", 1)
                    name, value = name.strip(), value.strip()
                else:
                    continue
                if name in ("SESSDATA", "bili_jct", "DedeUserID"):
                    cookies[name] = value

        found = [k for k, v in cookies.items() if v]
        if found:
            self._log(f"[*] 从 Cookie 文件中解析到: {', '.join(found)}")
        else:
            self._log("[!] Cookie 文件中未找到 SESSDATA/bili_jct/DedeUserID")
        return cookies

    def _fetch_info(self):
        raw = self.bvid_var.get().strip()
        bvid = _extract_bvid(raw)
        if not bvid:
            self._log("[!] 未找到有效的 BVID（BV 号）")
            self._log("    请输入类似 BV1oNPCzkE8Q 或粘贴完整视频链接")
            return
        self.bvid_var.set(bvid)

        def task():
            self._log(f"[*] 正在获取 {bvid} 的信息...")
            try:
                cookies = self._parse_cookies()
                has_cookie = bool(cookies.get("SESSDATA"))
                if has_cookie:
                    self._log(f"[*] 已加载 Cookie (SESSDATA 长度={len(cookies['SESSDATA'])})")
                else:
                    self._log("[*] 未加载 Cookie，仅能获取公开信息")
                self.api = BilibiliAPI(
                    sessdata=cookies.get("SESSDATA", ""),
                    bili_jct=cookies.get("bili_jct", ""),
                    dedeuserid=cookies.get("DedeUserID", ""),
                )

                # 验证 Cookie 有效性
                if has_cookie:
                    try:
                        login_info = self.api.check_login()
                        if login_info["is_login"]:
                            self._log(
                                f"[OK] Cookie 有效: {login_info['username']} "
                                f"(Lv.{login_info['level']})"
                            )
                        else:
                            self._log(
                                f"[!] Cookie 无效或已过期! (API返回: {login_info['error_msg']})"
                            )
                            self._log("[!] 请重新导出 Cookie，充电视频下载会受限")
                    except Exception as e:
                        self._log(f"[!] Cookie 验证失败: {e}")

                info = self.api.get_video_info(bvid)
                title = info.get("title", "N/A")
                desc = info.get("desc", "")[:200]
                duration = info.get("duration", 0)
                pages = info.get("pages", [])
                rights = info.get("rights", {})

                lines = [f"标题: {title}", f"时长: {duration}s  |  分P: {len(pages)}"]

                # 付费状态检测
                vip_type = login_info.get("vip_type", 0) if has_cookie else 0
                is_charging = rights.get("elec_high", 0) == 1
                is_pay = info.get("elec", 0) == 1
                if is_charging or is_pay:
                    if vip_type == 0:
                        lines.append("⚠ 充电付费视频 — 当前账号无大会员，仅能获取15秒预览")
                        lines.append("  需要开通大会员或对该UP主充电后才能下载完整版")
                    else:
                        lines.append("ℹ 大会员专属视频（你的账号已具备访问权限）")

                # 检查实际 API 返回的访问状态
                if pages:
                    cid = pages[0]["cid"]
                    access = self.api.check_video_access(bvid, cid)
                    self._log(f"  访问检测: quality={access.get('quality')} "
                              f"dash={access.get('has_dash')} "
                              f"durl时长={access.get('durl_total_ms', 0)}ms")
                    if access.get("durl_total_ms", 0) < 60000 and (is_charging or is_pay):
                        self._log("  [!] 确认: 仅返回预览片段，需要付费购买完整版")

                if desc:
                    lines.append(f"简介: {desc}")

                self._set_info_text("\n".join(lines))
                self._log(f"[OK] 视频信息获取成功: {title}")
            except APIError as e:
                self._log(f"[!] API 错误: {e}")
                if "412" in str(e):
                    self._log("[!] 被风控拦截，请检查 Cookie 是否有效")
            except Exception as e:
                self._log(f"[!] 错误: {e}")

        threading.Thread(target=task, daemon=True).start()

    # ----------------------------------------------------------------
    # 下载
    # ----------------------------------------------------------------

    def _start_download(self):
        if self._downloading:
            self._log("[!] 正在下载中，请等待当前任务完成")
            return

        raw = self.bvid_var.get().strip()
        bvid = _extract_bvid(raw)
        if not bvid:
            self._log("[!] 未找到有效的 BVID（BV 号）")
            return
        self.bvid_var.set(bvid)

        qn = QUALITY_MAP.get(self.quality_var.get(), 80)
        outdir = self.outdir_var.get().strip() or "downloads"
        skip_merge = self.skip_merge_var.get()
        if not self.api:
            cookies = self._parse_cookies()
            self.api = BilibiliAPI(
                sessdata=cookies.get("SESSDATA", ""),
                bili_jct=cookies.get("bili_jct", ""),
                dedeuserid=cookies.get("DedeUserID", ""),
            )

        speed_map = {"慢速 (4线程)": 4, "标准 (8线程)": 8, "快速 (16线程)": 16, "极速 (32线程)": 32}
        self._workers = speed_map.get(self.speed_var.get(), 8)

        self._downloading = True
        self._set_progress(0, "准备中...")
        threading.Thread(
            target=self._download_task,
            args=(bvid, outdir, qn, skip_merge),
            daemon=True,
        ).start()

    def _start_browser_capture(self):
        """方案2: 使用浏览器自动化捕获视频流地址"""
        if self._downloading:
            self._log("[!] 正在下载中，请等待当前任务完成")
            return

        raw = self.bvid_var.get().strip()
        bvid = _extract_bvid(raw)
        if not bvid:
            self._log("[!] 未找到有效的 BVID（BV 号）")
            return
        self.bvid_var.set(bvid)

        cookie_path = self.cookie_var.get().strip()
        outdir = self.outdir_var.get().strip() or "downloads"
        skip_merge = self.skip_merge_var.get()

        self._downloading = True
        self._set_progress(0, "启动浏览器...")
        threading.Thread(
            target=self._browser_capture_task,
            args=(bvid, cookie_path, outdir, skip_merge),
            daemon=True,
        ).start()

    def _browser_capture_task(self, bvid: str, cookie_path: str, outdir: str, skip_merge: bool):
        """浏览器捕获后台任务 — 调用外部系统 Python 脚本"""
        import subprocess
        import tempfile
        import json as jmod

        try:
            self._log(f"\n[*] 方案2: 浏览器自动化捕获")
            self._log("[*] 将打开浏览器窗口，等待视频加载完成后自动关闭")

            # 找到系统 Python（不能是 exe 自身）
            python_exe = None
            for candidate in ["python", "python3", "py"]:
                try:
                    r = subprocess.run([candidate, "--version"], capture_output=True, timeout=5, text=True)
                    if "Python" in r.stdout or "Python" in r.stderr:
                        # 确认不是指向自己的 exe
                        where = subprocess.run(["where", candidate], capture_output=True, text=True, timeout=5)
                        exe_path = where.stdout.strip().split("\n")[0].strip() if where.stdout else ""
                        if exe_path and "BilibiliDownloader" not in exe_path:
                            python_exe = candidate
                            break
                except Exception:
                    continue

            if not python_exe:
                self._log("[!] 找不到系统 Python（需要安装 Python 才能使用浏览器捕获功能）")
                self._log("[!] 也可以直接运行: python capture_playurl.py BV号 cookie文件.json")
                return

            # 找到 capturer 脚本（在 exe 同目录）
            exe_dir = os.path.dirname(os.path.abspath(sys.executable if getattr(sys, 'frozen', False) else __file__))
            capturer_script = os.path.join(exe_dir, "capture_playurl.py")
            if not os.path.exists(capturer_script):
                # 也可能在 exe 的 _MEIPASS 临时目录
                meipass = getattr(sys, '_MEIPASS', None)
                if meipass:
                    capturer_script = os.path.join(meipass, "capture_playurl.py")

            if not os.path.exists(capturer_script):
                self._log(f"[!] 找不到捕获脚本 (capture_playurl.py)")
                self._log(f"    尝试: python capture_playurl.py {bvid} {cookie_path or ''}")
                return

            result_file = os.path.join(tempfile.gettempdir(), f"bili_capture_{bvid}.json")

            self._set_progress(10, "启动浏览器...")
            self._log(f"[*] 启动浏览器...")

            cmd = [python_exe, capturer_script, bvid, cookie_path or "", result_file]
            proc = subprocess.Popen(
                cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                creationflags=subprocess.CREATE_NO_WINDOW if hasattr(subprocess, "CREATE_NO_WINDOW") else 0,
            )

            # 实时输出捕获脚本的日志（兼容 Windows GBK 编码）
            enc = "gbk" if sys.platform == "win32" else "utf-8"
            for raw_line in iter(proc.stdout.readline, b""):
                try:
                    line = raw_line.decode(enc, errors="replace").strip()
                except Exception:
                    line = raw_line.decode("utf-8", errors="replace").strip()
                if line:
                    self._log(f"  {line}")
            proc.wait()

            if proc.returncode != 0:
                self._log("[!] 浏览器捕获失败，请确保已安装 Chrome 或运行过:")
                self._log("    python -m playwright install chromium")
                self._set_progress(0, "失败")
                return

            # 读取结果
            with open(result_file, encoding="utf-8") as f:
                data = jmod.load(f)

            if not data:
                self._log("[!] 未能捕获到播放地址（可能是Cookie过期或需要付费）")
                self._set_progress(0, "失败")
                return

            self._log(f"[OK] 捕获成功: quality={data.get('quality')}, "
                      f"dash={'dash' in data}")
            self._set_progress(30, "解析流地址...")

            # 获取视频信息
            info = self.api.get_video_info(bvid) if self.api else None
            title = info.get("title", bvid) if info else bvid
            safe_title = "".join(c if c.isalnum() or c in (" ", "_", "-") else "_" for c in title)
            video_dir = os.path.join(outdir, f"{bvid}_{safe_title}")
            os.makedirs(video_dir, exist_ok=True)

            # 解析流
            temp_api = BilibiliAPI()
            dash = temp_api.extract_dash_urls(data)
            durl = temp_api.extract_durl_urls(data)

            merger = FFmpegMerger()
            can_merge = merger.check_available() and not skip_merge

            if dash["video"] and dash["audio"]:
                best_video = max(dash["video"], key=lambda v: v.get("bandwidth", 0))
                best_audio = max(dash["audio"], key=lambda v: v.get("bandwidth", 0))
                seg_count = len(best_video.get("segment_urls", []))
                self._log(f"  DASH: {best_video['width']}x{best_video['height']} [{seg_count} 分片]")
                self._set_progress(40, f"下载 {seg_count} 分片...")

                downloader = VideoDownloader(max_workers=4)
                bc_part = _sanitize_filename(info.get("pages", [{}])[0].get("part", "P1") if info.get("pages") else "P1", 60)
                bc_prefix = f"01_{bc_part}"
                video_out = os.path.join(video_dir, f"{bc_prefix}_video.m4s")
                audio_out = os.path.join(video_dir, f"{bc_prefix}_audio.m4s")

                if seg_count > 1:
                    self._download_track_segments(downloader, best_video, video_out)
                    self._download_track_segments(downloader, best_audio, audio_out)
                else:
                    downloader.download(best_video["base_url"], video_out)
                    downloader.download(best_audio["base_url"], audio_out)

                self._set_progress(85, "合并...")
                if can_merge:
                    output_file = os.path.join(video_dir, f"{bc_prefix}.mp4")
                    merger.merge_video_audio(video_out, audio_out, output_file)
                    os.remove(video_out)
                    os.remove(audio_out)
                    self._log(f"[OK] {output_file}")
                else:
                    self._log(f"[OK] 视频: {video_out}")
                    self._log(f"[OK] 音频: {audio_out}")

            elif durl:
                self._log(f"  DURL: {len(durl)} 分片")
                output_file = os.path.join(video_dir, f"{bc_prefix}.flv")
                downloader = VideoDownloader(max_workers=4)
                downloader.download_with_concat(durl, output_file)
                self._log(f"[OK] {output_file}")
            else:
                self._log("[!] 未找到可播放的流")

            self._set_progress(100, "完成!")
            self._log(f"\n[✔] 文件保存在: {video_dir}")

        except Exception as e:
            self._log(f"[!] 浏览器捕获失败: {e}")
            import traceback
            self._log(traceback.format_exc())
        finally:
            self._downloading = False

    def _progress_cb_factory(self):
        """返回进度回调闭包"""
        current = {"last_pct": 0}

        def cb(downloaded: int, total: int):
            if total > 0:
                pct = int(downloaded / total * 100)
                if pct != current["last_pct"]:
                    current["last_pct"] = pct
                    self._set_progress(pct, f"{pct}%")

        return cb

    def _download_track_segments(self, downloader: VideoDownloader, track: dict, output_path: str):
        """下载 track 的所有 segment 并合并为单个文件"""
        urls = track.get("segment_urls", [])
        if not urls:
            self._log("    [!] 没有可用的分片 URL")
            return

        if len(urls) == 1:
            downloader.download(urls[0], output_path)
            return

        # 多分片: 每个分片单独下载后合并
        self._log(f"    [{len(urls)} 个分片，正在并发下载...]")
        seg_dir = output_path + ".parts"
        os.makedirs(seg_dir, exist_ok=True)

        seg_paths = []
        for idx, url in enumerate(urls):
            seg_file = os.path.join(seg_dir, f"seg_{idx:04d}.m4s")
            try:
                downloader.download(url, seg_file)
                seg_paths.append((idx, seg_file))
            except Exception as e:
                self._log(f"    [WARN] 分片 {idx} 下载失败: {e}")

        if not seg_paths:
            raise DownloadError("所有分片下载失败")

        seg_paths.sort(key=lambda x: x[0])
        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as out:
            for _, sp in seg_paths:
                with open(sp, "rb") as f:
                    out.write(f.read())

        # 清理临时分片目录
        import shutil
        shutil.rmtree(seg_dir, ignore_errors=True)
        self._log(f"    [OK] 合并完成 ({len(seg_paths)} 个分片)")

    def _download_task(self, bvid: str, outdir: str, qn: int, skip_merge: bool):
        try:
            self._log(f"[*] 开始下载 {bvid} (画质: {self.quality_var.get()})")
            self._log(f"[*] 输出目录: {outdir}")

            # 验证 Cookie 状态
            try:
                login_info = self.api.check_login()
                if login_info["is_login"]:
                    self._log(
                        f"[OK] Cookie 状态: {login_info['username']} "
                        f"(Lv.{login_info['level']}, VIP={login_info['vip_type']})"
                    )
                else:
                    self._log(f"[!] Cookie 无效或已过期: {login_info['error_msg']}")
                    self._log("[!] 充电视频只能下载到 15 秒预览版!")
            except Exception as e:
                self._log(f"[!] Cookie 验证失败: {e}")

            info = self.api.get_video_info(bvid)
            title = info.get("title", bvid)
            pages = info.get("pages", [])
            safe_title = "".join(c if c.isalnum() or c in (" ", "_", "-") else "_" for c in title)
            video_dir = os.path.join(outdir, f"{bvid}_{safe_title}")
            os.makedirs(video_dir, exist_ok=True)
            self._log(f"[*] 共 {len(pages)} 个分P")

            merger = FFmpegMerger()
            if not skip_merge:
                if merger.check_available():
                    self._log("[*] FFmpeg 可用，下载后将自动合并")
                else:
                    self._log("[*] FFmpeg 未安装，将保留原始文件")
                    skip_merge = True

            total_pages = len(pages)
            w = getattr(self, "_workers", 8)
            self._log(f"[*] 下载线程: {w} (可调: 速度下拉框)")

            # 并行处理分P
            page_workers = min(w, total_pages)
            page_lock = threading.Lock()
            completed = [0]  # mutable for closure

            def process_page(page: dict):
                cid = page["cid"]
                part_title = page.get("part", f"P{page['page']}")
                part_prefix = f"{page['page']:02d}_{_sanitize_filename(part_title, 60)}"

                with page_lock:
                    self._log(f"\n[P{page['page']}] 开始: {part_title}")

                try:
                    playurl_data = self.api.get_playurl(bvid, cid, qn=qn)
                    with page_lock:
                        self._log(f"[P{page['page']}] quality={playurl_data.get('quality')}, "
                                  f"dash={'dash' in playurl_data}")
                    dash = self.api.extract_dash_urls(playurl_data)
                    durl = self.api.extract_durl_urls(playurl_data)

                    dl = VideoDownloader(max_workers=max(2, w // 4))

                    if dash["video"] and dash["audio"]:
                        best_video = max(dash["video"], key=lambda v: v.get("bandwidth", 0))
                        best_audio = max(dash["audio"], key=lambda v: v.get("bandwidth", 0))
                        seg_count = len(best_video.get("segment_urls", []))

                        video_out = os.path.join(video_dir, f"{part_prefix}_video.m4s")
                        audio_out = os.path.join(video_dir, f"{part_prefix}_audio.m4s")

                        if seg_count > 1:
                            self._download_track_segments(dl, best_video, video_out)
                            self._download_track_segments(dl, best_audio, audio_out)
                        else:
                            dl.download(best_video["base_url"], video_out)
                            dl.download(best_audio["base_url"], audio_out)

                        if not skip_merge:
                            output_file = os.path.join(video_dir, f"{part_prefix}.mp4")
                            with page_lock:
                                self._log(f"  [P{page['page']}] 正在合并...")
                            merger.merge_video_audio(video_out, audio_out, output_file)
                            os.remove(video_out)
                            os.remove(audio_out)
                            with page_lock:
                                self._log(f"  [OK] {output_file}")
                        else:
                            with page_lock:
                                self._log(f"  [OK] 视频: {video_out}")
                                self._log(f"  [OK] 音频: {audio_out}")

                    elif durl:
                        output_file = os.path.join(video_dir, f"{part_prefix}.flv")
                        dl.download_with_concat(durl, output_file)
                        with page_lock:
                            self._log(f"  [OK] {output_file}")
                    else:
                        with page_lock:
                            self._log(f"  [P{page['page']}] [!] 无可用播放地址")

                except Exception as e:
                    with page_lock:
                        self._log(f"  [P{page['page']}] [!] 处理失败: {e}")

                with page_lock:
                    completed[0] += 1
                    pct = int(completed[0] / total_pages * 95)
                    self._set_progress(pct, f"{completed[0]}/{total_pages}")

            # 并行处理分P
            from concurrent.futures import ThreadPoolExecutor, as_completed
            with ThreadPoolExecutor(max_workers=page_workers) as pool:
                fut = [pool.submit(process_page, p) for p in pages]
                for f in as_completed(fut):
                    f.result()

            self._set_progress(100, "完成!")
            self._log(f"\n[✔] 全部完成! 文件保存在: {video_dir}")

        except APIError as e:
            self._log(f"[!] API 错误: {e}")
            if "412" in str(e):
                self._log("[!] 被风控拦截，请检查 Cookie 或稍后重试")
        except Exception as e:
            self._log(f"[!] 下载失败: {e}")
        finally:
            self._downloading = False


def main():
    app = BilibiliDownloaderGUI()
    app.root.mainloop()


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        import traceback
        with open(_log_file, "a", encoding="utf-8") as f:
            f.write(f"FATAL: {e}\n{traceback.format_exc()}\n")
        raise
