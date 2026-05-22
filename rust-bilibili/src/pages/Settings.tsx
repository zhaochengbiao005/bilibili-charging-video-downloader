import React, { useState, useEffect } from 'react';
import { Database, FolderOpen, HardDrive, Wrench } from 'lucide-react';
import type { AppConfig } from '../types';
import * as Bridge from '../bridge';
import type { FfmpegStatus } from '../types';

export function Settings() {
  const [cfg, setCfg] = useState<AppConfig>({
    default_quality: '1080P',
    default_speed: '标准 (8线程)',
    default_outdir: 'downloads',
    auto_merge: true,
    max_history: 200,
  });
  const [saved, setSaved] = useState(false);
  const [appDir, setAppDir] = useState('');
  const [ffmpegStatus, setFfmpegStatus] = useState<FfmpegStatus>({ available: false });
  const [ffmpegChecked, setFfmpegChecked] = useState(false);
  const [installingFfmpeg, setInstallingFfmpeg] = useState(false);
  const [ffmpegTaskId, setFfmpegTaskId] = useState<string | null>(null);
  const [ffmpegInstallProgress, setFfmpegInstallProgress] = useState(0);
  const [ffmpegInstallError, setFfmpegInstallError] = useState('');

  useEffect(() => {
    Bridge.getConfig().then(setCfg);
    Bridge.getAppDir().then(setAppDir);
    Bridge.checkFfmpeg().then(status => {
      setFfmpegStatus(status);
      setFfmpegChecked(true);
    });
  }, []);

  const handleSave = async () => {
    await Bridge.saveConfig(cfg);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleInstallFfmpeg = async () => {
    setInstallingFfmpeg(true);
    setFfmpegInstallProgress(0);
    setFfmpegInstallError('');
    try {
      const taskId = await Bridge.installFfmpeg();
      setFfmpegTaskId(taskId);
      const removeProgress = Bridge.addProgressListener((eventTaskId, percent) => {
        if (eventTaskId === taskId) setFfmpegInstallProgress(percent);
      });
      const removeDone = Bridge.addTaskDoneListener(async (eventTaskId, result) => {
        if (eventTaskId !== taskId) return;
        removeProgress();
        removeDone();
        setInstallingFfmpeg(false);
        setFfmpegTaskId(null);
        if (result.status === 'completed') {
          setFfmpegInstallProgress(100);
          setFfmpegStatus(await Bridge.checkFfmpeg());
          setFfmpegChecked(true);
        } else {
          setFfmpegInstallError(result.message || 'FFmpeg 安装失败');
        }
      });
    } catch (err) {
      setInstallingFfmpeg(false);
      setFfmpegTaskId(null);
      setFfmpegInstallError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleChooseOutdir = async () => {
    const selected = await Bridge.chooseOutputDir();
    if (selected) {
      setCfg({ ...cfg, default_outdir: selected });
    }
  };

  return (
    <div className="max-w-4xl mx-auto w-full p-8 md:p-12 flex flex-col gap-8 min-h-full">
      <div>
        <h1 className="text-3xl font-black tracking-tight text-gray-900">设置</h1>
        <p className="text-gray-500 font-medium mt-1">管理程序工具、文件保存和本地数据</p>
      </div>

      {/* 程序信息 */}
      <div className="glass-panel rounded-[2rem] p-6 md:p-8 flex flex-col gap-5">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-[#E7F6FF] text-bili-blue rounded-xl flex items-center justify-center">
            <Wrench size={20} />
          </div>
          <h2 className="text-lg font-bold text-gray-900">程序信息</h2>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
          <div className="bg-white/40 rounded-2xl p-4 border border-white/60">
            <span className="text-gray-500 font-medium">安装目录</span>
            <p className="font-bold text-gray-800 mt-1 truncate">{appDir || '加载中...'}</p>
          </div>
          <div className="bg-white/40 rounded-2xl p-4 border border-white/60">
            <span className="text-gray-500 font-medium">FFmpeg 状态</span>
            <p className="font-bold mt-1">
              {!ffmpegChecked ? (
                <span className="inline-flex items-center gap-2 text-gray-500">
                  <span className="h-2 w-2 rounded-full bg-gray-300" />
                  正在检测
                </span>
              ) : ffmpegStatus.available ? (
                <span className="inline-flex items-center gap-2 text-green-600">
                  <span className="h-2 w-2 rounded-full bg-green-500" />
                  可用，可直接合并
                </span>
              ) : (
                <span className="inline-flex items-center gap-2 text-orange-500">
                  <span className="h-2 w-2 rounded-full bg-orange-400" />
                  未找到
                </span>
              )}
            </p>
            {ffmpegStatus.path && (
              <p className="text-xs text-gray-500 mt-2 truncate">{ffmpegStatus.path}</p>
            )}
          </div>
        </div>
        {ffmpegChecked && !ffmpegStatus.available && (
          <div className="flex flex-col items-start gap-3">
            <button
              onClick={handleInstallFfmpeg}
              disabled={installingFfmpeg}
              className="px-6 py-3 bg-gradient-to-r from-orange-400 to-orange-500 text-white rounded-2xl font-bold shadow-md hover:shadow-lg transition-all disabled:opacity-60"
            >
              {installingFfmpeg ? '安装中...' : '下载 FFmpeg 兜底组件'}
            </button>
            {installingFfmpeg && (
              <div className="w-full max-w-md">
                <div className="h-2 bg-orange-100 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-orange-400 transition-all"
                    style={{ width: `${Math.max(3, ffmpegInstallProgress)}%` }}
                  />
                </div>
                <p className="mt-1 text-xs font-bold text-orange-500">
                  {ffmpegTaskId ? `正在下载并安装 FFmpeg · ${Math.round(ffmpegInstallProgress)}%` : '正在准备安装'}
                </p>
              </div>
            )}
            {ffmpegInstallError && (
              <p className="text-xs font-bold text-red-500 bg-red-50 border border-red-100 rounded-xl px-4 py-2">
                {ffmpegInstallError}
              </p>
            )}
          </div>
        )}
      </div>

      {/* 文件保存 */}
      <div className="glass-panel rounded-[2rem] p-6 md:p-8 flex flex-col gap-5">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-pink-100 text-bili-pink rounded-xl flex items-center justify-center">
            <HardDrive size={20} />
          </div>
          <h2 className="text-lg font-bold text-gray-900">文件保存</h2>
        </div>

        <div className="grid grid-cols-1 gap-5">
          <div>
            <label className="block text-sm font-bold text-gray-600 mb-2">默认输出目录</label>
            <div className="flex gap-3">
              <input
                type="text"
                value={cfg.default_outdir}
                onChange={e => setCfg({ ...cfg, default_outdir: e.target.value })}
                className="flex-1 bg-white/60 border border-white/80 rounded-2xl py-3 px-4 focus:outline-none focus:ring-2 focus:ring-bili-pink/30 text-sm font-medium text-gray-700"
              />
              <button
                onClick={handleChooseOutdir}
                className="px-5 py-3 bg-white/60 border border-white/80 rounded-2xl text-gray-500 hover:text-bili-pink font-bold transition-all"
                title="选择输出目录"
              >
                <FolderOpen size={20} />
              </button>
            </div>
          </div>

          <div>
            <label className="flex items-center gap-3 cursor-pointer">
              <input
                type="checkbox"
                checked={cfg.auto_merge}
                onChange={e => setCfg({ ...cfg, auto_merge: e.target.checked })}
                className="accent-bili-pink w-5 h-5"
              />
              <span className="font-bold text-gray-700">
                自动合并音视频（需要 FFmpeg）
              </span>
            </label>
          </div>
        </div>

        <button
          onClick={handleSave}
          className="self-end px-8 py-3 bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white rounded-2xl font-bold shadow-[0_12px_28px_rgba(255,143,179,0.24)] hover:shadow-[0_16px_34px_rgba(255,143,179,0.32)] hover:scale-[1.02] transition-all"
        >
          {saved ? '已保存' : '保存设置'}
        </button>
      </div>

      {/* 数据存储说明 */}
      <div className="bg-[#EAF7FF]/70 border border-[#D8E4F0] rounded-2xl p-5 text-sm text-[#2377A6]">
        <p className="font-bold mb-1 flex items-center gap-2">
          <Database size={16} />
          数据存储说明
        </p>
        <p className="text-[#2377A6]/80">
          程序配置、Cookie、下载历史和兜底 FFmpeg 都保存在 <strong>{appDir}</strong> 目录中。
          下载的视频默认保存在 <strong>{appDir}/downloads/</strong>，避免文件散落到系统数据目录或用户视频目录。
        </p>
      </div>

    </div>
  );
}
