import React from 'react';
import { User, Eye, Clock, Layers, AlertTriangle, Crown } from 'lucide-react';
import type { VideoData } from '../types';

interface VideoInfoProps {
  data: VideoData | null;
}

export function VideoInfo({ data }: VideoInfoProps) {
  if (!data) return null;

  return (
    <div className="glass-panel rounded-[2rem] p-6 lg:p-8 flex flex-col gap-6">
      {/* 缩略图 */}
      <div className="w-full aspect-video rounded-2xl overflow-hidden relative shadow-inner bg-gray-100">
        {data.thumbnail ? (
          <img src={data.thumbnail} alt="Video thumbnail" className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-gray-300 text-lg font-bold">
            暂无缩略图
          </div>
        )}
        <div className="absolute bottom-4 right-4 bg-black/70 backdrop-blur-md text-white text-xs font-bold font-mono px-3 py-1.5 rounded-lg border border-white/10 shadow-sm flex items-center gap-1.5">
          <Clock size={12} />
          {data.duration}
        </div>
      </div>

      {/* 信息详情 */}
      <div className="flex flex-col gap-4">
        {/* 标题 */}
        <h2 className="text-2xl font-black text-gray-900 leading-snug">
          {data.title}
        </h2>

        {/* 状态标签 */}
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

        {/* 元数据行 */}
        <div className="flex flex-wrap items-center gap-4 text-sm text-gray-500">
          <div className="flex items-center gap-1.5 font-medium">
            <div className="w-7 h-7 rounded-full bg-gradient-to-br from-bili-pink to-pink-300 flex items-center justify-center text-white text-[10px] font-bold">
              {data.author.charAt(0)}
            </div>
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

        {/* 分P列表 */}
        {data.pages && data.pages.length > 1 && (
          <details className="text-sm">
            <summary className="font-bold text-gray-600 cursor-pointer hover:text-bili-pink transition-colors">
              查看分P列表 ({data.pages.length})
            </summary>
            <div className="mt-2 flex flex-col gap-1 max-h-32 overflow-y-auto pr-2">
              {data.pages.map((p, i) => (
                <div key={p.cid} className="flex items-center gap-2 px-3 py-1.5 bg-white/40 rounded-lg text-xs text-gray-600">
                  <span className="font-bold text-bili-pink">P{p.page}</span>
                  <span className="truncate">{p.part}</span>
                </div>
              ))}
            </div>
          </details>
        )}

        {/* 简介 */}
        {data.desc && (
          <p className="text-sm text-gray-500 leading-relaxed line-clamp-3">
            {data.desc}
          </p>
        )}

        {/* 登录状态 */}
        {data.is_login && (
          <div className="flex items-center gap-2 text-xs text-gray-400 border-t border-gray-100 pt-3 mt-1">
            <span className="w-2 h-2 rounded-full bg-green-400" />
            {data.login_name} (Lv.{data.login_level})
          </div>
        )}
      </div>
    </div>
  );
}
