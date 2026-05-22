import React, { useEffect, useRef, useState } from 'react';
import { AlertCircle, CheckCircle2, FileJson, Loader2, Lock, LogOut, QrCode, RefreshCw, Smartphone, X } from 'lucide-react';
import * as Bridge from '../bridge';
import mascot22 from '../assets/mascot-22.png';
import mascot33 from '../assets/mascot-33.png';
import defaultLoginAvatar from '../assets/user-login-default.jpg';
import type { LoginStatus, QrLoginStartResponse } from '../types';

interface LoginModalProps {
  isOpen: boolean;
  loginStatus: LoginStatus | null;
  onLoginChange: (status: LoginStatus) => void;
  onClose: () => void;
}

export function LoginModal({ isOpen, loginStatus, onLoginChange, onClose }: LoginModalProps) {
  const [mode, setMode] = useState<'qr' | 'password'>('qr');
  const [message, setMessage] = useState('');
  const [qrData, setQrData] = useState<QrLoginStartResponse | null>(null);
  const [isQrLoading, setIsQrLoading] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [avatarSrc, setAvatarSrc] = useState(defaultLoginAvatar);
  const qrRequestInFlight = useRef(false);

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
          window.setTimeout(onClose, 450);
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

  useEffect(() => {
    if (!isOpen) {
      qrRequestInFlight.current = false;
      setMode('qr');
      setMessage('');
      setQrData(null);
      setIsQrLoading(false);
      return;
    }

    if (mode !== 'qr' || loginStatus?.is_login || qrData || qrRequestInFlight.current) return;

    let stopped = false;
    qrRequestInFlight.current = true;
    setMessage('');
    setIsQrLoading(true);

    Bridge.startQrLogin()
      .then((data) => {
        if (stopped) return;
        setQrData(data);
        setMessage('二维码已生成，请在哔哩哔哩客户端确认登录。');
      })
      .catch((err) => {
        if (stopped) return;
        setMessage(err instanceof Error ? err.message : '扫码登录启动失败。');
      })
      .finally(() => {
        qrRequestInFlight.current = false;
        if (!stopped) setIsQrLoading(false);
      });

    return () => {
      stopped = true;
    };
  }, [isOpen, loginStatus?.is_login, mode, qrData]);

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

  const handleQrLogin = async () => {
    if (qrRequestInFlight.current) return;
    setMessage('');
    setQrData(null);
    qrRequestInFlight.current = true;
    setIsQrLoading(true);
    try {
      const data = await Bridge.startQrLogin();
      setQrData(data);
      setMessage('二维码已生成，请在哔哩哔哩客户端确认登录。');
    } catch (err) {
      setMessage(err instanceof Error ? err.message : '扫码登录启动失败。');
    } finally {
      qrRequestInFlight.current = false;
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
      if (status.is_login) window.setTimeout(onClose, 450);
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
    <div className="relative flex w-full max-h-[calc(100vh-1.5rem)]">
      <img
        src={mascot22}
        alt="22娘"
        className="pointer-events-none absolute -left-32 top-14 z-20 hidden h-60 w-48 object-contain drop-shadow-[0_22px_34px_rgba(37,99,235,0.22)] sm:block md:-left-52 md:top-10 md:h-80 md:w-64 lg:-left-60 lg:h-[22rem] lg:w-72"
      />
      <img
        src={mascot33}
        alt="33娘"
        className="pointer-events-none absolute -right-32 top-14 z-20 hidden h-60 w-48 object-contain drop-shadow-[0_22px_34px_rgba(255,143,179,0.24)] sm:block md:-right-52 md:top-10 md:h-80 md:w-64 lg:-right-60 lg:h-[22rem] lg:w-72"
      />

      <button
        onClick={onClose}
        className="absolute right-3 top-3 z-20 h-9 w-9 rounded-full border border-white bg-white/85 text-gray-500 shadow-lg transition hover:scale-105 hover:text-bili-pink"
        aria-label="关闭登录"
      >
        <X className="mx-auto" size={18} />
      </button>

      <section className="relative flex w-full max-h-[calc(100vh-1.5rem)] flex-col overflow-hidden rounded-[22px] border border-pink-100 bg-white/92 shadow-[0_24px_70px_rgba(31,38,135,0.22)] backdrop-blur-[28px]">
        <div className="shrink-0 px-6 pt-5 text-center sm:px-7">
          <h2 className="text-[26px] font-black tracking-tight text-bili-pink">
            {loginStatus?.is_login ? '账号信息' : '登录哔哩哔哩'}
          </h2>
          <p className="mt-1 text-xs font-medium text-gray-400">
            {loginStatus?.is_login
              ? `${loginStatus.username || '已登录'} · LV${loginStatus.level ?? 0}`
              : '解锁 1080P/4K 高清画质与高帧率视频下载'}
          </p>
        </div>

        {loginStatus?.is_login ? (
          <div className="flex min-h-[390px] flex-1 flex-col items-center justify-center px-7 pb-8 pt-6">
            <div className="relative">
              <img
                src={avatarSrc}
                alt="用户头像"
                className="h-24 w-24 rounded-full border-4 border-white object-cover shadow-[0_16px_38px_rgba(255,143,179,0.24)]"
                referrerPolicy="no-referrer"
                onError={() => setAvatarSrc(defaultLoginAvatar)}
              />
              <span className="absolute -bottom-1 -right-1 flex h-8 w-8 items-center justify-center rounded-full border-4 border-white bg-green-400 text-white shadow-sm">
                <CheckCircle2 size={16} strokeWidth={3} />
              </span>
            </div>
            <h3 className="mt-5 max-w-full truncate text-center text-2xl font-black text-gray-900">
              {loginStatus.username || '已登录用户'}
            </h3>
            <div className="mt-3 flex flex-wrap justify-center gap-2 text-xs font-black text-gray-500">
              {loginStatus.level !== null && loginStatus.level !== undefined && (
                <span className="rounded-full border border-pink-100 bg-pink-50 px-3 py-1.5 text-bili-pink">
                  LV{loginStatus.level}
                </span>
              )}
              {loginStatus.uid !== null && loginStatus.uid !== undefined && (
                <span className="rounded-full border border-blue-100 bg-blue-50 px-3 py-1.5 text-bili-blue">
                  UID {loginStatus.uid}
                </span>
              )}
              {loginStatus.vip_type !== null && loginStatus.vip_type !== undefined && loginStatus.vip_type > 0 && (
                <span className="rounded-full border border-yellow-100 bg-yellow-50 px-3 py-1.5 text-yellow-600">
                  大会员
                </span>
              )}
            </div>
            <div className="mt-6 w-full rounded-3xl border border-green-100 bg-green-50 px-5 py-4 text-center">
              <p className="flex items-center justify-center gap-2 text-sm font-black text-green-600">
                <CheckCircle2 size={17} strokeWidth={2.5} />
                当前已登录，可以直接解析和下载需要登录权限的视频
              </p>
            </div>
            <button
              onClick={handleLogout}
              className="mt-6 flex w-full items-center justify-center gap-2 rounded-2xl border border-pink-100 bg-white px-5 py-3.5 text-base font-black text-bili-pink shadow-[0_12px_26px_rgba(255,143,179,0.16)] transition hover:scale-[1.01] hover:bg-pink-50"
            >
              <LogOut size={18} strokeWidth={2.5} />
              退出登录
            </button>
          </div>
        ) : (
          <>
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
                className="grid h-[212px] w-[212px] shrink-0 place-items-center rounded-[1.75rem] border border-pink-100 bg-white p-3 shadow-inner [&_svg]:block [&_svg]:h-full [&_svg]:w-full"
                dangerouslySetInnerHTML={{ __html: qrData.qrcode_svg }}
              />
            ) : (
              <div className="grid h-[212px] w-[212px] shrink-0 place-items-center rounded-[1.75rem] border border-pink-100 bg-white p-4 text-center shadow-inner">
                {isQrLoading ? (
                  <div className="flex flex-col items-center gap-3 text-bili-pink">
                    <Loader2 className="animate-spin" size={34} />
                    <span className="text-sm font-black">正在生成真实二维码</span>
                  </div>
                ) : (
                  <div className="flex flex-col items-center gap-3 text-gray-400">
                    <AlertCircle size={34} />
                    <span className="text-sm font-black">二维码未生成</span>
                  </div>
                )}
              </div>
            )}
            <p className="mt-4 text-sm font-bold text-gray-600">
              请使用 <span className="text-bili-pink">哔哩哔哩客户端</span> 扫码登录
            </p>
            <p className="mt-1 text-xs text-gray-400">
              {qrData ? `二维码约 ${Math.round(qrData.expires_in_sec / 60)} 分钟后过期` : '打开登录后会自动获取真实扫码二维码'}
            </p>
            <button
              onClick={handleQrLogin}
              disabled={isQrLoading}
              className="mt-5 flex items-center gap-2 rounded-full bg-gradient-to-r from-bili-pink to-bili-pink-hover px-7 py-3 text-sm font-black text-white shadow-[0_14px_30px_rgba(255,143,179,0.32)] transition hover:scale-[1.02] disabled:cursor-not-allowed disabled:opacity-60"
            >
              {qrData ? <RefreshCw size={16} /> : <Smartphone size={16} />}
              {qrData ? '刷新二维码' : isQrLoading ? '生成中...' : '重新生成二维码'}
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
          <div className="custom-scrollbar min-h-0 flex-1 overflow-y-auto px-6 pb-6 pt-6 sm:px-7">
            <div className="rounded-3xl border border-pink-100 bg-pink-50/70 p-5 text-center">
              <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-white text-bili-pink shadow-sm">
                <AlertCircle size={22} />
              </div>
              <h3 className="mt-4 text-base font-black text-gray-800">密码 / 短信登录暂未接入</h3>
              <p className="mt-2 text-sm font-medium leading-relaxed text-gray-500">
                B站网页密码登录需要真实风控验证、短信发送和加密登录流程。当前版本只开放可验证的扫码登录和 Cookie 文件导入，避免展示无法工作的假表单。
              </p>
            </div>

            <button
              onClick={() => setMode('qr')}
              className="mt-5 w-full rounded-2xl bg-gradient-to-r from-bili-pink to-bili-pink-hover py-3.5 text-base font-black text-white shadow-[0_14px_30px_rgba(255,143,179,0.32)]"
            >
              返回扫码 / Cookie 登录
            </button>
          </div>
            )}
          </>
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
