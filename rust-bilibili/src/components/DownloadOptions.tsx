import React from 'react';
import { SlidersHorizontal, Film, Music, Download, AlertTriangle } from 'lucide-react';
import type { VideoData } from '../types';

interface DownloadOptionsProps {
  data: VideoData | null;
  selectedQuality: string;
  onSelectQuality: (q: string) => void;
  onDownload: () => void;
  format: 'video' | 'audio';
  onFormatChange: (f: 'video' | 'audio') => void;
  ffmpegAvailable?: boolean;
}

export function DownloadOptions({
  data, selectedQuality, onSelectQuality, onDownload,
  format, onFormatChange, ffmpegAvailable = true,
}: DownloadOptionsProps) {
  if (!data) return null;

  const audioQualities = ['320kbps 高品质', '192kbps 标准', '128kbps 基础'];
  const qualities = format === 'video' ? data.qualities : audioQualities;

  const handleFormatSwitch = (f: 'video' | 'audio') => {
    onFormatChange(f);
    if (f === 'audio') {
      onSelectQuality(audioQualities[0]);
    } else if (data.qualities.length > 0) {
      onSelectQuality(data.qualities[0]);
    }
  };

  return (
    <div className="glass-panel rounded-[2rem] p-8 flex flex-col h-full shrink-0">
      <div className="flex items-center gap-3 mb-6">
        <div className="text-bili-pink flex items-center justify-center">
          <SlidersHorizontal size={24} strokeWidth={2.5} />
        </div>
        <h3 className="text-xl font-black text-gray-900">下载设置</h3>
      </div>

      <div className="mb-5">
        <label className="block text-sm font-black text-gray-600 mb-3">格式</label>
        <div className="flex gap-3">
          <button
            onClick={() => handleFormatSwitch('video')}
            className={`flex-1 py-4 rounded-2xl flex flex-col items-center justify-center gap-2 font-bold transition-all border-2 ${
              format === 'video'
                ? 'border-bili-pink text-bili-pink bg-pink-50/50 shadow-sm'
                : 'border-white bg-white/50 text-gray-600 hover:border-pink-200'
            }`}
          >
            <Film size={24} />
            视频 (MP4)
          </button>
          <button
            onClick={() => handleFormatSwitch('audio')}
            className={`flex-1 py-4 rounded-2xl flex flex-col items-center justify-center gap-2 font-bold transition-all border-2 ${
              format === 'audio'
                ? 'border-bili-pink text-bili-pink bg-pink-50/50 shadow-sm'
                : 'border-white bg-white/50 text-gray-600 hover:border-pink-200'
            }`}
          >
            <Music size={24} />
            音频 (MP3)
          </button>
        </div>
      </div>

      <div className="flex-1">
        <label className="block text-sm font-black text-gray-600 mb-3">
          {format === 'video' ? '画质' : '音质'}
        </label>
        <div className="flex flex-col gap-2.5">
          {qualities.map((q, i) => {
            const isSelected = selectedQuality === q;
            const isPremium = q.includes('4K') || q.includes('HDR') || q.includes('320kbps');
            const fileSize = format === 'video'
              ? (q.includes('4K') ? '~850 MB' : q.includes('1080P60') ? '~450 MB' : q.includes('1080P') ? '~245 MB' : q.includes('720P') ? '~120 MB' : '~50 MB')
              : (q.includes('320kbps') ? '~12 MB' : q.includes('192kbps') ? '~8 MB' : '~5 MB');

            return (
              <label
                key={q}
                className={`group flex items-center justify-between p-4 rounded-2xl border-2 cursor-pointer transition-all ${
                  isSelected ? 'border-bili-pink bg-pink-50/30' : 'border-white bg-white/40 hover:border-pink-200'
                }`}
              >
                <div className="flex items-center gap-4">
                  <div className={`w-5 h-5 rounded-full border-2 flex items-center justify-center transition-all ${
                    isSelected ? 'border-bili-pink' : 'border-gray-300'
                  }`}>
                    {isSelected && <div className="w-2.5 h-2.5 bg-bili-pink rounded-full"></div>}
                  </div>
                  <span className="font-bold text-gray-800 flex items-center gap-2">
                    {q}
                    {isPremium && <span className="text-[10px] bg-yellow-100 text-yellow-600 px-2 py-0.5 rounded-full">高级</span>}
                  </span>
                </div>
                <div className={`text-xs font-bold px-3 py-1.5 rounded-lg ${
                  isSelected ? 'bg-pink-100/50 text-pink-600' : 'bg-gray-100/50 text-gray-500'
                }`}>
                  {fileSize}
                </div>
                <input type="radio" name="quality" className="hidden" checked={isSelected} onChange={() => onSelectQuality(q)} />
              </label>
            );
          })}
        </div>
      </div>

      {format === 'audio' && !ffmpegAvailable && (
        <div className="mt-4 flex items-center gap-2 text-xs text-orange-500 bg-orange-50 rounded-xl px-4 py-2.5 border border-orange-100">
          <AlertTriangle size={14} />
          音频格式需要 FFmpeg 支持
        </div>
      )}

      {data.is_charging && (
        <div className="mt-3 flex items-center gap-2 text-xs text-orange-500 bg-orange-50 rounded-xl px-4 py-2.5 border border-orange-100">
          <AlertTriangle size={14} />
          {data.is_vip ? '大会员可下载完整版' : '充电专属视频，需开通大会员'}
        </div>
      )}

      <button
        onClick={onDownload}
        className="w-full mt-6 bg-gradient-to-r from-[#fb7299] to-[#ff85a8] text-white py-4 rounded-2xl font-bold flex items-center justify-center gap-2 transition-transform hover:scale-[1.02] shadow-lg shadow-pink-300/40 active:scale-[0.98]"
      >
        <Download size={20} strokeWidth={2.5} />
        开始下载
      </button>
    </div>
  );
}
