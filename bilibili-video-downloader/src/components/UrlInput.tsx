import React, { useState, useCallback } from 'react';
import { Link2, AlertCircle, FileText, QrCode } from 'lucide-react';
import * as Bridge from '../bridge';

interface UrlInputProps {
  onParse: (url: string) => void;
  isParsing: boolean;
  error?: string | null;
  cookiePath: string;
  onCookieChange: (path: string) => void;
}

export function UrlInput({ onParse, isParsing, error, cookiePath, onCookieChange }: UrlInputProps) {
  const [url, setUrl] = useState('');
  const [cookieInput, setCookieInput] = useState(cookiePath);
  const [loginState, setLoginState] = useState<'idle' | 'logging' | 'done'>('idle');

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;
    onParse(url);
  }, [url, onParse]);

  const handleCookieBrowse = useCallback(() => {
    // PyWebView doesn't have file dialog, use a hidden input
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json,.txt';
    input.onchange = (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        // For PyWebView, we need the actual path, not a blob URL
        // The path is sent to Python backend
        const path = (file as any).path || file.name;
        setCookieInput(path);
        onCookieChange(path);
      }
    };
    input.click();
  }, [onCookieChange]);

  const handleQrLogin = useCallback(async () => {
    setLoginState('logging');
    try {
      const result = await Bridge.qrLogin();
      if (result.path) {
        setCookieInput(result.path);
        onCookieChange(result.path);
        setLoginState('done');
      }
    } catch (err) {
      console.error('Login failed:', err);
    }
    // Don't reset state — let parent decide
  }, [onCookieChange]);

  return (
    <div className="w-full relative flex flex-col">
      <form onSubmit={handleSubmit} className="w-full flex flex-col gap-2">
        {/* 视频链接行 */}
        <div className="flex items-center gap-3">
          <div className="flex items-center text-gray-400 pl-2">
            <Link2 size={20} strokeWidth={2.5} />
          </div>
          <input
            type="text"
            className="flex-1 bg-transparent border-none focus:outline-none focus:ring-0 py-4 text-base text-gray-800 font-medium placeholder-gray-400"
            placeholder="https://www.bilibili.com/video/BV..."
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            disabled={isParsing}
          />
          <button
            type="submit"
            disabled={isParsing || !url.trim()}
            className="bg-gradient-to-r from-[#fb7299] to-[#ff85a8] text-white px-8 py-3.5 rounded-full font-bold shadow-md shadow-pink-200 hover:shadow-lg hover:shadow-pink-300 transition-all disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-2 m-1 shrink-0"
          >
            {isParsing ? '解析中...' : <>解析 <span className="text-xl leading-none">→</span></>}
          </button>
        </div>

        {/* Cookie 行 */}
        <div className="flex items-center gap-3 px-2 pb-2">
          <div className="flex items-center text-gray-400">
            <FileText size={16} strokeWidth={2.5} />
          </div>
          <input
            type="text"
            className="flex-1 bg-transparent border border-white/40 rounded-xl px-3 py-2 text-xs text-gray-600 font-medium placeholder-gray-400 focus:outline-none focus:border-bili-pink/50"
            placeholder="Cookie 文件路径（可选）"
            value={cookieInput}
            onChange={(e) => {
              setCookieInput(e.target.value);
              onCookieChange(e.target.value);
            }}
          />
          <button
            type="button"
            onClick={handleCookieBrowse}
            className="px-4 py-2 bg-white/60 border border-white/80 rounded-xl text-xs font-bold text-gray-500 hover:text-bili-pink transition-all whitespace-nowrap"
          >
            浏览
          </button>
          <button
            type="button"
            onClick={handleQrLogin}
            disabled={loginState === 'logging'}
            className="px-4 py-2 bg-gradient-to-r from-[#00a1d6] to-[#40c5f1] text-white rounded-xl text-xs font-bold shadow-sm hover:shadow-md transition-all whitespace-nowrap disabled:opacity-60 flex items-center gap-1.5"
          >
            <QrCode size={14} />
            {loginState === 'logging' ? '登录中...' : '扫码登录'}
          </button>
        </div>
      </form>

      {error && (
        <div className="absolute top-full left-1/2 -translate-x-1/2 mt-4 text-sm text-red-500 font-bold bg-red-50 px-5 py-2.5 rounded-xl flex items-center gap-2 shadow-sm border border-red-100 whitespace-nowrap z-50">
          <AlertCircle size={16} strokeWidth={2.5} />
          {error}
        </div>
      )}
    </div>
  );
}
