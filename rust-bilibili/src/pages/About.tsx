import React from 'react';
import { Info, Code, ShieldCheck, Heart } from 'lucide-react';

export function About() {
  return (
    <div className="max-w-4xl mx-auto w-full p-8 md:p-12 flex flex-col gap-10 min-h-full">
       <div className="text-center flex flex-col items-center mt-4 shrink-0">
          <div className="w-16 h-16 bg-gradient-to-tr from-bili-pink to-bili-pink-hover rounded-2xl flex items-center justify-center text-white mb-6 shadow-[0_12px_28px_rgba(255,143,179,0.24)] rotate-3">
             <Info size={32} strokeWidth={2.5} className="-rotate-3" />
          </div>
          <h1 className="text-4xl font-black tracking-tight mb-4 text-gray-900 drop-shadow-sm">关于 B站视频下载器</h1>
          <p className="text-lg text-gray-600 font-medium max-w-xl shadow-sm">
            一个快速、安全、现代的 B站视频下载工具，帮助你在本地完成解析、下载、合并和历史管理。
          </p>
       </div>

       <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mt-4">
         <div className="glass-panel p-8 rounded-[2rem] flex flex-col gap-4">
            <div className="w-12 h-12 bg-[#E7F6FF] text-bili-blue rounded-xl flex items-center justify-center mb-2">
               <ShieldCheck size={24} strokeWidth={2.5} />
            </div>
            <h3 className="text-xl font-bold text-gray-900">本地处理，隐私优先</h3>
            <p className="text-gray-600 font-medium leading-relaxed">
               配置、Cookie 和下载历史都保存在你的电脑上。前端只通过 Tauri command 调用本地 Rust 后端。
            </p>
         </div>
         <div className="glass-panel p-8 rounded-[2rem] flex flex-col gap-4">
            <div className="w-12 h-12 bg-pink-100 text-bili-pink rounded-xl flex items-center justify-center mb-2">
               <Code size={24} strokeWidth={2.5} />
            </div>
            <h3 className="text-xl font-bold text-gray-900">Rust + Tauri 新主线</h3>
            <p className="text-gray-600 font-medium leading-relaxed">
               界面使用 React 与 Tailwind，桌面壳使用 Tauri，下载、合并和任务调度逐步迁移到 Rust 后端。
            </p>
         </div>
       </div>

       <div className="glass-panel p-8 md:p-10 rounded-[2rem] mt-6 flex flex-col md:flex-row items-center justify-between gap-8">
          <div>
            <h3 className="text-2xl font-bold text-gray-900 mb-2">支持项目</h3>
            <p className="text-gray-600 font-medium">如果这个工具帮到了你，可以继续关注后续的登录、下载和合并能力完善。</p>
          </div>
          <button className="flex items-center gap-2 px-8 py-4 bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white rounded-2xl font-bold shadow-[0_14px_30px_rgba(255,143,179,0.28)] hover:scale-105 transition-all shrink-0">
             <Heart size={20} strokeWidth={2.5} />
             支持一下
          </button>
       </div>

       <div className="text-center text-gray-500 font-medium text-sm mt-12 pb-8">
          <p>© {new Date().getFullYear()} B站视频下载器</p>
          <p className="mt-1">Rust/Tauri 重写版</p>
       </div>
    </div>
  );
}
