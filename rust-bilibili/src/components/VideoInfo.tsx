import React, { useEffect, useState } from 'react';
import { ChevronLeft, ChevronRight, Eye, Clock, Layers, AlertTriangle, Crown } from 'lucide-react';
import type { VideoData, VideoPage } from '../types';
import { getCachedImageDataUrl, getImageDataUrl, prefetchImageDataUrls } from '../imageCache';

interface VideoInfoProps {
  data: VideoData | null;
  currentPage?: VideoPage | null;
  currentPageIndex?: number;
  onPrevPage?: () => void;
  onNextPage?: () => void;
  onSelectPage?: (page: VideoPage) => void;
}

export function VideoInfo({
  data,
  currentPage,
  currentPageIndex = 0,
  onPrevPage,
  onNextPage,
  onSelectPage,
}: VideoInfoProps) {
  const [thumbnailSrc, setThumbnailSrc] = useState('');
  const [thumbnailFailed, setThumbnailFailed] = useState(false);
  const [authorAvatarSrc, setAuthorAvatarSrc] = useState('');
  const [authorAvatarFailed, setAuthorAvatarFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setThumbnailFailed(false);

    const thumbnailUrl = currentPage?.thumbnail || data?.thumbnail || '';
    if (!thumbnailUrl) {
      setThumbnailSrc('');
      return;
    }

    const cached = getCachedImageDataUrl(thumbnailUrl);
    if (cached) {
      setThumbnailSrc(cached);
      return;
    }

    setThumbnailSrc(thumbnailUrl);

    getImageDataUrl(thumbnailUrl)
      .then((src) => {
        if (!cancelled) setThumbnailSrc(src);
      })
      .catch(() => {
        if (!cancelled) {
          setThumbnailSrc(thumbnailUrl);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [data?.thumbnail, currentPage?.thumbnail]);

  useEffect(() => {
    const pages = data?.pages ?? [];
    prefetchImageDataUrls([
      pages[currentPageIndex - 1]?.thumbnail,
      pages[currentPageIndex]?.thumbnail,
      pages[currentPageIndex + 1]?.thumbnail,
      data?.thumbnail,
    ], 4);
  }, [data?.id, data?.thumbnail, data?.pages, currentPageIndex]);

  useEffect(() => {
    let cancelled = false;
    setAuthorAvatarFailed(false);

    if (!data?.author_avatar) return;
    const cached = getCachedImageDataUrl(data.author_avatar);
    if (cached) {
      setAuthorAvatarSrc(cached);
      return;
    }

    setAuthorAvatarSrc(data.author_avatar);

    getImageDataUrl(data.author_avatar)
      .then((src) => {
        if (!cancelled) setAuthorAvatarSrc(src);
      })
      .catch(() => {
        if (!cancelled) {
          setAuthorAvatarSrc(data.author_avatar);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [data?.author_avatar]);

  if (!data) return null;
  const pages = data.pages ?? [];
  const hasMultiplePages = pages.length > 1;
  const displayPage = currentPage ?? pages[0] ?? null;
  const displayDuration = displayPage?.duration || data.duration;

  return (
    <div className="glass-panel rounded-[1.75rem] p-4 md:p-5 flex flex-col gap-4">
      <div className="relative mx-auto w-full max-w-[640px] px-10 md:px-14">
        {hasMultiplePages && (
          <button
            type="button"
            onClick={onPrevPage}
            className="motion-button absolute left-0 top-1/2 z-10 flex h-11 w-11 -translate-y-1/2 items-center justify-center rounded-full border border-white/90 bg-white/84 text-bili-pink shadow-[0_12px_28px_rgba(255,143,179,0.18)] hover:bg-pink-50 hover:shadow-[0_16px_36px_rgba(255,143,179,0.28)]"
            aria-label="上一个分P"
          >
            <ChevronLeft size={24} strokeWidth={2.8} />
          </button>
        )}
        <div className="w-full aspect-video rounded-[1.2rem] overflow-hidden relative shadow-[0_14px_34px_rgba(20,32,70,0.09)] bg-gray-100">
        {thumbnailSrc && !thumbnailFailed ? (
          <img
            src={thumbnailSrc}
            alt="视频封面"
            className="w-full h-full object-cover"
            referrerPolicy="no-referrer"
            onError={() => setThumbnailFailed(true)}
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-gray-300 text-lg font-bold">暂无封面</div>
        )}
        <div className="absolute bottom-3 right-3 bg-black/70 backdrop-blur-md text-white text-xs font-bold font-mono px-3 py-1.5 rounded-lg border border-white/10 shadow-sm flex items-center gap-1.5">
          <Clock size={12} />
          {displayDuration}
        </div>
        {hasMultiplePages && displayPage && (
          <div className="absolute left-3 top-3 rounded-xl border border-white/15 bg-black/58 px-3 py-1.5 text-xs font-black text-white shadow-sm backdrop-blur-md">
            P{displayPage.page} / {pages.length}
          </div>
        )}
        </div>
        {hasMultiplePages && (
          <button
            type="button"
            onClick={onNextPage}
            className="motion-button absolute right-0 top-1/2 z-10 flex h-11 w-11 -translate-y-1/2 items-center justify-center rounded-full border border-white/90 bg-white/84 text-bili-pink shadow-[0_12px_28px_rgba(255,143,179,0.18)] hover:bg-pink-50 hover:shadow-[0_16px_36px_rgba(255,143,179,0.28)]"
            aria-label="下一个分P"
          >
            <ChevronRight size={24} strokeWidth={2.8} />
          </button>
        )}
      </div>

      <div className="flex flex-col gap-3">
        <h2 className="text-[20px] 2xl:text-[24px] font-black text-gray-900 leading-snug">
          {displayPage && hasMultiplePages ? displayPage.part : data.title}
        </h2>
        {displayPage && hasMultiplePages && (
          <p className="text-sm font-bold text-gray-500">
            {data.title}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-2">
          {data.is_stein && (
            <span className="inline-flex items-center gap-1 px-3 py-1 bg-violet-50 text-violet-700 rounded-full text-xs font-bold border border-violet-100">
              <Layers size={12} />
              互动视频
              {data.stein_graph?.segment_count
                ? ` · ${data.stein_graph.segment_count} 分片`
                : ''}
            </span>
          )}
          {data.is_charging && (
            <span className="inline-flex items-center gap-1 px-3 py-1 bg-orange-50 text-orange-600 rounded-full text-xs font-bold border border-orange-100">
              <AlertTriangle size={12} />
              充电专属
            </span>
          )}
          {data.is_preview && (
            <span className="inline-flex items-center gap-1 px-3 py-1 bg-red-50 text-red-600 rounded-full text-xs font-bold border border-red-100">
              <AlertTriangle size={12} />
              仅试看
            </span>
          )}
          {data.is_vip && (
            <span className="inline-flex items-center gap-1 px-3 py-1 bg-pink-50 text-bili-pink rounded-full text-xs font-bold border border-pink-100">
              <Crown size={12} />
              {data.vip_type === 2 ? '年度大会员' : '大会员'}
            </span>
          )}
          {!data.is_charging && !data.is_vip && !data.is_stein && (
            <span className="inline-flex items-center gap-1 px-3 py-1 bg-green-50 text-green-600 rounded-full text-xs font-bold border border-green-100">
              公开视频
            </span>
          )}
        </div>

        {(data.is_preview || data.access_message) && (
          <div className="flex items-start gap-2 text-xs text-orange-600 bg-orange-50 rounded-xl px-3 py-2.5 border border-orange-100 leading-relaxed">
            <AlertTriangle size={14} className="mt-0.5 shrink-0" />
            <span>
              {data.access_message ||
                '当前仅能获取充电试看流。请登录并对该 UP 开通对应档位包月充电后再下载完整版。'}
            </span>
          </div>
        )}

        <div className="flex flex-wrap items-center gap-3 text-sm text-gray-500">
          <div className="flex items-center gap-1.5 font-medium">
            {authorAvatarSrc && !authorAvatarFailed ? (
              <img
                src={authorAvatarSrc}
                alt={`${data.author} 的头像`}
                className="h-7 w-7 rounded-full border border-white object-cover shadow-sm"
                referrerPolicy="no-referrer"
                onError={() => setAuthorAvatarFailed(true)}
              />
            ) : (
              <div className="w-7 h-7 rounded-full bg-gradient-to-br from-bili-pink to-bili-pink-hover flex items-center justify-center text-white text-[10px] font-bold">
                {data.author.charAt(0)}
              </div>
            )}
            {data.author}
          </div>
          <div className="flex items-center gap-1.5">
            <Eye size={14} />
            {data.views} 次播放
          </div>
          <div className="flex items-center gap-1.5">
            <Layers size={14} />
            {data.pages?.length || 1} 个分P
          </div>
        </div>

        {hasMultiplePages && (
          <details className="text-sm">
            <summary className="font-bold text-gray-600 cursor-pointer hover:text-bili-pink transition-colors">
              查看分P列表 ({pages.length})
            </summary>
            <div className="mt-2 flex flex-col gap-1 max-h-32 overflow-y-auto pr-2 custom-scrollbar">
              {pages.map((p, index) => (
                <button
                  key={p.cid}
                  type="button"
                  onClick={() => onSelectPage?.(p)}
                  className={`motion-button flex items-center gap-2 px-3 py-1.5 rounded-lg text-left text-xs ${
                    index === currentPageIndex
                      ? 'bg-pink-50 text-bili-pink'
                      : 'bg-white/40 text-gray-600 hover:bg-white/72 hover:text-bili-pink'
                  }`}
                >
                  <span className="font-bold text-bili-pink">P{p.page}</span>
                  <span className="truncate">{p.part}</span>
                </button>
              ))}
            </div>
          </details>
        )}

        {data.desc && (
          <p className="text-sm text-gray-500 leading-relaxed line-clamp-2">
            {data.desc}
          </p>
        )}

        {data.is_login && (
          <div className="flex items-center gap-2 text-xs text-gray-400 border-t border-gray-100 pt-3 mt-1">
            <span className="w-2 h-2 rounded-full bg-green-400" />
            {data.login_name}（等级 {data.login_level}）
          </div>
        )}
      </div>
    </div>
  );
}
