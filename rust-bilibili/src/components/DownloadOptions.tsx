import React from 'react';
import { SlidersHorizontal, Film, Music, Download, AlertTriangle, Cpu } from 'lucide-react';
import type { VideoData } from '../types';

interface DownloadOptionsProps {
  data: VideoData | null;
  selectedQuality: string;
  onSelectQuality: (q: string) => void;
  onDownload: () => void;
  format: 'video' | 'audio';
  onFormatChange: (f: 'video' | 'audio') => void;
  threads: number;
  onThreadsChange: (threads: number) => void;
  ffmpegAvailable?: boolean;
}

export function DownloadOptions({
  data, selectedQuality, onSelectQuality, onDownload,
  format, onFormatChange, threads, onThreadsChange, ffmpegAvailable = true,
}: DownloadOptionsProps) {
  if (!data) return null;

  const audioQualities = ['320kbps 高品质', '192kbps 标准', '128kbps 基础'];
  const firstAvailableAudioQuality =
    data.audio_streams?.find((stream) => stream.available)?.label ?? audioQualities[0];
  const qualities = format === 'video' ? data.qualities : audioQualities;
  const visibleStreams = format === 'video'
    ? data.streams
    : [];

  const handleFormatSwitch = (f: 'video' | 'audio') => {
    onFormatChange(f);
    if (f === 'audio') {
      onSelectQuality(firstAvailableAudioQuality);
    } else if (data.qualities.length > 0) {
      onSelectQuality(data.qualities[0]);
    }
  };

  return (
    <div className="glass-panel rounded-[2rem] p-7 2xl:p-8 flex flex-col h-full shrink-0">
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

      <div className="mb-5">
        <label className="flex items-center gap-2 text-sm font-black text-gray-600 mb-3">
          <Cpu size={16} />
          下载线程
        </label>
        <div className="grid grid-cols-4 gap-2 rounded-2xl bg-white/35 border border-white/70 p-1.5">
          {[4, 8, 16, 32].map((value) => (
            <button
              key={value}
              onClick={() => onThreadsChange(value)}
              className={`min-h-11 rounded-xl text-sm font-black transition-all ${
                threads === value
                  ? 'bg-white text-bili-pink shadow-sm border border-pink-100'
                  : 'text-gray-500 hover:text-bili-pink hover:bg-white/50 border border-transparent'
              }`}
            >
              {value}
            </button>
          ))}
        </div>
      </div>

      <div className="flex-1">
        <label className="block text-sm font-black text-gray-600 mb-3">
          {format === 'video' ? '画质' : '音质'}
        </label>
        <div className="flex flex-col gap-2.5">
          {qualities.map((q) => {
            const isSelected = selectedQuality === q;
            const isPremium = format === 'video' && (q.includes('4K') || q.includes('HDR'));
            const stream = visibleStreams.find((item) => item.label === q);
            const needsLogin = Boolean(stream?.requires_login && !data.is_login);
            const audioStream = data.audio_streams?.find((item) => item.label === q);
            const unavailableReason = needsLogin
              ? '需登录'
              : format === 'audio' && audioStream?.available === false
                ? '无此音质'
                : stream?.unavailable_reason;
            const isDisabled = Boolean(unavailableReason);
            const fileSize = format === 'video'
              ? formatFileSize(stream?.size_bytes)
              : formatFileSize(audioStream?.size_bytes);

            return (
              <label
                key={q}
                className={`group flex items-center justify-between p-4 rounded-2xl border-2 transition-all ${
                  isDisabled
                    ? 'cursor-not-allowed border-white/60 bg-white/25 opacity-60'
                    : isSelected
                      ? 'cursor-pointer border-bili-pink bg-pink-50/30'
                      : 'cursor-pointer border-white bg-white/40 hover:border-pink-200'
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
                    {unavailableReason && <span className="text-[10px] bg-orange-100 text-orange-600 px-2 py-0.5 rounded-full">{unavailableReason}</span>}
                  </span>
                </div>
                <div className={`text-xs font-bold px-3 py-1.5 rounded-lg ${
                  isSelected ? 'bg-pink-100/50 text-pink-600' : 'bg-gray-100/50 text-gray-500'
                }`}>
                  {fileSize}
                </div>
                <input
                  type="radio"
                  name="quality"
                  className="hidden"
                  checked={isSelected}
                  disabled={isDisabled}
                  onChange={() => {
                    if (!isDisabled) onSelectQuality(q);
                  }}
                />
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

      {format === 'video' && !ffmpegAvailable && (
        <div className="mt-4 flex items-center gap-2 text-xs text-orange-500 bg-orange-50 rounded-xl px-4 py-2.5 border border-orange-100">
          <AlertTriangle size={14} />
          MP4 合并需要 FFmpeg，未检测到时下载会提示安装
        </div>
      )}

      {data.is_charging && (
        <div className="mt-3 flex items-center gap-2 text-xs text-orange-500 bg-orange-50 rounded-xl px-4 py-2.5 border border-orange-100">
          <AlertTriangle size={14} />
          {data.is_vip ? '大会员可下载完整版' : '充电专属视频，需开通大会员'}
        </div>
      )}

      {data.streams.some((stream) => stream.requires_login) && !data.is_login && (
        <div className="mt-3 flex items-center gap-2 text-xs text-orange-500 bg-orange-50 rounded-xl px-4 py-2.5 border border-orange-100">
          <AlertTriangle size={14} />
          部分高清画质需要登录后才能下载
        </div>
      )}

      <button
        onClick={onDownload}
        className="w-full mt-6 bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white py-4 rounded-2xl font-bold flex items-center justify-center gap-2 transition-transform hover:scale-[1.02] shadow-[0_14px_30px_rgba(255,143,179,0.32)] active:scale-[0.98]"
      >
        <Download size={20} strokeWidth={2.5} />
        开始下载
      </button>
    </div>
  );
}

function formatFileSize(sizeBytes?: number | null): string {
  if (!sizeBytes || !Number.isFinite(sizeBytes) || sizeBytes <= 0) return '大小未知';
  const mib = sizeBytes / 1024 / 1024;
  if (mib < 100) return `${mib.toFixed(1)} MB`;
  return `${Math.round(mib)} MB`;
}
