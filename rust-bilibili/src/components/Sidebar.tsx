import React from 'react';
import { Clock, Home, Info, Settings as SettingsIcon, Heart } from 'lucide-react';
import { NavLink, useLocation } from 'react-router-dom';
import { AkariSilhouette } from './Mascots';
import type { LoginStatus } from '../types';

interface SidebarProps {
  loginStatus: LoginStatus | null;
  onLoginClick: () => void;
}

export function Sidebar({ loginStatus, onLoginClick }: SidebarProps) {
  const linkClass = ({ isActive }: { isActive: boolean }) =>
    `flex items-center gap-4 px-4 py-3 rounded-2xl font-bold transition-all ${
      isActive
        ? 'bg-white/70 text-bili-pink shadow-sm border border-white/50'
        : 'text-gray-600 hover:bg-white/40 border border-transparent'
    }`;

  return (
    <aside className="w-[256px] bg-white/55 border-r border-white/75 h-full flex flex-col shrink-0 rounded-none shadow-none">
      <button
        onClick={onLoginClick}
        className="group px-8 pt-9 pb-7 flex items-center gap-3 text-left transition hover:bg-white/35"
        title="登录哔哩哔哩"
      >
        <AkariSilhouette className="h-12 w-12 opacity-90 transition group-hover:scale-105" />
        <div>
          <h2 className="font-black text-lg text-bili-pink tracking-tight leading-tight">B站视频下载器</h2>
          <p className="text-xs text-gray-500 font-medium">
            {loginStatus?.is_login
              ? `${loginStatus.username || '已登录'}${loginStatus.level !== null && loginStatus.level !== undefined ? ` · LV${loginStatus.level}` : ''}`
              : '点击登录哔哩哔哩'}
          </p>
        </div>
      </button>

      <nav className="flex-1 px-4 py-6 flex flex-col gap-3">
        <NavLink to="/" end className={linkClass}>
          <Home size={20} strokeWidth={2.5} />
          首页
        </NavLink>

        <NavLink to="/history" className={linkClass}>
          <Clock size={20} strokeWidth={2.5} />
          下载历史
        </NavLink>

        <NavLink to="/about" className={linkClass}>
          <Info size={20} strokeWidth={2.5} />
          关于
        </NavLink>

        <NavLink to="/settings" className={linkClass}>
          <SettingsIcon size={20} strokeWidth={2.5} />
          设置
        </NavLink>
      </nav>

      <div className="px-10 pb-6 mt-auto">
        <button className="w-full flex items-center justify-center gap-2 py-3.5 bg-pink-50/55 hover:bg-pink-100/60 text-bili-pink rounded-2xl font-black transition-all border border-pink-100 shadow-[0_8px_18px_rgba(251,114,153,0.12)]">
          <Heart size={18} strokeWidth={2.5} />
          支持项目
        </button>
      </div>
    </aside>
  );
}
