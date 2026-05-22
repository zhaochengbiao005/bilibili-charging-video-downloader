import React, { useState, useEffect, useCallback } from 'react';
import { Search } from 'lucide-react';
import { UrlInput } from '../components/UrlInput';
import { VideoInfo } from '../components/VideoInfo';
import { DownloadOptions } from '../components/DownloadOptions';
import { DownloadQueue } from '../components/DownloadQueue';
import type { DanmakuMode, VideoData, DownloadTask } from '../types';
import * as Bridge from '../bridge';
import homeBg from '../assets/home-bg.png';

export function Home() {
  const [isParsing, setIsParsing] = useState(false);
  const [videoData, setVideoData] = useState<VideoData | null>(null);
  const [selectedQuality, setSelectedQuality] = useState('1080P');
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [cookiePath, setCookiePath] = useState('');
  const [outdir, setOutdir] = useState('downloads');
  const [format, setFormat] = useState<'video' | 'audio'>('video');
  const [threads, setThreads] = useState(8);
  const [danmakuMode, setDanmakuMode] = useState<DanmakuMode>('none');
  const [ffmpegAvailable, setFfmpegAvailable] = useState(false);

  useEffect(() => {
    Bridge.checkFfmpeg().then(status => setFfmpegAvailable(status.available));
    Bridge.getDefaultOutdir().then(setOutdir);
    // Restore cookie path
    Bridge.getConfig().then(cfg => {
      if (cfg.default_outdir) setOutdir(cfg.default_outdir);
      if (cfg.default_quality) setSelectedQuality(cfg.default_quality);
    });
  }, []);

  // ── 注册 Rust 后端事件回调 ──
  useEffect(() => {
    Bridge.setOnProgress((taskId, percent, speed, event) => {
      setTasks(prev => prev.map(t =>
        t.id === taskId ? { ...t, progress: percent, speed, message: event.message || t.message } : t
      ));
    });

    Bridge.setOnLog((msg) => {
      // Could also show in a toast/notification
      console.log('[DL]', msg);
    });

    Bridge.setOnTaskDone((taskId, result) => {
      setTasks(prev => prev.map(t =>
        t.id === taskId ? finishTask(t, result) : t
      ));
    });

    return () => {
      Bridge.setOnProgress(null);
      Bridge.setOnLog(null);
      Bridge.setOnTaskDone(null);
    };
  }, []);

  const handleParse = useCallback(async (url: string) => {
    setError(null);
    const bvid = extractBvid(url);
    if (!bvid) {
      setError('请输入包含 BV 号的有效 B站视频链接');
      return;
    }

    setIsParsing(true);
    try {
      const result = await Bridge.fetchInfo(bvid, cookiePath);
      if (result.error) {
        setError(result.error);
        setIsParsing(false);
        return;
      }
      setVideoData(result);
      if (result.qualities?.length > 0) {
        setSelectedQuality(result.qualities[0]);
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : '获取视频信息失败');
    }
    setIsParsing(false);
  }, [cookiePath]);

  const handleDownload = useCallback(async () => {
    if (!videoData) {
      setError('请先解析视频链接，再开始下载');
      return;
    }

    try {
      const taskId = await Bridge.startDownload(
        videoData.id, selectedQuality, format,
        outdir, cookiePath, format === 'audio',
        threads,
        format === 'video' ? danmakuMode : 'none'
      );
      const effectiveDanmakuMode = format === 'video' ? danmakuMode : 'none';
      const newTask: DownloadTask = {
        id: taskId,
        bvid: videoData.id,
        title: videoData.title,
        quality: selectedQuality,
        format,
        progress: 0,
        status: 'downloading',
        output_dir: outdir,
        download_danmaku: effectiveDanmakuMode !== 'none',
        danmaku_mode: effectiveDanmakuMode,
        message: danmakuTaskMessage(effectiveDanmakuMode),
      };
      setTasks(prev => [newTask, ...prev]);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [videoData, selectedQuality, format, outdir, cookiePath, threads, danmakuMode]);

  const handleCancel = useCallback(async (taskId: string) => {
    await Bridge.cancelDownload(taskId);
    setTasks(prev => prev.map(t =>
      t.id === taskId ? { ...t, status: 'cancelled' } : t
    ));
  }, []);

  const handleRemove = useCallback((taskId: string) => {
    setTasks(prev => prev.filter(t => t.id !== taskId));
  }, []);

  const handleCookieChange = useCallback((path: string) => {
    setCookiePath(path);
  }, []);

  const handleFormatChange = useCallback((f: 'video' | 'audio') => {
    setFormat(f);
  }, []);

  const filteredTasks = tasks.filter(t =>
    t.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
    (t.bvid?.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  return (
    <div className="relative w-full min-h-full px-6 md:px-10 xl:px-14 2xl:px-16 py-8 md:py-10 flex flex-col gap-7 md:gap-8">
      <img
        src={homeBg}
        alt=""
        aria-hidden="true"
        className="pointer-events-none fixed bottom-0 right-0 z-0 h-[min(68vh,680px)] w-auto max-w-[46vw] object-contain object-bottom opacity-68 drop-shadow-[0_22px_52px_rgba(123,207,255,0.16)] xl:h-[min(74vh,780px)] 2xl:h-[min(78vh,880px)]"
      />
      <div className="text-center flex flex-col items-center shrink-0 relative">
        <div className="bili-soft-pattern absolute -top-4 left-1/2 h-24 w-[420px] -translate-x-1/2 rounded-full opacity-45 blur-[0.2px]" />
        <h1 className="relative text-[34px] font-black tracking-tight text-[#1F2937] drop-shadow-sm">下载你喜欢的视频</h1>
        <p className="mt-3 text-base text-gray-500 font-medium">粘贴 B站视频链接，快速解析并下载高清视频与音频</p>
      </div>

      <div className="flex justify-center shrink-0 relative z-10 mt-8 md:mt-10">
        <div className="w-full max-w-[980px] relative">
          <UrlInput
            onParse={handleParse}
            isParsing={isParsing}
            error={error}
          />
        </div>
      </div>

      {videoData ? (
        <div className="relative z-10 grid grid-cols-1 xl:grid-cols-[minmax(500px,720px)_430px] 2xl:grid-cols-[minmax(540px,780px)_460px] gap-6 xl:gap-10 2xl:gap-12 pb-8 items-start justify-center flex-1">
          <div className="min-w-0 flex flex-col gap-3">
            <VideoInfo data={videoData} />
            {tasks.length > 0 && (
              <div className="flex flex-col gap-4">
                <div className="relative">
                  <Search size={18} className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-400" />
                  <input
                    type="text"
                    placeholder="搜索下载记录..."
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    className="w-full bg-white/68 border border-white/90 rounded-2xl py-3 pl-12 pr-4 focus:outline-none focus:ring-2 focus:ring-bili-pink/25 text-sm shadow-[0_10px_28px_rgba(255,143,179,0.12)] backdrop-blur-sm text-[#1F2937] font-medium placeholder-gray-400"
                  />
                </div>
                <DownloadQueue
                  tasks={filteredTasks}
                  onRemove={handleRemove}
                  onCancel={handleCancel}
                />
              </div>
            )}
          </div>
          <div className="flex flex-col gap-8 xl:sticky xl:top-8">
            <DownloadOptions
              data={videoData}
              selectedQuality={selectedQuality}
              onSelectQuality={setSelectedQuality}
              onDownload={handleDownload}
              format={format}
              onFormatChange={handleFormatChange}
              threads={threads}
              onThreadsChange={setThreads}
              danmakuMode={danmakuMode}
              onDanmakuModeChange={setDanmakuMode}
              ffmpegAvailable={ffmpegAvailable}
            />
          </div>
        </div>
      ) : null}
    </div>
  );
}

function finishTask(task: DownloadTask, result: { status: string; message?: string }): DownloadTask {
  const status = result.status === 'completed'
    ? 'completed' as const
    : result.status === 'cancelled'
      ? 'cancelled' as const
      : 'error' as const;
  const shouldKeepDanmakuMessage =
    task.download_danmaku &&
    task.message &&
    (task.message.includes('弹幕文件') || task.message.includes('弹幕'));

  return {
    ...task,
    status,
    progress: result.status === 'completed' ? 100 : task.progress,
    error_message: result.status === 'failed' ? result.message : undefined,
    message: shouldKeepDanmakuMessage ? task.message : result.message || task.message,
  };
}

function danmakuTaskMessage(mode: DanmakuMode): string | undefined {
  if (mode === 'ass') return '已选择外挂弹幕';
  if (mode === 'burn') return '已选择烧录弹幕';
  return undefined;
}

function extractBvid(text: string): string | null {
  const m = /BV\w{10,}/.exec(text.trim());
  return m ? m[0] : null;
}
