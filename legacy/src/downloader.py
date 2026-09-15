"""视频分片下载器 - 支持多线程并发、分块加速、连接复用"""

import os
import time
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Callable

import requests


class VideoDownloader:
    """高性能视频/音频流下载器

    特性:
    - 连接复用 (Session + HTTP keep-alive)
    - 单文件多线程分块下载 (Range 请求)
    - 多分片并发下载
    """

    CHUNK_SIZE = 1024 * 1024  # 分块大小
    MAX_RETRIES = 3
    RETRY_DELAY = 1

    DEFAULT_HEADERS = {
        "User-Agent": (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
            "AppleWebKit/537.36 (KHTML, like Gecko) "
            "Chrome/125.0.0.0 Safari/537.36"
        ),
        "Referer": "https://www.bilibili.com/",
        "Origin": "https://www.bilibili.com",
    }

    def __init__(self, max_workers: int = 8, progress_callback: Callable = None):
        self.max_workers = max_workers
        self.progress_callback = progress_callback
        self._session = requests.Session()
        # 连接池: 足够的并发连接数
        adapter = requests.adapters.HTTPAdapter(
            pool_connections=64,
            pool_maxsize=128,
            max_retries=2,
        )
        self._session.mount("https://", adapter)
        self._session.mount("http://", adapter)
        self._session.headers.update(self.DEFAULT_HEADERS)
        self._lock = threading.Lock()
        self._total_downloaded = 0
        self._total_size = 0

    def _cb(self, chunk_size: int):
        """线程安全的进度回调"""
        if not self.progress_callback:
            return
        with self._lock:
            self._total_downloaded += chunk_size
            self.progress_callback(self._total_downloaded, self._total_size)

    # ------------------------------------------------------------------
    # 单文件多线程分块下载
    # ------------------------------------------------------------------

    def download(
        self,
        url: str,
        output_path: str,
        headers: dict = None,
        total_size: int = 0,
    ) -> str:
        """下载单个文件（自动启用或禁用分块加速）"""
        req_headers = {**self.DEFAULT_HEADERS, **(headers or {})}

        if total_size == 0:
            # 先 HEAD 获取文件大小
            try:
                head = self._session.head(url, headers=req_headers, timeout=15)
                total_size = int(head.headers.get("content-length", 0))
            except Exception:
                pass

        self._total_size = total_size
        self._total_downloaded = 0

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)

        # 大文件 + 多 worker → 分块下载
        if total_size > 5 * 1024 * 1024 and self.max_workers > 1:
            return self._download_chunked(url, output_path, req_headers, total_size)
        else:
            return self._download_stream(url, output_path, req_headers)

    def _download_chunked(
        self, url: str, output_path: str, headers: dict, total_size: int
    ) -> str:
        """多线程 Range 分块下载"""
        n_workers = min(self.max_workers, total_size // (1024 * 1024) + 1)
        chunk_size = total_size // n_workers
        results = {}

        def download_range(start: int, end: int, idx: int) -> tuple[int, bytes]:
            for attempt in range(self.MAX_RETRIES):
                try:
                    h = {**headers, "Range": f"bytes={start}-{end}"}
                    resp = self._session.get(url, headers=h, timeout=30)
                    if resp.status_code in (200, 206):
                        data = resp.content
                        self._cb(len(data))
                        return (idx, data)
                except Exception:
                    if attempt < self.MAX_RETRIES - 1:
                        time.sleep(self.RETRY_DELAY)
            raise DownloadError(f"Chunk {idx} failed after {self.MAX_RETRIES} retries")

        with ThreadPoolExecutor(max_workers=n_workers) as pool:
            futures = {}
            for i in range(n_workers):
                s = i * chunk_size
                e = s + chunk_size - 1 if i < n_workers - 1 else total_size - 1
                futures[pool.submit(download_range, s, e, i)] = i

            for future in as_completed(futures):
                idx, data = future.result()
                results[idx] = data

        with open(output_path, "wb") as f:
            for i in range(n_workers):
                f.write(results[i])

        return output_path

    def _download_stream(
        self, url: str, output_path: str, headers: dict
    ) -> str:
        """普通流式下载（小文件或单线程模式）"""
        for attempt in range(self.MAX_RETRIES):
            try:
                resp = self._session.get(url, headers=headers, stream=True, timeout=30)
                resp.raise_for_status()
                cl = int(resp.headers.get("content-length", 0))
                self._total_size = cl if cl > 0 else self._total_size
                downloaded = 0
                with open(output_path, "wb") as f:
                    for chunk in resp.iter_content(chunk_size=1024 * 1024):
                        if chunk:
                            f.write(chunk)
                            downloaded += len(chunk)
                            self._cb(len(chunk))
                return output_path
            except Exception as e:
                if attempt < self.MAX_RETRIES - 1:
                    time.sleep(self.RETRY_DELAY * (attempt + 1))
                    continue
                raise DownloadError(f"Download failed: {e}") from e

    # ------------------------------------------------------------------
    # 多分片下载
    # ------------------------------------------------------------------

    def download_segments(
        self,
        segments: list[dict],
        output_dir: str,
        prefix: str = "seg",
    ) -> list[str]:
        """并发下载多个分片"""
        os.makedirs(output_dir, exist_ok=True)
        results = []

        with ThreadPoolExecutor(max_workers=self.max_workers) as executor:
            futures = {}
            for i, seg in enumerate(segments):
                url = seg.get("url") or seg.get("base_url")
                if not url:
                    continue
                ext = ".m4s" if "m4s" in url else ".flv"
                out = os.path.join(output_dir, f"{prefix}_{i:04d}{ext}")
                futures[executor.submit(self.download, url, out)] = i

            for future in as_completed(futures):
                idx = futures[future]
                try:
                    results.append((idx, future.result()))
                except DownloadError as e:
                    print(f"[WARN] Segment {idx} failed: {e}")

        results.sort(key=lambda x: x[0])
        return [p for _, p in results]

    def download_with_concat(
        self,
        segments: list[dict],
        output_path: str,
    ) -> str:
        """下载所有分片并合并"""
        temp_dir = os.path.join(os.path.dirname(output_path), ".temp_dl")
        downloaded = self.download_segments(segments, temp_dir)
        if not downloaded:
            raise DownloadError("No segments downloaded")
        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as out:
            for p in downloaded:
                with open(p, "rb") as f:
                    out.write(f.read())
        import shutil
        shutil.rmtree(temp_dir, ignore_errors=True)
        return output_path


class DownloadError(Exception):
    pass
