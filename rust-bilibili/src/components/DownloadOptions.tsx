import React, { useEffect, useMemo } from 'react';
import {
  SlidersHorizontal, Film, Music, Download, AlertTriangle, Cpu,
  MessageSquareText, Flame, Cloud, HardDrive, GitBranch,
} from 'lucide-react';
import type {
  CloudSaveMode, DanmakuMode, SteinChoice, SteinDownloadMode, SteinNode, VideoData,
} from '../types';

interface DownloadOptionsProps {
  data: VideoData | null;
  selectedQuality: string;
  onSelectQuality: (q: string) => void;
  onDownload: () => void;
  batchCount?: number;
  currentPageLabel?: string;
  onDownloadAll?: () => void;
  isBatchStarting?: boolean;
  saveMode: CloudSaveMode;
  onSaveModeChange: (mode: CloudSaveMode) => void;
  cloudAuthorized?: boolean;
  cloudRemoteDir?: string;
  format: 'video' | 'audio';
  onFormatChange: (f: 'video' | 'audio') => void;
  threads: number;
  onThreadsChange: (threads: number) => void;
  danmakuMode: DanmakuMode;
  onDanmakuModeChange: (mode: DanmakuMode) => void;
  ffmpegAvailable?: boolean;
  isLoadingSizes?: boolean;
  steinMode?: SteinDownloadMode;
  onSteinModeChange?: (mode: SteinDownloadMode) => void;
  steinPathEdges?: number[];
  onSteinPathEdgesChange?: (edges: number[]) => void;
}

export function DownloadOptions({
  data, selectedQuality, onSelectQuality, onDownload,
  batchCount = 1, currentPageLabel, onDownloadAll,
  isBatchStarting = false,
  saveMode, onSaveModeChange, cloudAuthorized = false, cloudRemoteDir = '',
  format, onFormatChange, threads, onThreadsChange,
  danmakuMode, onDanmakuModeChange, ffmpegAvailable = true, isLoadingSizes = false,
  steinMode = 'all', onSteinModeChange, steinPathEdges = [], onSteinPathEdgesChange,
}: DownloadOptionsProps) {
  if (!data) return null;

  const isStein = Boolean(data.is_stein);
  const graph = data.stein_graph;
  const audioQualities = ['320kbps 高品质', '192kbps 标准', '128kbps 基础'];
  const firstAvailableAudioQuality =
    data.audio_streams?.find((stream) => stream.available)?.label ?? audioQualities[0];
  const qualities = format === 'video' ? data.qualities : audioQualities;
  const visibleStreams = format === 'video' ? data.streams : [];
  const isHighFidelityVideo = format === 'video' && (
    selectedQuality.includes('8K') ||
    selectedQuality.includes('杜比') ||
    selectedQuality.includes('HDR')
  );
  const isCloudMode = saveMode === 'baidu_netdisk';
  const effectiveDanmakuMode = isHighFidelityVideo && danmakuMode === 'burn' ? 'ass' : danmakuMode;

  const pathSteps = useMemo(() => {
    if (!graph || steinMode !== 'path') {
      return [] as Array<{ node: SteinNode; choice?: SteinChoice }>;
    }
    const steps: Array<{ node: SteinNode; choice?: SteinChoice }> = [];
    let edgeId = graph.entry_edge_id;
    const nodeMap = new Map(graph.nodes.map((n) => [n.edge_id, n]));
    for (let i = 0; i <= steinPathEdges.length; i += 1) {
      const node = nodeMap.get(edgeId);
      if (!node) break;
      const choiceEdge = steinPathEdges[i];
      const choice = choiceEdge != null
        ? node.choices.find((c) => c.edge_id === choiceEdge)
        : undefined;
      steps.push({ node, choice });
      if (choiceEdge == null) break;
      edgeId = choiceEdge;
    }
    return steps;
  }, [graph, steinMode, steinPathEdges]);

  const currentPathNode = pathSteps[pathSteps.length - 1]?.node;
  const pathComplete = Boolean(
    steinMode === 'path'
      && currentPathNode
      && (currentPathNode.is_leaf || currentPathNode.choices.length === 0),
  );

  useEffect(() => {
    if (isStein && isCloudMode) {
      onSaveModeChange('local');
    }
  }, [isStein, isCloudMode, onSaveModeChange]);

  const handleFormatSwitch = (f: 'video' | 'audio') => {
    onFormatChange(f);
    if (f === 'audio') {
      onSelectQuality(firstAvailableAudioQuality);
    } else if (data.qualities.length > 0) {
      onSelectQuality(data.qualities[0]);
    }
  };

  const downloadDisabled =
    (isCloudMode && !cloudAuthorized)
    || (isStein && steinMode === 'path' && steinPathEdges.length === 0);

  const downloadLabel = isStein
    ? steinMode === 'path'
      ? (pathComplete
        ? '下载路径并拼接'
        : steinPathEdges.length === 0
          ? '请先选择分支'
          : '下载当前路径并拼接')
      : `下载全部 ${graph?.segment_count ?? ''} 个分片`
    : isCloudMode
      ? (currentPageLabel ? `保存当前 ${currentPageLabel}` : '保存到百度网盘')
      : (currentPageLabel ? `下载当前 ${currentPageLabel}` : '开始下载');

  return (
    <div className="rounded-[1.75rem] border border-white/95 bg-white/92 p-5 shadow-[0_18px_52px_rgba(31,41,55,0.08),0_12px_34px_rgba(255,143,179,0.12)] backdrop-blur-[20px] flex flex-col shrink-0">
      <div className="flex items-center gap-3 mb-5">
        <div className="text-bili-pink flex items-center justify-center">
          <SlidersHorizontal size={24} strokeWidth={2.5} />
        </div>
        <h3 className="text-xl font-black text-gray-900">下载设置</h3>
      </div>

      {isStein && (
        <div className="mb-4 rounded-2xl border border-violet-100 bg-violet-50/70 p-3.5">
          <div className="mb-2 flex items-center gap-2 text-sm font-black text-violet-700">
            <GitBranch size={16} />
            互动视频
            {graph?.segment_count ? ` · ${graph.segment_count} 分片` : ''}
          </div>
          <div className="grid grid-cols-2 gap-2 rounded-xl bg-white/80 p-1">
            {([
              { value: 'all' as const, label: '整图下载' },
              { value: 'path' as const, label: '路径拼接' },
            ]).map((item) => (
              <button
                key={item.value}
                type="button"
                onClick={() => {
                  onSteinModeChange?.(item.value);
                  if (item.value === 'all') onSteinPathEdgesChange?.([]);
                }}
                className={`rounded-lg py-2 text-xs font-bold transition-colors ${
                  steinMode === item.value
                    ? 'bg-violet-600 text-white'
                    : 'text-violet-700 hover:bg-violet-100'
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
          {steinMode === 'all' && (
            <p className="mt-2 text-[11px] leading-relaxed text-violet-700/80">
              下载全部去重分片，并生成 play.html 离线互动播放器（浏览器打开即可像 B 站一样选分支）。
            </p>
          )}
          {steinMode === 'path' && graph && (
            <div className="mt-3 space-y-2">
              <p className="text-[11px] leading-relaxed text-violet-700/80">
                从入口依次选择分支，下载后按路径拼接成一条成片。
              </p>
              {pathSteps.map((step, idx) => (
                <div key={`${step.node.edge_id}-${idx}`} className="text-xs">
                  <div className="font-bold text-gray-700">
                    {idx === 0 ? '入口' : `第 ${idx} 步`}
                    ：
                    {step.node.title}
                    {step.choice ? ` → ${step.choice.option}` : ''}
                  </div>
                </div>
              ))}
              {currentPathNode && !pathComplete && currentPathNode.choices.length > 0 && (
                <div className="flex flex-col gap-1.5">
                  {currentPathNode.choices.map((choice) => (
                    <button
                      key={choice.edge_id}
                      type="button"
                      onClick={() => onSteinPathEdgesChange?.([...steinPathEdges, choice.edge_id])}
                      className="rounded-xl border border-violet-100 bg-white px-3 py-2 text-left text-xs font-bold text-violet-800 hover:border-violet-300 hover:bg-violet-50"
                    >
                      {choice.option}
                      {choice.cid ? ` · cid ${choice.cid}` : ''}
                    </button>
                  ))}
                </div>
              )}
              {steinPathEdges.length > 0 && (
                <button
                  type="button"
                  onClick={() => onSteinPathEdgesChange?.([])}
                  className="text-[11px] font-bold text-violet-600 underline"
                >
                  重选路径
                </button>
              )}
            </div>
          )}
        </div>
      )}

      <div className="mb-4">
        <label className="block text-sm font-black text-gray-600 mb-3">保存位置</label>
        <div className="grid grid-cols-2 gap-2 rounded-2xl bg-white/82 border border-pink-50 p-1.5">
          {[
            { value: 'local' as const, label: '本地', icon: HardDrive },
            { value: 'baidu_netdisk' as const, label: '百度网盘', icon: Cloud, disabled: isStein },
          ].map((item) => {
            const Icon = item.icon;
            const active = saveMode === item.value;
            const disabled = Boolean(item.disabled);
            return (
              <button
                key={item.value}
                type="button"
                disabled={disabled}
                onClick={() => !disabled && onSaveModeChange(item.value)}
                className={`motion-button flex items-center justify-center gap-2 rounded-xl py-2.5 text-sm font-bold transition-all ${
                  active
                    ? 'bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white shadow-md'
                    : disabled
                      ? 'cursor-not-allowed text-gray-300'
                      : 'text-gray-600 hover:bg-pink-50/80'
                }`}
              >
                <Icon size={16} />
                {item.label}
              </button>
            );
          })}
        </div>
        {isStein && (
          <p className="mt-2 text-[11px] text-gray-400">互动视频暂仅支持本地下载</p>
        )}
        {isCloudMode && cloudRemoteDir && (
          <p className="mt-2 text-[11px] text-gray-400 truncate">
            网盘目录：
            {cloudRemoteDir}
          </p>
        )}
      </div>

      <div className="mb-4">
        <label className="block text-sm font-black text-gray-600 mb-3">格式</label>
        <div className="grid grid-cols-2 gap-2 rounded-2xl bg-white/82 border border-pink-50 p-1.5">
          {[
            { value: 'video' as const, label: '视频 MP4', icon: Film },
            { value: 'audio' as const, label: '音频 MP3', icon: Music, disabled: isStein },
          ].map((item) => {
            const Icon = item.icon;
            const active = format === item.value;
            const disabled = Boolean(item.disabled);
            return (
              <button
                key={item.value}
                type="button"
                disabled={disabled}
                onClick={() => !disabled && handleFormatSwitch(item.value)}
                className={`motion-button flex items-center justify-center gap-2 rounded-xl py-2.5 text-sm font-bold transition-all ${
                  active
                    ? 'bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white shadow-md'
                    : disabled
                      ? 'cursor-not-allowed text-gray-300'
                      : 'text-gray-600 hover:bg-pink-50/80'
                }`}
              >
                <Icon size={16} />
                {item.label}
              </button>
            );
          })}
        </div>
      </div>

      <div className="mb-4">
        <label className="block text-sm font-black text-gray-600 mb-3">画质 / 音质</label>
        <div className="flex flex-col gap-2 max-h-48 overflow-y-auto pr-1 custom-scrollbar">
          {qualities.map((q) => {
            const stream = visibleStreams.find((s) => s.label === q);
            const isSelected = selectedQuality === q;
            const isPremium = q.includes('4K') || q.includes('HDR') || q.includes('杜比') || q.includes('8K');
            const isDisabled = stream ? !stream.available && Boolean(stream.unavailable_reason) : false;
            const unavailableReason = stream?.unavailable_reason;
            const fileSize = format === 'video'
              ? formatFileSize(stream?.size_bytes, isLoadingSizes)
              : formatFileSize(
                data.audio_streams?.find((a) => a.label === q)?.size_bytes,
                isLoadingSizes,
              );
            return (
              <label
                key={q}
                className={`motion-button flex items-center justify-between rounded-2xl border px-4 py-3 transition-all ${
                  isDisabled
                    ? 'cursor-not-allowed opacity-50 border-gray-100 bg-gray-50'
                    : isSelected
                      ? 'cursor-pointer border-bili-pink bg-pink-50/70 shadow-sm'
                      : 'cursor-pointer border-white bg-white/82 hover:border-pink-200'
                }`}
              >
                <div className="flex items-center gap-4">
                  <div className={`w-5 h-5 rounded-full border-2 flex items-center justify-center transition-all ${
                    isSelected ? 'border-bili-pink' : 'border-gray-300'
                  }`}
                  >
                    {isSelected && <div className="w-2.5 h-2.5 bg-bili-pink rounded-full" />}
                  </div>
                  <span className="font-bold text-gray-800 flex items-center gap-2">
                    {q}
                    {isPremium && <span className="text-[10px] bg-yellow-100 text-yellow-600 px-2 py-0.5 rounded-full">高级</span>}
                    {unavailableReason && <span className="text-[10px] bg-orange-100 text-orange-600 px-2 py-0.5 rounded-full">{unavailableReason}</span>}
                  </span>
                </div>
                <div className={`text-xs font-bold px-3 py-1.5 rounded-lg ${
                  isSelected ? 'bg-pink-100/50 text-pink-600' : 'bg-gray-100/50 text-gray-500'
                }`}
                >
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

      <div className="mb-4 grid grid-cols-2 gap-3">
        <div>
          <label className="mb-2 flex items-center gap-1.5 text-sm font-black text-gray-600">
            <Cpu size={14} />
            {' '}
            线程
          </label>
          <select
            value={threads}
            onChange={(e) => onThreadsChange(Number(e.target.value))}
            className="w-full rounded-xl border border-pink-50 bg-white/90 px-3 py-2.5 text-sm font-bold text-gray-700 outline-none focus:ring-2 focus:ring-bili-pink/20"
          >
            {[2, 4, 8, 12, 16, 24, 32].map((n) => (
              <option key={n} value={n}>{n}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="mb-2 flex items-center gap-1.5 text-sm font-black text-gray-600">
            <MessageSquareText size={14} />
            {' '}
            弹幕
          </label>
          <select
            value={effectiveDanmakuMode}
            onChange={(e) => onDanmakuModeChange(e.target.value as DanmakuMode)}
            disabled={format !== 'video' || isStein}
            className="w-full rounded-xl border border-pink-50 bg-white/90 px-3 py-2.5 text-sm font-bold text-gray-700 outline-none focus:ring-2 focus:ring-bili-pink/20 disabled:opacity-50"
          >
            <option value="none">关闭</option>
            <option value="ass">外挂 ASS</option>
            <option value="burn">烧录</option>
          </select>
        </div>
      </div>

      {format === 'audio' && !ffmpegAvailable && (
        <div className="mt-1 flex items-center gap-2 text-xs text-orange-500 bg-orange-50 rounded-xl px-4 py-2.5 border border-orange-100">
          <AlertTriangle size={14} />
          音频格式需要 FFmpeg 支持
        </div>
      )}

      {format === 'video' && !ffmpegAvailable && (
        <div className="mt-1 flex items-center gap-2 text-xs text-orange-500 bg-orange-50 rounded-xl px-4 py-2.5 border border-orange-100">
          <AlertTriangle size={14} />
          MP4 合并和弹幕烧录需要 FFmpeg，未检测到时下载会提示安装
        </div>
      )}

      {data.is_charging && (
        <div className="mt-3 flex items-start gap-2 text-xs text-orange-500 bg-orange-50 rounded-xl px-4 py-2.5 border border-orange-100 leading-relaxed">
          <AlertTriangle size={14} className="mt-0.5 shrink-0" />
          <span>
            {data.is_preview || data.access_message
              ? data.access_message
                || '当前仅试看流。请登录并对该 UP 开通对应档位包月充电后再下载完整版。'
              : data.is_upower_play
                ? '充电专属视频：当前账号已具备完整播放权限'
                : '充电专属视频：需对该 UP 开通对应档位包月充电（不是大会员）'}
          </span>
        </div>
      )}

      {isStein && data.access_message && !data.is_charging && (
        <div className="mt-3 flex items-start gap-2 text-xs text-violet-600 bg-violet-50 rounded-xl px-4 py-2.5 border border-violet-100 leading-relaxed">
          <Flame size={14} className="mt-0.5 shrink-0" />
          <span>{data.access_message}</span>
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
        disabled={downloadDisabled}
        className="motion-button w-full mt-5 bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white py-3.5 rounded-2xl font-bold flex items-center justify-center gap-2 shadow-[0_14px_30px_rgba(255,143,179,0.32)] disabled:cursor-not-allowed disabled:opacity-55"
      >
        <Download size={20} strokeWidth={2.5} />
        {downloadLabel}
      </button>
      {onDownloadAll && batchCount > 1 && !isStein && (
        <button
          type="button"
          onClick={onDownloadAll}
          disabled={isBatchStarting || (isCloudMode && !cloudAuthorized)}
          className="motion-button mt-3 flex w-full items-center justify-center gap-2 rounded-2xl border border-pink-100 bg-white/84 py-3 text-sm font-black text-bili-pink shadow-sm hover:bg-pink-50/80 disabled:cursor-not-allowed disabled:opacity-55"
        >
          <Download size={18} strokeWidth={2.5} />
          {isBatchStarting
            ? '正在创建队列'
            : isCloudMode ? `全部保存 ${batchCount} 个视频` : `下载全部 ${batchCount} 个视频`}
        </button>
      )}
    </div>
  );
}

function formatFileSize(sizeBytes?: number | null, isLoading = false): string {
  if (!sizeBytes || !Number.isFinite(sizeBytes) || sizeBytes <= 0) {
    return isLoading ? '获取中...' : '大小未知';
  }
  const mib = sizeBytes / 1024 / 1024;
  if (mib < 100) return `${mib.toFixed(1)} MB`;
  return `${Math.round(mib)} MB`;
}