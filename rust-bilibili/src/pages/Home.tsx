import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Check, Download, Search, Video } from 'lucide-react';
import { useLocation } from 'react-router-dom';
import { UrlInput } from '../components/UrlInput';
import { VideoInfo } from '../components/VideoInfo';
import { DownloadOptions } from '../components/DownloadOptions';
import { DownloadQueue } from '../components/DownloadQueue';
import type { CloudAuthStatus, CloudSaveMode, DanmakuMode, SteinDownloadMode, VideoData, DownloadTask, VideoPage } from '../types';
import * as Bridge from '../bridge';
import homeBg from '../assets/home-bg.png';

export function Home() {
  const location = useLocation();
  const [isParsing, setIsParsing] = useState(false);
  const [videos, setVideos] = useState<VideoData[]>([]);
  const [selectedVideoId, setSelectedVideoId] = useState<string | null>(null);
  const [selectedPageByVideo, setSelectedPageByVideo] = useState<Record<string, number>>({});
  const [selectedQuality, setSelectedQuality] = useState('1080P');
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [cookiePath, setCookiePath] = useState('');
  const [outdir, setOutdir] = useState('downloads');
  const [saveMode, setSaveMode] = useState<CloudSaveMode>('local');
  const [cloudRemoteDir, setCloudRemoteDir] = useState('/apps/B站充电视频下载器');
  const [cloudStatus, setCloudStatus] = useState<CloudAuthStatus>({
    provider: 'baidu_netdisk',
    is_authorized: false,
    message: '百度网盘未授权',
  });
  const [format, setFormat] = useState<'video' | 'audio'>('video');
  const [threads, setThreads] = useState(8);
  const [danmakuMode, setDanmakuMode] = useState<DanmakuMode>('none');
  const [steinMode, setSteinMode] = useState<SteinDownloadMode>('all');
  const [steinPathEdges, setSteinPathEdges] = useState<number[]>([]);
  const [ffmpegAvailable, setFfmpegAvailable] = useState(false);
  const [isBatchStarting, setIsBatchStarting] = useState(false);
  const [loadingSizeVideoIds, setLoadingSizeVideoIds] = useState<Set<string>>(() => new Set());
  const sizeLoadAttemptedRef = useRef<Set<string>>(new Set());
  const sizeLoadInFlightRef = useRef<Set<string>>(new Set());

  const refreshCloudState = useCallback(() => {
    Bridge.getCloudConfig().then(cfg => {
      setSaveMode(cfg.default_save_mode);
      setCloudRemoteDir(cfg.default_remote_dir);
    });
    Bridge.baiduAuthStatus().then(setCloudStatus).catch(() => {
      setCloudStatus({
        provider: 'baidu_netdisk',
        is_authorized: false,
        message: '百度网盘未授权',
      });
    });
  }, []);

  useEffect(() => {
    Bridge.checkFfmpeg().then(status => setFfmpegAvailable(status.available));
    Bridge.getDefaultOutdir().then(setOutdir);
    // Restore cookie path
    Bridge.getConfig().then(cfg => {
      if (cfg.default_outdir) setOutdir(cfg.default_outdir);
      if (cfg.default_quality) setSelectedQuality(cfg.default_quality);
    });
    refreshCloudState();
  }, [refreshCloudState]);

  useEffect(() => {
    if (location.pathname === '/') {
      refreshCloudState();
    }
  }, [location.pathname, refreshCloudState]);

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

  const activeVideo = videos.find(video => video.id === selectedVideoId) ?? videos[0] ?? null;
  const activePageIndex = activeVideo
    ? Math.min(selectedPageByVideo[activeVideo.id] ?? 0, Math.max(activeVideo.pages.length - 1, 0))
    : 0;
  const activePage = activeVideo?.pages[activePageIndex] ?? activeVideo?.pages[0] ?? null;

  useEffect(() => {
    if (activeVideo?.is_stein) {
      setSteinMode((prev) => (prev === 'path' ? 'path' : 'all'));
    } else {
      setSteinMode('none');
      setSteinPathEdges([]);
    }
  }, [activeVideo?.id, activeVideo?.is_stein]);

  const totalDownloadItems = videos.reduce(
    (total, video) => total + Math.max(video.pages.length, 1),
    0,
  );
  const visibleVideoCount = videos.length > 80 ? 80 : videos.length;
  const visibleVideos = videos.slice(0, visibleVideoCount);

  useEffect(() => {
    if (!activeVideo || videoHasKnownSizes(activeVideo)) {
      return;
    }

    let cancelled = false;
    const videoId = activeVideo.id;
    const cid = activePage?.cid ?? activeVideo.pages[0]?.cid ?? null;
    const cacheKey = `${videoId}:${cid ?? 'default'}:${cookiePath}`;

    if (sizeLoadAttemptedRef.current.has(cacheKey) || sizeLoadInFlightRef.current.has(cacheKey)) {
      return;
    }

    sizeLoadAttemptedRef.current.add(cacheKey);
    sizeLoadInFlightRef.current.add(cacheKey);
    setLoadingSizeVideoIds(prev => new Set(prev).add(videoId));
    Bridge.enrichVideoSizes(activeVideo, cid, cookiePath)
      .then((enriched) => {
        setVideos(prev => prev.map(video => (video.id === videoId ? enriched : video)));
        if (!cancelled) {
          setSelectedQuality(current => firstQualityForFormat(enriched, format, current));
        }
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : '获取视频容量失败';
        if (!cancelled) setNotice(`容量获取失败：${message}`);
      })
      .finally(() => {
        sizeLoadInFlightRef.current.delete(cacheKey);
        setLoadingSizeVideoIds(prev => {
          const next = new Set(prev);
          next.delete(videoId);
          return next;
        });
      });

    return () => {
      cancelled = true;
    };
  }, [activeVideo, activePage?.cid, cookiePath, format]);

  const handleParse = useCallback(async (url: string) => {
    setError(null);
    setNotice(null);
    const bvids = extractBvids(url);
    if (bvids.length === 0) {
      setError('请输入包含 BV 号的有效 B站视频链接');
      return;
    }

    setIsParsing(true);
    try {
      const parsedVideos: VideoData[] = [];
      const failures: string[] = [];

      for (const bvid of bvids) {
        try {
          const results = await Bridge.fetchInfoList(bvid, cookiePath);
          const validResults = results.filter(result => !result.error);
          if (validResults.length === 0) {
            failures.push(`${bvid}：${results[0]?.error || '获取视频信息失败'}`);
          } else {
            parsedVideos.push(...validResults);
          }
        } catch (err: unknown) {
          const message = err instanceof Error ? err.message : '获取视频信息失败';
          failures.push(`${bvid}：${message}`);
        }
      }

      if (parsedVideos.length === 0) {
        setVideos([]);
        setSelectedVideoId(null);
        setError(failures[0] || '获取视频信息失败');
        return;
      }

      const uniqueVideos = dedupeVideos(parsedVideos);
      sizeLoadAttemptedRef.current.clear();
      sizeLoadInFlightRef.current.clear();
      setLoadingSizeVideoIds(new Set());
      setVideos(uniqueVideos);
      setSelectedVideoId(uniqueVideos[0].id);
      setSelectedPageByVideo(Object.fromEntries(uniqueVideos.map(video => [video.id, 0])));
      setSelectedQuality(firstQualityForFormat(uniqueVideos[0], format, selectedQuality));
      setSteinMode(uniqueVideos[0].is_stein ? 'all' : 'none');
      setSteinPathEdges([]);
      if (failures.length > 0) {
        setNotice(`已解析 ${uniqueVideos.length} 个，失败 ${failures.length} 个`);
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : '获取视频信息失败');
    } finally {
      setIsParsing(false);
    }
  }, [cookiePath, format, selectedQuality]);

  const startVideoDownload = useCallback(async (
    video: VideoData,
    quality = selectedQuality,
    page: VideoPage | null = video.pages[0] ?? null,
  ): Promise<DownloadTask> => {
    const requestedDanmakuMode = safeDanmakuModeForQuality(format, quality, danmakuMode);
    const isCloudMode = saveMode === 'baidu_netdisk';
    const isStein = Boolean(video.is_stein);
    const effectiveSteinMode: SteinDownloadMode = isStein
      ? (steinMode === 'path' ? 'path' : 'all')
      : 'none';
    if (isStein && effectiveSteinMode === 'path' && steinPathEdges.length === 0) {
      throw new Error('请先选择互动视频的分支路径');
    }
    const taskId = isCloudMode
      ? await Bridge.startCloudUpload(
        video.id, quality, format,
        cloudRemoteDir, cookiePath,
        threads,
        format === 'video' ? requestedDanmakuMode : 'none',
        page,
      )
      : await Bridge.startDownload(
        video.id, quality, format,
        outdir, cookiePath, format === 'audio',
        threads,
        format === 'video' && !isStein ? requestedDanmakuMode : 'none',
        page,
        effectiveSteinMode,
        isStein && effectiveSteinMode === 'path' ? steinPathEdges : [],
      );
    const effectiveDanmakuMode = format === 'video' && !isStein ? requestedDanmakuMode : 'none';
    const title = isStein
      ? `${video.title} [互动${effectiveSteinMode === 'path' ? '路径' : '整图'}]`
      : page && video.pages.length > 1
        ? `${video.title} - P${page.page} ${page.part}`
        : video.title;
    return {
      id: taskId,
      bvid: video.id,
      title,
      quality,
      format,
      progress: 0,
      status: 'downloading',
      output_dir: isCloudMode ? cloudRemoteDir : outdir,
      storage: saveMode,
      download_danmaku: effectiveDanmakuMode !== 'none',
      danmaku_mode: effectiveDanmakuMode,
      message: isCloudMode ? cloudTaskMessage(effectiveDanmakuMode) : danmakuTaskMessage(effectiveDanmakuMode),
    };
  }, [selectedQuality, format, saveMode, cloudRemoteDir, outdir, cookiePath, threads, danmakuMode, steinMode, steinPathEdges]);

  const handleDownload = useCallback(async () => {
    if (!activeVideo) {
      setError('请先解析视频链接，再开始下载');
      return;
    }

    try {
      const newTask = await startVideoDownload(activeVideo, selectedQuality, activePage);
      setTasks(prev => [newTask, ...prev]);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [activeVideo, startVideoDownload]);

  const handleDownloadAll = useCallback(async () => {
    if (isBatchStarting) return;
    if (videos.length === 0) {
      setError('请先解析视频链接，再开始下载');
      return;
    }

    setError(null);
    setIsBatchStarting(true);
    setNotice(`正在展开合集并创建下载任务，请稍等...`);
    try {
      const expandedVideos = videos;
      const downloadItems = expandedVideos.flatMap(video => {
        const pages = video.pages.length > 0 ? video.pages : [null];
        return pages.map((page) => ({
          video,
          page,
          quality: firstQualityForFormat(video, format, selectedQuality),
        }));
      });
      setNotice(`已展开 ${downloadItems.length} 个下载项，正在创建队列...`);

      let startedCount = 0;
      let bufferedTasks: DownloadTask[] = [];
      const failedMessages: string[] = [];
      for (const [index, item] of downloadItems.entries()) {
        try {
          const task = await startVideoDownload(item.video, item.quality, item.page);
          bufferedTasks.push(task);
          startedCount += 1;
          if (bufferedTasks.length >= 5 || index === downloadItems.length - 1) {
            const chunk = bufferedTasks;
            bufferedTasks = [];
            setTasks(prev => [...chunk, ...prev]);
            setNotice(`正在创建下载队列 ${index + 1}/${downloadItems.length}`);
          }
        } catch (err) {
          const title = item.page && item.video.pages.length > 1
            ? `${item.video.title} - P${item.page.page}`
            : item.video.title;
          const message = err instanceof Error ? err.message : String(err);
          failedMessages.push(`${title}：${message}`);
        }
      }

      if (bufferedTasks.length > 0) {
        const chunk = bufferedTasks;
        bufferedTasks = [];
        setTasks(prev => [...chunk, ...prev]);
      }
      const failedCount = failedMessages.length;
      if (failedCount > 0) {
        setNotice(`已启动 ${startedCount} 个下载，失败 ${failedCount} 个`);
      }
      if (startedCount === 0 && failedCount > 0) {
        throw new Error(failedMessages[0] ?? '批量下载启动失败');
      }
      if (failedCount === 0) {
        setNotice(`已创建 ${startedCount} 个下载任务`);
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsBatchStarting(false);
    }
  }, [isBatchStarting, videos, format, selectedQuality, startVideoDownload]);

  const handleSelectVideo = useCallback((video: VideoData) => {
    setSelectedVideoId(video.id);
    setSelectedQuality(firstQualityForFormat(video, format, selectedQuality));
  }, [format, selectedQuality]);

  const handleSelectPage = useCallback((page: VideoPage) => {
    if (!activeVideo) return;
    const nextIndex = activeVideo.pages.findIndex(item => item.cid === page.cid);
    if (nextIndex >= 0) {
      setSelectedPageByVideo(prev => ({ ...prev, [activeVideo.id]: nextIndex }));
    }
  }, [activeVideo]);

  const handleStepPage = useCallback((direction: -1 | 1) => {
    if (!activeVideo || activeVideo.pages.length <= 1) return;
    setSelectedPageByVideo(prev => {
      const current = prev[activeVideo.id] ?? 0;
      const next = (current + direction + activeVideo.pages.length) % activeVideo.pages.length;
      return { ...prev, [activeVideo.id]: next };
    });
  }, [activeVideo]);

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
          {notice && (
            <div className="absolute left-1/2 top-full z-40 mt-4 -translate-x-1/2 whitespace-nowrap rounded-xl border border-orange-100 bg-orange-50 px-5 py-2.5 text-sm font-bold text-orange-500 shadow-sm">
              {notice}
            </div>
          )}
        </div>
      </div>

      {activeVideo ? (
        <div className="relative z-10 grid grid-cols-1 xl:grid-cols-[minmax(500px,720px)_430px] 2xl:grid-cols-[minmax(540px,780px)_460px] gap-6 xl:gap-10 2xl:gap-12 pb-8 items-start justify-center flex-1">
          <div className="min-w-0 flex flex-col gap-3">
            {videos.length > 1 && (
              <div className="glass-panel rounded-[1.75rem] p-4">
                <div className="mb-3 flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2 text-sm font-black text-gray-700">
                    <Video size={18} className="text-bili-pink" />
                    已解析 {videos.length} 个视频
                  </div>
                  <button
                    type="button"
                    onClick={handleDownloadAll}
                    disabled={isBatchStarting}
                    className="motion-button inline-flex min-h-10 items-center justify-center gap-2 rounded-2xl bg-gradient-to-r from-[#FF9FC0] to-[#FF86B2] px-4 text-sm font-black text-white shadow-[0_10px_22px_rgba(255,134,178,0.28)] disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    <Download size={16} strokeWidth={2.6} />
                    {isBatchStarting ? '正在创建队列' : '下载全部'}
                  </button>
                </div>
                <div className="flex max-h-36 flex-col gap-2 overflow-y-auto pr-1 custom-scrollbar">
                  {visibleVideos.map((video, index) => {
                    const selected = video.id === activeVideo.id;
                    return (
                      <button
                        key={video.id}
                        type="button"
                        onClick={() => handleSelectVideo(video)}
                        className={`motion-button flex items-center gap-3 rounded-2xl border px-3 py-2.5 text-left ${
                          selected
                            ? 'border-pink-100 bg-white/86 text-bili-pink shadow-sm'
                            : 'border-white/70 bg-white/52 text-gray-600 hover:border-pink-100 hover:bg-white/78'
                        }`}
                      >
                        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-xl bg-pink-50 text-xs font-black text-bili-pink">
                          {index + 1}
                        </span>
                        <span className="min-w-0 flex-1 truncate text-sm font-bold">{video.title}</span>
                        {video.pages.length > 1 && (
                          <span className="rounded-lg bg-white/70 px-2 py-1 text-[11px] font-black text-gray-400">
                            {video.pages.length}P
                          </span>
                        )}
                        {selected && <Check size={16} strokeWidth={2.8} />}
                      </button>
                    );
                  })}
                  {videos.length > visibleVideoCount && (
                    <div className="px-4 py-2 text-center text-xs font-bold text-gray-400">
                      已显示前 {visibleVideoCount} 个，下载全部仍会包含 {videos.length} 个视频
                    </div>
                  )}
                </div>
              </div>
            )}
            <VideoInfo
              data={activeVideo}
              currentPage={activePage}
              currentPageIndex={activePageIndex}
              onPrevPage={() => handleStepPage(-1)}
              onNextPage={() => handleStepPage(1)}
              onSelectPage={handleSelectPage}
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
              data={activeVideo}
              selectedQuality={selectedQuality}
              onSelectQuality={setSelectedQuality}
              onDownload={handleDownload}
              batchCount={totalDownloadItems}
              currentPageLabel={activePage && activeVideo.pages.length > 1 ? `P${activePage.page}` : undefined}
              onDownloadAll={totalDownloadItems > 1 && !activeVideo.is_stein ? handleDownloadAll : undefined}
              isBatchStarting={isBatchStarting}
              saveMode={saveMode}
              onSaveModeChange={setSaveMode}
              cloudAuthorized={cloudStatus.is_authorized}
              cloudRemoteDir={cloudRemoteDir}
              format={format}
              onFormatChange={handleFormatChange}
              threads={threads}
              onThreadsChange={setThreads}
              danmakuMode={danmakuMode}
              onDanmakuModeChange={setDanmakuMode}
              ffmpegAvailable={ffmpegAvailable}
              isLoadingSizes={activeVideo ? loadingSizeVideoIds.has(activeVideo.id) : false}
              steinMode={activeVideo.is_stein ? steinMode : 'none'}
              onSteinModeChange={setSteinMode}
              steinPathEdges={steinPathEdges}
              onSteinPathEdgesChange={setSteinPathEdges}
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
  // 弹幕状态消息由前端自己生成（danmakuTaskMessage），完成时用相等比较保留，不匹配后端散文
  const shouldKeepDanmakuMessage =
    task.download_danmaku &&
    task.message &&
    task.message === danmakuTaskMessage(task.danmaku_mode ?? 'none');

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

function cloudTaskMessage(mode: DanmakuMode): string {
  if (mode === 'ass') return '百度网盘直传 MP4，已选择外挂弹幕';
  if (mode === 'burn') return '百度网盘直传 MP4，已选择烧录弹幕';
  return '百度网盘直传 MP4，不写入本地大视频文件';
}

function safeDanmakuModeForQuality(
  format: 'video' | 'audio',
  quality: string,
  mode: DanmakuMode,
): DanmakuMode {
  if (format !== 'video') return 'none';
  if (
    mode === 'burn' &&
    (quality.includes('8K') ||
      quality.includes('HDR') ||
      quality.includes('杜比'))
  ) {
    return 'ass';
  }
  return mode;
}

function firstQualityForFormat(
  video: VideoData,
  format: 'video' | 'audio',
  currentQuality: string,
): string {
  if (format === 'audio') {
    const audioLabels = video.audio_streams?.map(stream => stream.label) ?? [];
    return audioLabels.includes(currentQuality)
      ? currentQuality
      : audioLabels[0] || '320kbps 高品质';
  }
  return video.qualities.includes(currentQuality)
    ? currentQuality
    : video.qualities[0] || currentQuality;
}

function videoHasKnownSizes(video: VideoData): boolean {
  return video.streams.some(stream => Boolean(stream.size_bytes && stream.size_bytes > 0)) ||
    video.audio_streams.some(stream => Boolean(stream.size_bytes && stream.size_bytes > 0));
}

function extractBvids(text: string): string[] {
  const matches = text.match(/BV[a-zA-Z0-9]{10,}/g) ?? [];
  return [...new Set(matches)];
}

function dedupeVideos(videos: VideoData[]): VideoData[] {
  return videos.reduce<VideoData[]>((unique, video) => {
    if (!unique.some(item => item.id === video.id)) {
      unique.push(video);
    }
    return unique;
  }, []);
}
