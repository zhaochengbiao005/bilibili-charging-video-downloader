"""视频/音频合并器 - 使用 FFmpeg 合并 DASH 分离的音视频流"""

import os
import subprocess
import sys
from pathlib import Path


class FFmpegMerger:
    """FFmpeg 封装，合并下载的音视频文件"""

    def __init__(self, ffmpeg_path: str | None = None):
        if ffmpeg_path:
            self.ffmpeg = ffmpeg_path
        else:
            self.ffmpeg = self._find_ffmpeg()

    @staticmethod
    def _find_ffmpeg() -> str:
        """按优先级查找 ffmpeg"""
        # 1. 同目录（PyInstaller 打包 / 手动放置）
        base = Path(sys.executable if getattr(sys, "frozen", False) else __file__).parent
        local = base / "ffmpeg.exe"
        if local.exists():
            return str(local)

        # 2. 用户数据目录（自动下载缓存）
        data_dir = Path.home() / ".bilibili_downloader"
        cached = data_dir / "ffmpeg.exe"
        if cached.exists():
            return str(cached)

        # 3. 系统 PATH
        return "ffmpeg"

    @property
    def cache_dir(self) -> Path:
        return Path.home() / ".bilibili_downloader"

    def check_available(self) -> bool:
        try:
            subprocess.run(
                [self.ffmpeg, "-version"],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                check=True, timeout=10,
            )
            return True
        except Exception:
            return False

    def auto_download(self, progress_callback=None) -> str:
        """自动下载 FFmpeg 并缓存"""
        import requests as req

        url = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
        cache_dir = self.cache_dir
        cache_dir.mkdir(parents=True, exist_ok=True)

        zip_path = cache_dir / "ffmpeg.zip"
        exe_path = cache_dir / "ffmpeg.exe"

        if exe_path.exists():
            self.ffmpeg = str(exe_path)
            return str(exe_path)

        # 下载
        if progress_callback:
            progress_callback(0, "下载 FFmpeg...")
        resp = req.get(url, stream=True, timeout=120)
        resp.raise_for_status()
        total = int(resp.headers.get("content-length", 0))
        downloaded = 0
        with open(zip_path, "wb") as f:
            for chunk in resp.iter_content(chunk_size=65536):
                f.write(chunk)
                downloaded += len(chunk)
                if total > 0 and progress_callback:
                    pct = int(downloaded / total * 100)
                    progress_callback(pct, f"下载 FFmpeg {pct}%")

        # 解压
        if progress_callback:
            progress_callback(90, "解压 FFmpeg...")
        import zipfile
        with zipfile.ZipFile(zip_path) as z:
            for name in z.namelist():
                if name.endswith("ffmpeg.exe"):
                    with open(exe_path, "wb") as f:
                        f.write(z.read(name))
                    break

        zip_path.unlink(missing_ok=True)
        self.ffmpeg = str(exe_path)
        if progress_callback:
            progress_callback(100, "FFmpeg 就绪")
        return str(exe_path)

    def merge_video_audio(
        self, video_path: str, audio_path: str,
        output_path: str, overwrite: bool = True,
    ) -> str:
        if not self.check_available():
            raise FFmpegNotFoundError(
                "FFmpeg not found. Use auto_download() or install from ffmpeg.org"
            )
        cmd = [self.ffmpeg, "-i", video_path, "-i", audio_path,
               "-c:v", "copy", "-c:a", "copy",
               "-map", "0:v:0", "-map", "1:a:0"]
        if overwrite:
            cmd.append("-y")
        cmd.append(output_path)
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise MergeError(f"FFmpeg merge failed:\n{result.stderr}")
        return output_path

    def convert_to_mp3(
        self, input_path: str, output_path: str | None = None,
        overwrite: bool = True, bitrate: str = "192k",
    ) -> str:
        """将音频文件转为 MP3（需要 FFmpeg）"""
        if not self.check_available():
            raise FFmpegNotFoundError("FFmpeg not found")
        if not output_path:
            output_path = os.path.splitext(input_path)[0] + ".mp3"
        cmd = [self.ffmpeg, "-i", input_path, "-codec:a", "libmp3lame",
               "-b:a", bitrate, "-vn"]
        if overwrite:
            cmd.append("-y")
        cmd.append(output_path)
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise MergeError(f"FFmpeg MP3 conversion failed:\n{result.stderr}")
        return output_path

    def merge_concat(
        self, segments: list[str],
        output_path: str, overwrite: bool = True,
    ) -> str:
        if not self.check_available():
            raise FFmpegNotFoundError("FFmpeg not found")
        concat_file = output_path + ".concat.txt"
        with open(concat_file, "w", encoding="utf-8") as f:
            for seg in segments:
                abspath = os.path.abspath(seg).replace("\\", "/")
                f.write(f"file '{abspath}'\n")
        cmd = [self.ffmpeg, "-f", "concat", "-safe", "0",
               "-i", concat_file, "-c", "copy"]
        if overwrite:
            cmd.append("-y")
        cmd.append(output_path)
        result = subprocess.run(cmd, capture_output=True, text=True)
        os.remove(concat_file)
        if result.returncode != 0:
            raise MergeError(f"FFmpeg concat failed:\n{result.stderr}")
        return output_path


class FFmpegNotFoundError(Exception):
    pass


class MergeError(Exception):
    pass
