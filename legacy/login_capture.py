#!/usr/bin/env python3
"""扫码登录 B站 — 打开浏览器扫码，自动保存 Cookie"""

import json
import os
import sys
import time

result_file = sys.argv[1] if len(sys.argv) > 1 else ""
if not result_file:
    print("Usage: login_capture.py result_file.json")
    sys.exit(1)

from playwright.sync_api import sync_playwright, TimeoutError


def find_chrome():
    candidates = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        os.path.expanduser(r"~\AppData\Local\Google\Chrome\Application\chrome.exe"),
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        os.path.expanduser(r"~\AppData\Local\Microsoft\Edge\Application\msedge.exe"),
    ]
    for p in candidates:
        if os.path.isfile(p):
            return p
    return None


print(f"[login] 启动浏览器...", flush=True)
chrome_path = find_chrome()

with sync_playwright() as p:
    launch_kwargs = {"headless": False, "args": ["--disable-blink-features=AutomationControlled"]}
    if chrome_path:
        launch_kwargs["executable_path"] = chrome_path
        print(f"[login] 使用系统 Chrome", flush=True)

    browser = p.chromium.launch(**launch_kwargs)
    context = browser.new_context(viewport={"width": 1280, "height": 800})
    page = context.new_page()

    print(f"[login] 打开 https://passport.bilibili.com/login ...", flush=True)
    print(f"[login] 请在浏览器中扫码登录...", flush=True)
    page.goto("https://passport.bilibili.com/login", wait_until="domcontentloaded")

    # 等待用户登录（检测 URL 变化或 cookie 出现）
    logged_in = False
    for _ in range(120):  # 最多等 2 分钟
        try:
            cookies = context.cookies()
            for c in cookies:
                if c["name"] == "SESSDATA" and c["value"]:
                    logged_in = True
                    break
            if logged_in:
                break
        except Exception:
            pass
        time.sleep(1)

    if logged_in:
        print(f"[login] 登录成功!", flush=True)
        # 提取需要的 cookie
        all_cookies = context.cookies()
        sessdata = next((c["value"] for c in all_cookies if c["name"] == "SESSDATA"), "")
        bili_jct = next((c["value"] for c in all_cookies if c["name"] == "bili_jct"), "")
        dedeuserid = next((c["value"] for c in all_cookies if c["name"] == "DedeUserID"), "")

        result = {
            "SESSDATA": sessdata,
            "bili_jct": bili_jct or "",
            "DedeUserID": dedeuserid or "",
        }
        with open(result_file, "w", encoding="utf-8") as f:
            json.dump(result, f, ensure_ascii=False, indent=2)
        print(f"[login] Cookie 已保存到 {result_file}", flush=True)
        sys.exit(0)
    else:
        print(f"[login] 登录超时，未检测到登录状态", flush=True)
        # 保存已获取到的 cookie（可能部分成功）
        all_cookies = context.cookies()
        result = {}
        for c in all_cookies:
            if c["name"] in ("SESSDATA", "bili_jct", "DedeUserID"):
                result[c["name"]] = c["value"]
        if result.get("SESSDATA"):
            with open(result_file, "w", encoding="utf-8") as f:
                json.dump(result, f, ensure_ascii=False, indent=2)
            print(f"[login] 已保存部分 Cookie", flush=True)
            sys.exit(0)
        sys.exit(1)

    browser.close()
