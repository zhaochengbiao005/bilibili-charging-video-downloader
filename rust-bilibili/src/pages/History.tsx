import React, { useState, useEffect } from 'react';
import { AlertCircle, Clock, FolderOpen, Search, Trash2, X } from 'lucide-react';
import type { HistoryItem } from '../types';
import * as Bridge from '../bridge';

export function History() {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [search, setSearch] = useState('');
  const [loaded, setLoaded] = useState(false);
  const [notice, setNotice] = useState<{ id: string; title: string; message: string } | null>(null);

  useEffect(() => {
    Bridge.getHistory().then(h => {
      setItems(h);
      setLoaded(true);
    });
  }, []);

  const filtered = items.filter(i =>
    i.title.toLowerCase().includes(search.toLowerCase()) ||
    i.bvid.toLowerCase().includes(search.toLowerCase())
  );

  const handleDelete = async (id: string) => {
    await Bridge.deleteHistoryItem(id);
    setItems(prev => prev.filter(i => i.id !== id));
  };

  const handleClear = async () => {
    await Bridge.clearHistory();
    setItems([]);
  };

  const handleOpen = async (item: HistoryItem) => {
    try {
      await Bridge.openPath(item.output_path);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setNotice({
        id: item.id,
        title: '文件已经不在原位置',
        message: message.replace(/^本地文件错误:\s*/, ''),
      });
    }
  };

  const handleRemoveMissing = async () => {
    if (!notice) return;
    await handleDelete(notice.id);
    setNotice(null);
  };

  if (!loaded) {
    return (
      <div className="flex items-center justify-center min-h-[50vh] text-gray-400">
        <Clock size={32} className="animate-pulse" />
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto w-full p-8 md:p-12 flex flex-col gap-8 min-h-full">
      {notice && (
        <div
          className="fixed inset-0 z-40 flex items-center justify-center px-5 py-8"
          role="dialog"
          aria-modal="true"
          aria-label={notice.title}
          onClick={() => setNotice(null)}
        >
          <div className="absolute inset-0 bg-[#1F2937]/18 backdrop-blur-sm" />
          <div
            className="relative w-full max-w-[430px] overflow-hidden rounded-[2rem] border border-white/90 bg-white/92 p-6 shadow-[0_24px_70px_rgba(31,41,55,0.13),0_18px_44px_rgba(255,143,179,0.18)]"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              onClick={() => setNotice(null)}
              className="absolute right-4 top-4 flex h-9 w-9 items-center justify-center rounded-full bg-white/70 text-gray-400 shadow-sm transition-all hover:bg-white hover:text-bili-pink"
              aria-label="关闭提示"
            >
              <X size={18} />
            </button>
            <div className="mb-5 flex h-14 w-14 items-center justify-center rounded-2xl bg-gradient-to-br from-[#FFE1EC] to-[#EAF7FF] text-bili-pink shadow-[0_12px_28px_rgba(255,143,179,0.18)]">
              <AlertCircle size={28} />
            </div>
            <h2 className="text-xl font-black text-[#1F2937]">{notice.title}</h2>
            <p className="mt-3 break-words text-sm font-medium leading-6 text-gray-500">
              {notice.message}
            </p>
            <div className="mt-6 flex justify-end gap-3">
              <button
                type="button"
                onClick={() => setNotice(null)}
                className="rounded-2xl border border-white/90 bg-white/70 px-5 py-2.5 text-sm font-bold text-gray-500 shadow-sm transition-all hover:bg-white hover:text-[#1F2937]"
              >
                知道了
              </button>
              <button
                type="button"
                onClick={handleRemoveMissing}
                className="rounded-2xl bg-gradient-to-r from-[#FF9FC0] to-[#FF86B2] px-5 py-2.5 text-sm font-bold text-white shadow-[0_12px_26px_rgba(255,143,179,0.28)] transition-all hover:brightness-105 hover:shadow-[0_16px_34px_rgba(255,143,179,0.36)]"
              >
                删除这条记录
              </button>
            </div>
          </div>
        </div>
      )}

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-black tracking-tight text-gray-900">下载历史</h1>
          <p className="text-gray-500 font-medium mt-1">共 {items.length} 条记录</p>
        </div>
        {items.length > 0 && (
          <button
            onClick={handleClear}
            className="px-5 py-2.5 rounded-2xl bg-white/60 border border-white/80 text-gray-500 hover:text-red-500 font-bold text-sm transition-all hover:bg-red-50/50"
          >
            清空历史
          </button>
        )}
      </div>

      {items.length > 0 && (
        <div className="relative">
          <Search size={18} className="absolute left-4 top-1/2 -translate-y-1/2 text-gray-400" />
          <input
            type="text"
            placeholder="搜索历史记录..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full bg-white/50 border border-white/80 rounded-2xl py-3 pl-12 pr-4 focus:outline-none focus:ring-2 focus:ring-bili-pink/30 text-sm shadow-sm backdrop-blur-sm text-gray-700 font-medium placeholder-gray-400"
          />
        </div>
      )}

      {filtered.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-24 text-gray-400 glass-panel border-dashed rounded-[2rem] opacity-70">
          <Clock size={48} className="mb-4 opacity-50" />
          <p className="font-bold text-gray-500">
            {items.length === 0 ? '暂无下载记录' : '未找到匹配记录'}
          </p>
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          {filtered.map(item => (
            <div
              key={item.id}
              className="glass-panel rounded-2xl p-5 flex items-center gap-4 group hover:bg-white/70 transition-all"
            >
              <div className="flex-1 min-w-0">
                <h3 className="font-bold text-gray-900 truncate">{item.title}</h3>
                <div className="flex items-center gap-3 mt-1.5 text-xs text-gray-500">
                  <span>{item.bvid}</span>
                  <span className="w-1 h-1 rounded-full bg-gray-300" />
                  <span>{item.quality}</span>
                  <span className="w-1 h-1 rounded-full bg-gray-300" />
                  <span>{item.format?.toUpperCase()}</span>
                  <span className="w-1 h-1 rounded-full bg-gray-300" />
                  <span>{item.timestamp}</span>
                </div>
              </div>

              <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={() => handleOpen(item)}
                  className="w-9 h-9 rounded-xl bg-white/70 border border-white flex items-center justify-center text-gray-400 hover:text-bili-pink transition-all"
                  title="打开文件夹"
                >
                  <FolderOpen size={16} />
                </button>
                <button
                  onClick={() => handleDelete(item.id)}
                  className="w-9 h-9 rounded-xl bg-white/70 border border-white flex items-center justify-center text-gray-400 hover:text-red-500 transition-all"
                  title="删除记录"
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
