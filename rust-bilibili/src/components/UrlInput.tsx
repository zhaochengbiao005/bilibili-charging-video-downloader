import React, { useState, useCallback } from 'react';
import { Link2, AlertCircle } from 'lucide-react';

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
            className="bg-gradient-to-r from-bili-pink to-bili-pink-hover text-white px-8 py-3.5 rounded-full font-bold shadow-[0_10px_24px_rgba(255,143,179,0.34)] hover:shadow-[0_14px_30px_rgba(255,143,179,0.42)] transition-all disabled:opacity-60 disabled:cursor-not-allowed flex items-center gap-2 m-1 shrink-0"
          >
            {isParsing ? '解析中...' : <>解析 <span className="text-xl leading-none">→</span></>}
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
