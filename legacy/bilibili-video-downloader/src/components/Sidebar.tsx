import React from 'react';
import { Home, Clock, Settings as SettingsIcon, Heart, Info } from 'lucide-react';
import { NavLink, useLocation } from 'react-router-dom';

export function Sidebar() {
  const location = useLocation();
  const isAbout = location.pathname === '/about';

  const linkClass = ({ isActive }: { isActive: boolean }) =>
    `flex items-center gap-4 px-4 py-3 rounded-2xl font-bold transition-all ${
      isActive
        ? 'bg-white/70 text-bili-pink shadow-sm border border-white/50'
        : 'text-gray-600 hover:bg-white/40 border border-transparent'
    }`;

  return (
    <aside className="w-64 glass-panel border-r border-white/80 h-full flex flex-col shrink-0 rounded-none shadow-none border-y-0 border-l-0">
      {/* Logo */}
      <div className="p-8 flex items-center gap-3">
        <div className="w-12 h-12 rounded-full bg-gradient-to-tr from-bili-pink to-pink-300 p-0.5 shadow-md flex items-center justify-center">
          <div className="w-full h-full rounded-full bg-white flex items-center justify-center text-xl font-black text-bili-pink">
            B
          </div>
        </div>
        <div>
          <h2 className="font-black text-lg text-bili-pink tracking-tight leading-tight">BiliDownloader</h2>
          <p className="text-xs text-gray-500 font-medium">视频下载工具</p>
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 px-4 py-6 flex flex-col gap-2">
        <NavLink to="/" end className={linkClass}>
          <Home size={20} strokeWidth={2.5} />
          首页
        </NavLink>

        <NavLink to="/history" className={linkClass}>
          <Clock size={20} strokeWidth={2.5} />
          下载历史
        </NavLink>

        <NavLink to="/settings" className={linkClass}>
          <SettingsIcon size={20} strokeWidth={2.5} />
          设置
        </NavLink>

        <NavLink to="/about" className={linkClass}>
          <Info size={20} strokeWidth={2.5} />
          关于
        </NavLink>
      </nav>

      {/* Bottom */}
      <div className="p-6 mt-auto">
        <button className="w-full flex items-center justify-center gap-2 py-3.5 bg-pink-50/50 hover:bg-pink-100/50 text-bili-pink rounded-2xl font-bold transition-all border border-pink-100 shadow-sm">
          <Heart size={18} strokeWidth={2.5} />
          支持项目
        </button>
      </div>
    </aside>
  );
}
