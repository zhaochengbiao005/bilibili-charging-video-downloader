import React, { useEffect, useState } from 'react';
import { CheckCircle2, FileJson, Lock, LogOut, Mail, QrCode, RotateCcw, Smartphone, X } from 'lucide-react';
import * as Bridge from '../bridge';
import { Mascot22, Mascot33, QrGlyph } from './Mascots';
import type { LoginStatus, QrLoginStartResponse } from '../types';

interface LoginModalProps {
  loginStatus: LoginStatus | null;
  onLoginChange: (status: LoginStatus) => void;
  onClose: () => void;
}

export function LoginModal({ loginStatus, onLoginChange, onClose }: LoginModalProps) {
  const [mode, setMode] = useState<'qr' | 'password'>('qr');
  const [passwordMode, setPasswordMode] = useState<'password' | 'sms'>('password');
  const [message, setMessage] = useState('');
  const [qrData, setQrData] = useState<QrLoginStartResponse | null>(null);
  const [isQrLoading, setIsQrLoading] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

  useEffect(() => {
    if (!qrData) return;

    let stopped = false;
    const timer = window.setInterval(async () => {
      try {
        const result = await Bridge.pollQrLogin(qrData.qrcode_key);
        if (stopped) return;
        setMessage(result.message);
        if (result.status === 'confirmed' && result.login) {
          onLoginChange(result.login);
          window.clearInterval(timer);
          setQrData(null);
        }
        if (result.status === 'expired') {
          window.clearInterval(timer);
          setQrData(null);
        }
      } catch (err) {
        if (!stopped) {
          setMessage(err instanceof Error ? err.message : '扫码状态检查失败');
        }
      }
    }, 2000);

    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [onLoginChange, qrData]);

  const handleQrLogin = async () => {
    setMessage('');
    setIsQrLoading(true);
    try {
      const data = await Bridge.startQrLogin();
      setQrData(data);
      setMessage('二维码已生成，请在哔哩哔哩客户端确认登录。');
    } catch (err) {
      setMessage(err instanceof Error ? err.message : '扫码登录启动失败。');
    } finally {
      setIsQrLoading(false);
    }
  };

  const handleImportCookie = async () => {
    setMessage('');
    setIsImporting(true);
    try {
      const path = await Bridge.chooseCookieFile();
      if (!path) return;
      const status = await Bridge.checkCookie(path);
      onLoginChange(status);
      setMessage(status.is_login ? 'Cookie 登录成功。' : status.message || 'Cookie 未登录或已失效。');
    } catch (err) {
      setMessage(err instanceof Error ? err.message : 'Cookie 导入失败。');
    } finally {
      setIsImporting(false);
    }
  };

  const handleLogout = async () => {
    setMessage('');
    try {
      const status = await Bridge.clearCookie();
      onLoginChange(status);
      setQrData(null);
      setMessage('已退出登录。');
    } catch (err) {
      setMessage(err instanceof Error ? err.message : '退出登录失败。');
    }
  };

  return (
    <div className="relative flex w-full max-h-[calc(100vh-1.5rem)] pt-10 sm:pt-12">
      <div className="absolute left-1/2 top-0 z-10 flex -translate-x-1/2 items-end gap-10">
        <Mascot22 className="h-12 w-16 sm:h-14 sm:w-[4.5rem] drop-shadow-[0_14px_20px_rgba(37,99,235,0.18)]" />
        <Mascot33 className="h-12 w-16 sm:h-14 sm:w-[4.5rem] drop-shadow-[0_14px_20px_rgba(251,114,153,0.2)]" />
      </div>

      <button
        onClick={onClose}
        className="absolute right-3 top-3 z-20 h-9 w-9 rounded-full border border-white bg-white/85 text-gray-500 shadow-lg transition hover:scale-105 hover:text-bili-pink"
        aria-label="关闭登录"
      >
        <X className="mx-auto" size={18} />
      </button>

      <section className="relative flex w-full max-h-[calc(100vh-1.5rem)] flex-col overflow-hidden rounded-[22px] border border-pink-100 bg-white/92 shadow-[0_24px_70px_rgba(31,38,135,0.22)] backdrop-blur-[28px]">
        <div className="shrink-0 px-6 pt-5 text-center sm:px-7">
          <h2 className="text-[26px] font-black tracking-tight text-bili-pink">登录哔哩哔哩</h2>
          <p className="mt-1 text-xs font-medium text-gray-400">
            {loginStatus?.is_login
              ? `${loginStatus.username || '已登录'} · LV${loginStatus.level ?? 0}`
              : '解锁 1080P/4K 高清画质与高帧率视频下载'}
          </p>
        </div>

        {loginStatus?.is_login && (
          <div className="mx-6 mt-4 flex shrink-0 items-center justify-between rounded-2xl border border-green-100 bg-green-50 px-4 py-3 text-sm sm:mx-7">
            <span className="flex min-w-0 items-center gap-2 font-black text-green-600">
              <CheckCircle2 size={16} />
              <span className="truncate">已登录：{loginStatus.username}</span>
            </span>
            <button
              onClick={handleLogout}
              className="flex shrink-0 items-center gap-1.5 rounded-full bg-white px-3 py-1.5 text-xs font-black text-gray-500 shadow-sm transition hover:text-bili-pink"
            >
              <LogOut size={13} />
              退出
            </button>
          </div>
        )}

        <div className="mx-6 mt-5 grid shrink-0 grid-cols-2 rounded-2xl bg-gray-100 p-1 sm:mx-7">
          <button
            onClick={() => setMode('qr')}
            className={`flex items-center justify-center gap-2 rounded-xl py-2.5 text-sm font-black transition ${
              mode === 'qr' ? 'bg-white text-bili-pink shadow-md' : 'text-gray-500'
            }`}
          >
            <QrCode size={16} />
            扫码 / Cookie
          </button>
          <button
            onClick={() => setMode('password')}
            className={`flex items-center justify-center gap-2 rounded-xl py-2.5 text-sm font-black transition ${
              mode === 'password' ? 'bg-white text-bili-pink shadow-md' : 'text-gray-500'
            }`}
          >
            <Lock size={16} />
            密码 / 短信登录
          </button>
        </div>

        {mode === 'qr' ? (
          <div className="custom-scrollbar flex min-h-0 flex-1 flex-col items-center overflow-y-auto px-6 pb-6 pt-6 sm:px-7">
            {qrData ? (
              <div
                className="grid h-[188px] w-[188px] shrink-0 place-items-center overflow-hidden rounded-[1.75rem] border border-pink-100 bg-white p-3 shadow-inner"
                dangerouslySetInnerHTML={{ __html: qrData.qrcode_svg }}
              />
            ) : (
              <QrGlyph className="h-[168px] w-[168px] shrink-0 sm:h-[188px] sm:w-[188px]" />
            )}
            <p className="mt-4 text-sm font-bold text-gray-600">
              请使用 <span className="text-bili-pink">哔哩哔哩客户端</span> 扫码登录
            </p>
            <p className="mt-1 text-xs text-gray-400">
              {qrData ? `二维码约 ${Math.round(qrData.expires_in_sec / 60)} 分钟后过期` : '也可以直接导入 Cookie 文件'}
            </p>
            <button
              onClick={handleQrLogin}
              disabled={isQrLoading}
              className="mt-5 flex items-center gap-2 rounded-full bg-gradient-to-r from-bili-pink to-pink-500 px-7 py-3 text-sm font-black text-white shadow-lg shadow-pink-200 transition hover:scale-[1.02] disabled:cursor-not-allowed disabled:opacity-60"
            >
              <Smartphone size={16} />
              {qrData ? '刷新二维码' : isQrLoading ? '生成中...' : '生成扫码二维码'}
            </button>
            <button
              onClick={handleImportCookie}
              disabled={isImporting}
              className="mt-3 flex items-center gap-2 rounded-full border border-pink-100 bg-white px-6 py-2.5 text-sm font-black text-bili-pink shadow-sm transition hover:scale-[1.02] disabled:cursor-not-allowed disabled:opacity-60"
            >
              <FileJson size={15} />
              {isImporting ? '导入中...' : '导入 Cookie 文件'}
            </button>
          </div>
        ) : (
          <div className="custom-scrollbar min-h-0 flex-1 overflow-y-auto px-6 pb-6 pt-5 sm:px-7">
            <div className="flex gap-4 border-b border-gray-100 text-sm font-black">
              <button
                onClick={() => setPasswordMode('password')}
                className={`pb-2.5 ${passwordMode === 'password' ? 'border-b-2 border-bili-pink text-bili-pink' : 'text-gray-400'}`}
              >
                密码登录
              </button>
              <button
                onClick={() => setPasswordMode('sms')}
                className={`pb-2.5 ${passwordMode === 'sms' ? 'border-b-2 border-bili-pink text-bili-pink' : 'text-gray-400'}`}
              >
                短信登录
              </button>
            </div>

            <label className="mt-5 block text-sm font-black text-gray-600">手机号 / 邮箱 / 用户名</label>
            <div className="mt-2 flex items-center gap-3 rounded-2xl border border-gray-200 bg-slate-50 px-4 py-2.5 text-gray-400">
              <Mail size={17} />
              <input className="flex-1 bg-transparent text-sm outline-none" placeholder="请输入手机号/邮箱/用户名" />
            </div>

            <label className="mt-4 block text-sm font-black text-gray-600">{passwordMode === 'password' ? '密码' : '验证码'}</label>
            <div className="mt-2 flex items-center gap-3 rounded-2xl border border-gray-200 bg-slate-50 px-4 py-2.5 text-gray-400">
              <Lock size={17} />
              <input className="flex-1 bg-transparent text-sm outline-none" placeholder={passwordMode === 'password' ? '请输入登录密码' : '请输入短信验证码'} type={passwordMode === 'password' ? 'password' : 'text'} />
            </div>

            <div className="mt-5 rounded-2xl border border-gray-100 bg-white p-4 max-[760px]:p-3">
              <p className="text-xs font-black text-gray-500">安全验证 <span className="font-medium text-gray-400">请拖动滑块使拼图吻合</span></p>
              <div className="relative mt-3 h-24 overflow-hidden rounded-xl bg-slate-200 max-[760px]:h-[76px]">
                <div className="absolute left-4 top-8 grid grid-cols-3 gap-1 rounded-md bg-white p-1 shadow-md max-[760px]:top-6">
                  {Array.from({ length: 9 }).map((_, i) => <span key={i} className="h-2 w-2 rounded-full bg-bili-pink" />)}
                </div>
                <div className="absolute right-16 top-7 h-10 w-10 rounded-md bg-slate-600 max-[760px]:top-5" />
              </div>
              <div className="mt-2 flex h-10 items-center rounded-full border border-gray-200 bg-slate-50 text-xs font-bold text-gray-400 max-[760px]:h-9">
                <div className="ml-1 flex h-9 w-9 items-center justify-center rounded-full bg-white shadow max-[760px]:h-8 max-[760px]:w-8">
                  <RotateCcw size={16} />
                </div>
                <span className="flex-1 text-center">向右滑动拼图以验证</span>
              </div>
            </div>

            <button className="mt-5 w-full rounded-2xl bg-gradient-to-r from-bili-pink to-pink-500 py-3.5 text-base font-black text-white shadow-lg shadow-pink-200">
              登录
            </button>
          </div>
        )}

        {message && (
          <p className="mx-6 mb-6 mt-3 shrink-0 rounded-xl bg-pink-50 px-4 py-2 text-center text-xs font-bold text-bili-pink sm:mx-7">
            {message}
          </p>
        )}
      </section>
    </div>
  );
}
