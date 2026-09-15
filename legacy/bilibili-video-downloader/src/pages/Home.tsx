import React, { useState, useEffect, useCallback } from 'react';
import { Search } from 'lucide-react';
import { UrlInput } from '../components/UrlInput';
import { VideoInfo } from '../components/VideoInfo';
import { DownloadOptions } from '../components/DownloadOptions';
import { DownloadQueue } from '../components/DownloadQueue';
import type { VideoData, DownloadTask } from '../types';
import * as Bridge from '../bridge';

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
  const [ffmpegAvailable, setFfmpegAvailable] = useState(false);

  useEffect(() => {
    Bridge.checkFfmpeg().then(setFfmpegAvailable);
    Bridge.getDefaultOutdir().then(setOutdir);
    // Restore cookie path
    Bridge.getConfig().then(cfg => {
      if (cfg.default_outdir) setOutdir(cfg.default_outdir);
      if (cfg.default_quality) setSelectedQuality(cfg.default_quality);
    });
  }, []);

  // ── 注册 Python→JS 回调 ──
  useEffect(() => {
    Bridge.setOnProgress((taskId, percent, speed) => {
      setTasks(prev => prev.map(t =>
        t.id === taskId ? { ...t, progress: percent, speed } : t
      ));
    });

    Bridge.setOnLog((msg) => {
      // Could also show in a toast/notification
      console.log('[DL]', msg);
    });

    Bridge.setOnTaskDone((taskId, result) => {
      setTasks(prev => prev.map(t =>
        t.id === taskId
          ? { ...t, status: result.status === 'completed' ? 'completed' as const : 'error' as const, progress: result.status === 'completed' ? 100 : t.progress }
          : t
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
      setError('请输入有效的 B站视频链接（bilibili.com/video/BV...）');
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
      setError(err instanceof Error ? err.message : '获取信息失败');
    }
    setIsParsing(false);
  }, [cookiePath]);

  const handleDownload = useCallback(async () => {
    if (!videoData) return;

    const newTask: DownloadTask = {
      id: `dl_${Date.now()}`,
      bvid: videoData.id,
      title: videoData.title,
      quality: selectedQuality,
      format,
      progress: 0,
      status: 'downloading',
      output_dir: outdir,
    };

    setTasks(prev => [newTask, ...prev]);

    try {
      await Bridge.startDownload(
        videoData.id, selectedQuality, format,
        outdir, cookiePath, format === 'audio' || !ffmpegAvailable
      );
    } catch (err: unknown) {
      setTasks(prev => prev.map(t =>
        t.id === newTask.id ? { ...t, status: 'error', error_message: err instanceof Error ? err.message : String(err) } : t
      ));
    }
  }, [videoData, selectedQuality, format, outdir, cookiePath, ffmpegAvailable]);

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
    <div className="max-w-6xl mx-auto w-full p-8 md:p-12 flex flex-col gap-10 min-h-full">
      {/* Header */}
      <div className="text-center flex flex-col items-center mt-4 shrink-0">
        <h1 className="text-4xl font-black tracking-tight mb-4 text-gray-900 drop-shadow-sm">
          下载你喜欢的视频
        </h1>
        <p className="text-lg text-gray-600 font-medium max-w-xl">
          粘贴 B站视频链接，快速解析并下载高清视频与音频
        </p>
      </div>

      {/* 搜索/输入 */}
      <div className="flex justify-center shrink-0">
        <div className="w-full max-w-3xl glass-panel rounded-full p-2 pl-6 relative">
          <UrlInput
            onParse={handleParse}
            isParsing={isParsing}
            error={error}
            cookiePath={cookiePath}
            onCookieChange={handleCookieChange}
          />
        </div>
      </div>

      {/* 内容区域 */}
      {videoData ? (
        <div className="flex flex-col lg:flex-row gap-8 pb-10">
          <div className="flex-1 min-w-0">
            <VideoInfo data={videoData} />
          </div>
          <div className="w-full lg:w-[380px] xl:w-[420px] shrink-0 flex flex-col gap-8">
            <DownloadOptions
              data={videoData}
              selectedQuality={selectedQuality}
              onSelectQuality={setSelectedQuality}
              onDownload={handleDownload}
              format={format}
              onFormatChange={handleFormatChange}
              ffmpegAvailable={ffmpegAvailable}
            />
            {tasks.length > 0 && (
              <div className="flex flex-col gap-4">
                <div className="relative">
                  <Search size={18} className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-400" />
                  <input
                    type="text"
                    placeholder="搜索下载记录..."
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    className="w-full bg-white/50 border border-white/80 rounded-2xl py-3 pl-12 pr-4 focus:outline-none focus:ring-2 focus:ring-bili-pink/30 text-sm shadow-sm backdrop-blur-sm text-gray-700 font-medium placeholder-gray-400"
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
        </div>
      ) : (
        <div className="flex flex-col items-center justify-center py-24 text-gray-400 glass-panel border-dashed rounded-[2rem] mt-4 opacity-70">
          <div className="w-20 h-20 bg-white/50 rounded-full flex items-center justify-center text-3xl mb-4 backdrop-blur-md shadow-sm border border-white">
            ✨
          </div>
          <p className="font-bold text-gray-500">粘贴视频链接开始使用</p>
        </div>
      )}
    </div>
  );
}

function extractBvid(text: string): string | null {
  const m = /BV\w{10,}/.exec(text.trim());
  return m ? m[0] : null;
}
