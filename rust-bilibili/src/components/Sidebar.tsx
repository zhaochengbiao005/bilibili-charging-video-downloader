import React, { useEffect, useState } from 'react';
import { Clock, Home, Info, Settings as SettingsIcon } from 'lucide-react';
import { NavLink, useLocation } from 'react-router-dom';
import defaultLoginAvatar from '../assets/user-login-default.jpg';
import type { LoginStatus } from '../types';
import * as Bridge from '../bridge';

interface SidebarProps {
  loginStatus: LoginStatus | null;
  onLoginClick: () => void;
}

export function Sidebar({ loginStatus, onLoginClick }: SidebarProps) {
  const [avatarSrc, setAvatarSrc] = useState(defaultLoginAvatar);

  useEffect(() => {
    let cancelled = false;
    const avatar = loginStatus?.is_login ? loginStatus.avatar?.trim() : '';

    if (!avatar) {
      setAvatarSrc(defaultLoginAvatar);
      return;
    }

    Bridge.fetchImageDataUrl(avatar)
      .then((src) => {
        if (!cancelled) setAvatarSrc(src || defaultLoginAvatar);
      })
      .catch(() => {
        if (!cancelled) setAvatarSrc(avatar);
      });

    return () => {
      cancelled = true;
    };
  }, [loginStatus?.avatar, loginStatus?.is_login]);

  const linkClass = ({ isActive }: { isActive: boolean }) =>
    `flex items-center gap-4 px-4 py-3 rounded-2xl font-bold transition-all ${
      isActive
        ? 'bg-white/76 text-bili-pink shadow-[0_10px_26px_rgba(255,143,179,0.14)] border border-white/70'
        : 'text-gray-600 hover:bg-white/48 border border-transparent'
    }`;

  return (
    <aside className="w-[256px] bg-white/58 border-r border-white/80 h-full flex flex-col shrink-0 rounded-none shadow-none backdrop-blur-[24px]">
      <button
        onClick={onLoginClick}
        className="group px-8 pt-9 pb-7 flex items-center gap-3 text-left transition hover:bg-white/42"
        title="登录哔哩哔哩"
      >
        <img
          src={avatarSrc}
          alt={loginStatus?.is_login ? '用户头像' : '未登录头像'}
          className="h-12 w-12 rounded-full border-2 border-white/90 object-cover shadow-[0_8px_20px_rgba(255,143,179,0.14)] transition group-hover:scale-105"
          referrerPolicy="no-referrer"
          onError={() => setAvatarSrc(defaultLoginAvatar)}
        />
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

      <div className="pb-6 mt-auto" />
    </aside>
  );
}
