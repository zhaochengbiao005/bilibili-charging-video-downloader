import React, { useState, useEffect } from 'react';
import { Cloud, Database, ExternalLink, FolderOpen, HardDrive, KeyRound, LogOut, Wrench } from 'lucide-react';
import type { AppConfig, CloudAuthStatus, CloudConfig, FfmpegStatus } from '../types';
import * as Bridge from '../bridge';

export function Settings() {
  const [cfg, setCfg] = useState<AppConfig>({
    default_quality: '1080P',
    default_speed: '标准 (8线程)',
    default_outdir: 'downloads',
    auto_merge: true,
    max_history: 200,
  });
  const [cloudCfg, setCloudCfg] = useState<CloudConfig>({
    default_provider: 'baidu_netdisk',
    default_remote_dir: '/apps/B站充电视频下载器',
    default_save_mode: 'local',
    part_size_mb: 4,
    baidu: {
      client_id: '',
      client_secret: '',
      redirect_uri: 'http://localhost:1421/baidu/callback',
      scope: 'basic,netdisk',
    },
  });
  const [cloudStatus, setCloudStatus] = useState<CloudAuthStatus>({
    provider: 'baidu_netdisk',
    is_authorized: false,
    message: '百度网盘未授权',
  });
  const [authCode, setAuthCode] = useState('');
  const [authState, setAuthState] = useState('');
  const [cloudSaved, setCloudSaved] = useState(false);
  const [cloudMessage, setCloudMessage] = useState('');
  const [saved, setSaved] = useState(false);
  const [appDir, setAppDir] = useState('');
  const [ffmpegStatus, setFfmpegStatus] = useState<FfmpegStatus>({ available: false });
  const [ffmpegChecked, setFfmpegChecked] = useState(false);
  const [installingFfmpeg, setInstallingFfmpeg] = useState(false);
  const [ffmpegTaskId, setFfmpegTaskId] = useState<string | null>(null);
  const [ffmpegInstallProgress, setFfmpegInstallProgress] = useState(0);
  const [ffmpegInstallError, setFfmpegInstallError] = useState('');
  const cloudInputClass = 'bg-[#F6F9FE]/90 border border-[#D8E4F0] rounded-2xl py-3 px-4 shadow-[0_10px_24px_rgba(255,143,179,0.10)] focus:outline-none focus:ring-2 focus:ring-bili-pink/30 text-sm font-medium text-gray-700';

  useEffect(() => {
    Bridge.getConfig().then(setCfg);
    Bridge.getCloudConfig().then(setCloudCfg);
    Bridge.baiduAuthStatus().then(setCloudStatus).catch(err => {
      setCloudStatus({
        provider: 'baidu_netdisk',
        is_authorized: false,
        message: err instanceof Error ? err.message : String(err),
      });
    });
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

  const handleSaveCloud = async () => {
    const savedConfig = await Bridge.saveCloudConfig(cloudCfg);
    setCloudCfg(savedConfig);
    setCloudSaved(true);
    setCloudMessage('百度网盘设置已保存');
    setTimeout(() => setCloudSaved(false), 2000);
  };

  const handleStartBaiduAuth = async () => {
    try {
      const savedConfig = await Bridge.saveCloudConfig(cloudCfg);
      setCloudCfg(savedConfig);
      const response = await Bridge.baiduAuthStart();
      setAuthState(response.state);
      setCloudMessage('已打开百度授权页面');
    } catch (err) {
      setCloudMessage(err instanceof Error ? err.message : String(err));
    }
  };

  const handleFinishBaiduAuth = async () => {
    try {
      const status = await Bridge.baiduAuthFinish({
        code: authCode,
        state: authState || null,
      });
      setCloudStatus(status);
      setAuthCode('');
      setCloudMessage(status.message || '百度网盘已授权');
    } catch (err) {
      setCloudMessage(err instanceof Error ? err.message : String(err));
    }
  };

  const handleBaiduLogout = async () => {
    const status = await Bridge.baiduLogout();
    setCloudStatus(status);
    setCloudMessage(status.message || '已退出百度网盘');
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
              className="motion-button px-6 py-3 bg-gradient-to-r from-orange-400 to-orange-500 text-white rounded-2xl font-bold shadow-md hover:shadow-lg disabled:opacity-60"
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

      {/* 百度网盘 */}
      <div className="glass-panel rounded-[2rem] p-6 md:p-8 flex flex-col gap-5">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 bg-[#EAF7FF] text-bili-blue rounded-xl flex items-center justify-center">
              <Cloud size={20} />
            </div>
            <h2 className="text-lg font-bold text-gray-900">百度网盘</h2>
          </div>
          <span className={`inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-xs font-black ${
            cloudStatus.is_authorized
              ? 'bg-green-50 text-green-600 border border-green-100'
              : 'bg-orange-50 text-orange-500 border border-orange-100'
          }`}>
            <span className={`h-2 w-2 rounded-full ${cloudStatus.is_authorized ? 'bg-green-500' : 'bg-orange-400'}`} />
            {cloudStatus.is_authorized ? '已授权' : '未授权'}
          </span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <label className="flex flex-col gap-2">
            <span className="text-sm font-bold text-gray-600">应用 ID</span>
            <input
              type="text"
              value={cloudCfg.baidu.client_id}
              onChange={e => setCloudCfg({
                ...cloudCfg,
                baidu: { ...cloudCfg.baidu, client_id: e.target.value },
              })}
              className={cloudInputClass}
            />
          </label>
          <label className="flex flex-col gap-2">
            <span className="text-sm font-bold text-gray-600">应用密钥</span>
            <input
              type="password"
              autoComplete="off"
              value={cloudCfg.baidu.client_secret}
              onChange={e => setCloudCfg({
                ...cloudCfg,
                baidu: { ...cloudCfg.baidu, client_secret: e.target.value },
              })}
              className={cloudInputClass}
            />
          </label>
          <label className="flex flex-col gap-2 md:col-span-2">
            <span className="text-sm font-bold text-gray-600">回调地址</span>
            <input
              type="text"
              value={cloudCfg.baidu.redirect_uri}
              onChange={e => setCloudCfg({
                ...cloudCfg,
                baidu: { ...cloudCfg.baidu, redirect_uri: e.target.value },
              })}
              className={cloudInputClass}
            />
          </label>
          <label className="flex flex-col gap-2">
            <span className="text-sm font-bold text-gray-600">云端目录</span>
            <input
              type="text"
              value={cloudCfg.default_remote_dir}
              onChange={e => setCloudCfg({ ...cloudCfg, default_remote_dir: e.target.value })}
              className={cloudInputClass}
            />
          </label>
          <label className="flex flex-col gap-2">
            <span className="text-sm font-bold text-gray-600">分片大小（MB）</span>
            <input
              type="number"
              min={1}
              max={64}
              value={cloudCfg.part_size_mb}
              onChange={e => setCloudCfg({
                ...cloudCfg,
                part_size_mb: Math.max(1, Number(e.target.value) || 4),
              })}
              className={cloudInputClass}
            />
          </label>
        </div>

        <div className="grid grid-cols-2 gap-2 rounded-2xl bg-white/70 border border-pink-50 p-1.5">
          {[
            { value: 'local' as const, label: '默认本地' },
            { value: 'baidu_netdisk' as const, label: '默认网盘' },
          ].map((item) => (
            <button
              key={item.value}
              type="button"
              onClick={() => setCloudCfg({ ...cloudCfg, default_save_mode: item.value })}
              className={`motion-button min-h-11 rounded-xl text-sm font-black ${
                cloudCfg.default_save_mode === item.value
                  ? 'bg-white text-bili-pink shadow-sm border border-pink-100'
                  : 'text-gray-500 hover:text-bili-pink hover:bg-white/50 border border-transparent'
              }`}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <button
            onClick={handleSaveCloud}
            className="motion-button px-5 py-3 bg-white/70 border border-white/85 rounded-2xl text-gray-600 hover:text-bili-pink font-bold"
          >
            {cloudSaved ? '已保存' : '保存网盘设置'}
          </button>
          <button
            onClick={handleStartBaiduAuth}
            className="motion-button inline-flex items-center gap-2 px-5 py-3 bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white rounded-2xl font-bold shadow-[0_12px_28px_rgba(255,143,179,0.24)]"
          >
            <ExternalLink size={18} />
            打开授权
          </button>
          {cloudStatus.is_authorized && (
            <button
              onClick={handleBaiduLogout}
              className="motion-button inline-flex items-center gap-2 px-5 py-3 bg-white/70 border border-white/85 rounded-2xl text-gray-600 hover:text-bili-pink font-bold"
            >
              <LogOut size={18} />
              退出授权
            </button>
          )}
        </div>

        <div className="flex flex-col md:flex-row gap-3">
          <div className="relative flex-1">
            <KeyRound className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-400" size={18} />
            <input
              type="text"
              value={authCode}
              onChange={e => setAuthCode(e.target.value)}
              placeholder="粘贴百度授权码"
              className="w-full bg-white/70 border border-white/85 rounded-2xl py-3 pl-11 pr-4 focus:outline-none focus:ring-2 focus:ring-bili-pink/30 text-sm font-medium text-gray-700"
            />
          </div>
          <button
            onClick={handleFinishBaiduAuth}
            disabled={!authCode.trim()}
            className="motion-button px-6 py-3 bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white rounded-2xl font-bold shadow-[0_12px_28px_rgba(255,143,179,0.24)] disabled:opacity-50"
          >
            完成授权
          </button>
        </div>

        {(cloudMessage || cloudStatus.message) && (
          <p className="text-xs font-bold text-[#2377A6] bg-[#EAF7FF]/70 border border-[#D8E4F0] rounded-xl px-4 py-2">
            {cloudMessage || cloudStatus.message}
          </p>
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
                className="motion-button px-5 py-3 bg-white/60 border border-white/80 rounded-2xl text-gray-500 hover:text-bili-pink font-bold"
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
          className="motion-button self-end px-8 py-3 bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white rounded-2xl font-bold shadow-[0_12px_28px_rgba(255,143,179,0.24)] hover:shadow-[0_16px_34px_rgba(255,143,179,0.32)]"
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
          下载的视频默认保存在 <strong>{appDir}/downloads/</strong>。
        </p>
      </div>

    </div>
  );
}
