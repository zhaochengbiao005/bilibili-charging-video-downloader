import React, { useEffect, useState } from 'react';
import { User, Eye, Clock, Layers, AlertTriangle, Crown } from 'lucide-react';
import type { VideoData } from '../types';
import * as Bridge from '../bridge';

interface VideoInfoProps {
  data: VideoData | null;
}

export function VideoInfo({ data }: VideoInfoProps) {
  const [thumbnailSrc, setThumbnailSrc] = useState('');
  const [thumbnailFailed, setThumbnailFailed] = useState(false);
  const [authorAvatarSrc, setAuthorAvatarSrc] = useState('');
  const [authorAvatarFailed, setAuthorAvatarFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setThumbnailFailed(false);
    setThumbnailSrc('');

    if (!data?.thumbnail) return;

    Bridge.fetchImageDataUrl(data.thumbnail)
      .then((src) => {
        if (!cancelled) setThumbnailSrc(src);
      })
      .catch(() => {
        if (!cancelled) {
          setThumbnailSrc(data.thumbnail);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [data?.thumbnail]);

  useEffect(() => {
    let cancelled = false;
    setAuthorAvatarFailed(false);
    setAuthorAvatarSrc('');

    if (!data?.author_avatar) return;

    Bridge.fetchImageDataUrl(data.author_avatar)
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

  return (
    <div className="glass-panel rounded-[1.75rem] p-4 md:p-5 flex flex-col gap-4">
      <div className="w-full max-w-[540px] mx-auto aspect-video rounded-[1.2rem] overflow-hidden relative shadow-[0_14px_34px_rgba(20,32,70,0.09)] bg-gray-100">
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
          {data.duration}
        </div>
      </div>

      <div className="flex flex-col gap-3">
        <h2 className="text-[20px] 2xl:text-[24px] font-black text-gray-900 leading-snug">
          {data.title}
        </h2>

        <div className="flex flex-wrap items-center gap-2">
          {data.is_charging && (
            <span className="inline-flex items-center gap-1 px-3 py-1 bg-orange-50 text-orange-600 rounded-full text-xs font-bold border border-orange-100">
              <AlertTriangle size={12} />
              充电专属
            </span>
          )}
          {data.is_vip && (
            <span className="inline-flex items-center gap-1 px-3 py-1 bg-pink-50 text-bili-pink rounded-full text-xs font-bold border border-pink-100">
              <Crown size={12} />
              {data.vip_type === 2 ? '年度大会员' : '大会员'}
            </span>
          )}
          {!data.is_charging && !data.is_vip && (
            <span className="inline-flex items-center gap-1 px-3 py-1 bg-green-50 text-green-600 rounded-full text-xs font-bold border border-green-100">
              公开视频
            </span>
          )}
        </div>

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

        {data.pages && data.pages.length > 1 && (
          <details className="text-sm">
            <summary className="font-bold text-gray-600 cursor-pointer hover:text-bili-pink transition-colors">
              查看分P列表 ({data.pages.length})
            </summary>
            <div className="mt-2 flex flex-col gap-1 max-h-32 overflow-y-auto pr-2">
              {data.pages.map((p) => (
                <div key={p.cid} className="flex items-center gap-2 px-3 py-1.5 bg-white/40 rounded-lg text-xs text-gray-600">
                  <span className="font-bold text-bili-pink">P{p.page}</span>
                  <span className="truncate">{p.part}</span>
                </div>
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
