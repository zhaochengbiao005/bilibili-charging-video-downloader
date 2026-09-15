import React, { useState, useEffect } from 'react';
import { Settings2, FolderOpen, Download, Wifi, Cpu } from 'lucide-react';
import type { AppConfig } from '../types';
import * as Bridge from '../bridge';

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
  const [ffmpegOk, setFfmpegOk] = useState(false);
  const [installingFfmpeg, setInstallingFfmpeg] = useState(false);

  useEffect(() => {
    Bridge.getConfig().then(setCfg);
    Bridge.getAppDir().then(setAppDir);
    Bridge.checkFfmpeg().then(setFfmpegOk);
  }, []);

  const handleSave = async () => {
    await Bridge.saveConfig(cfg);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleInstallFfmpeg = async () => {
    setInstallingFfmpeg(true);
    await Bridge.installFfmpeg();
    setInstallingFfmpeg(false);
    setFfmpegOk(true);
  };

  return (
    <div className="max-w-4xl mx-auto w-full p-8 md:p-12 flex flex-col gap-8 min-h-full">
      <div>
        <h1 className="text-3xl font-black tracking-tight text-gray-900">设置</h1>
        <p className="text-gray-500 font-medium mt-1">自定义下载和程序行为</p>
      </div>

      {/* 程序信息 */}
      <div className="glass-panel rounded-[2rem] p-6 md:p-8 flex flex-col gap-5">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-blue-100 text-blue-600 rounded-xl flex items-center justify-center">
            <Download size={20} />
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
              {ffmpegOk ? (
                <span className="text-green-600">✅ 已安装</span>
              ) : (
                <span className="text-orange-500">❌ 未安装</span>
              )}
            </p>
          </div>
        </div>
        {!ffmpegOk && (
          <button
            onClick={handleInstallFfmpeg}
            disabled={installingFfmpeg}
            className="self-start px-6 py-3 bg-gradient-to-r from-orange-400 to-orange-500 text-white rounded-2xl font-bold shadow-md hover:shadow-lg transition-all disabled:opacity-60"
          >
            {installingFfmpeg ? '安装中...' : '安装 FFmpeg'}
          </button>
        )}
      </div>

      {/* 下载设置 */}
      <div className="glass-panel rounded-[2rem] p-6 md:p-8 flex flex-col gap-5">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-pink-100 text-bili-pink rounded-xl flex items-center justify-center">
            <Settings2 size={20} />
          </div>
          <h2 className="text-lg font-bold text-gray-900">默认下载设置</h2>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
          <div>
            <label className="block text-sm font-bold text-gray-600 mb-2">默认画质</label>
            <select
              value={cfg.default_quality}
              onChange={e => setCfg({ ...cfg, default_quality: e.target.value })}
              className="w-full bg-white/60 border border-white/80 rounded-2xl py-3 px-4 focus:outline-none focus:ring-2 focus:ring-bili-pink/30 text-sm font-medium text-gray-700"
            >
              {['360P', '480P', '720P', '1080P', '1080P60', '4K', 'HDR'].map(q => (
                <option key={q} value={q}>{q}</option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-bold text-gray-600 mb-2">下载速度</label>
            <select
              value={cfg.default_speed}
              onChange={e => setCfg({ ...cfg, default_speed: e.target.value })}
              className="w-full bg-white/60 border border-white/80 rounded-2xl py-3 px-4 focus:outline-none focus:ring-2 focus:ring-bili-pink/30 text-sm font-medium text-gray-700"
            >
              {['慢速 (4线程)', '标准 (8线程)', '快速 (16线程)', '极速 (32线程)'].map(s => (
                <option key={s} value={s}>{s}</option>
              ))}
            </select>
          </div>

          <div className="md:col-span-2">
            <label className="block text-sm font-bold text-gray-600 mb-2">默认输出目录</label>
            <div className="flex gap-3">
              <input
                type="text"
                value={cfg.default_outdir}
                onChange={e => setCfg({ ...cfg, default_outdir: e.target.value })}
                className="flex-1 bg-white/60 border border-white/80 rounded-2xl py-3 px-4 focus:outline-none focus:ring-2 focus:ring-bili-pink/30 text-sm font-medium text-gray-700"
              />
              <button className="px-5 py-3 bg-white/60 border border-white/80 rounded-2xl text-gray-500 hover:text-bili-pink font-bold transition-all">
                <FolderOpen size={20} />
              </button>
            </div>
          </div>

          <div className="md:col-span-2">
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
          className="self-end px-8 py-3 bg-gradient-to-r from-bili-pink to-pink-400 text-white rounded-2xl font-bold shadow-md hover:shadow-lg hover:scale-[1.02] transition-all"
        >
          {saved ? '✅ 已保存' : '保存设置'}
        </button>
      </div>

      {/* 数据存储说明 */}
      <div className="bg-blue-50/50 border border-blue-100 rounded-2xl p-5 text-sm text-blue-700">
        <p className="font-bold mb-1">📁 数据存储说明</p>
        <p className="text-blue-600/80">
          程序配置、Cookie 和下载历史保存在 <strong>{appDir}/data/</strong> 目录中。
          下载的视频保存在输出目录（默认 <strong>{appDir}/downloads/</strong>），卸载程序时视频文件不会被删除。
        </p>
      </div>
    </div>
  );
}
