import React, { useState, useCallback } from 'react';
import { AlertCircle, ArrowRight, Download, Link2, Loader2 } from 'lucide-react';

interface UrlInputProps {
  onParse: (url: string) => void;
  isParsing: boolean;
  error?: string | null;
}

export function UrlInput({ onParse, isParsing, error }: UrlInputProps) {
  const [url, setUrl] = useState('');

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;
    onParse(url);
  }, [url, onParse]);

  return (
    <div className="w-full relative flex flex-col">
      <form onSubmit={handleSubmit} className="w-full">
        <div className="group flex min-h-[78px] items-center gap-4 rounded-[2.4rem] border border-white/90 bg-white/82 p-2 pl-8 shadow-[0_28px_70px_rgba(255,143,179,0.16),0_18px_52px_rgba(123,207,255,0.11),inset_0_1px_0_rgba(255,255,255,0.92)] backdrop-blur-[28px] transition-all duration-300 focus-within:border-pink-100 focus-within:bg-white/92 focus-within:shadow-[0_32px_78px_rgba(255,143,179,0.19),0_20px_56px_rgba(123,207,255,0.13),inset_0_1px_0_rgba(255,255,255,0.95)]">
          <div className="flex shrink-0 items-center justify-center text-slate-400 transition-colors duration-300 group-focus-within:text-bili-pink">
            <Link2 size={23} strokeWidth={2.7} />
          </div>
          <textarea
            rows={1}
            className="min-h-[58px] min-w-0 flex-1 resize-none bg-transparent py-4 text-[18px] font-semibold leading-7 text-gray-800 placeholder:text-slate-400/85 outline-none disabled:cursor-not-allowed disabled:opacity-70"
            placeholder="粘贴一个或多个 B站视频链接"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={(e) => {
              if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
                e.currentTarget.form?.requestSubmit();
              }
            }}
            disabled={isParsing}
          />
          <button
            type="submit"
            disabled={isParsing || !url.trim()}
            className="flex h-[62px] shrink-0 items-center justify-center gap-2 rounded-[2rem] bg-gradient-to-r from-[#FF9FC0] to-[#FF86B2] px-5 text-[16px] font-black text-white shadow-[0_12px_28px_rgba(255,134,178,0.36),inset_0_1px_0_rgba(255,255,255,0.24)] transition-all duration-300 hover:brightness-105 hover:shadow-[0_18px_42px_rgba(255,134,178,0.5),0_0_24px_rgba(255,159,192,0.18),inset_0_1px_0_rgba(255,255,255,0.3)] active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:brightness-100 disabled:hover:shadow-[0_12px_28px_rgba(255,134,178,0.36),inset_0_1px_0_rgba(255,255,255,0.24)] sm:min-w-[220px] sm:gap-3 sm:px-10 sm:text-[18px]"
          >
            {isParsing ? (
              <>
                <Loader2 size={22} strokeWidth={2.6} className="animate-spin" />
                解析中
              </>
            ) : (
              <>
                <Download size={24} strokeWidth={3} />
                立即解析
                <ArrowRight size={23} strokeWidth={3} />
              </>
            )}
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
