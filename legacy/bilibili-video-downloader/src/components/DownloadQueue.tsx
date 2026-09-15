import React from 'react';
import { X, Download, AlertCircle, CheckCircle, PauseCircle } from 'lucide-react';
import type { DownloadTask } from '../types';

interface DownloadQueueProps {
  tasks: DownloadTask[];
  onRemove: (id: string) => void;
  onCancel?: (id: string) => void;
}

const statusConfig: Record<string, { icon: React.ReactNode; color: string; bg: string }> = {
  downloading: {
    icon: <Download size={14} />,
    color: 'text-bili-pink',
    bg: 'bg-gradient-to-r from-[#fb7299] to-[#ff85a8]',
  },
  completed: {
    icon: <CheckCircle size={14} />,
    color: 'text-green-500',
    bg: 'bg-green-400',
  },
  error: {
    icon: <AlertCircle size={14} />,
    color: 'text-red-500',
    bg: 'bg-red-400',
  },
  cancelled: {
    icon: <PauseCircle size={14} />,
    color: 'text-gray-400',
    bg: 'bg-gray-300',
  },
};

export function DownloadQueue({ tasks, onRemove, onCancel }: DownloadQueueProps) {
  if (tasks.length === 0) return null;

  return (
    <div className="glass-panel rounded-[2rem] p-6 flex flex-col gap-4">
      <h3 className="font-bold text-gray-900 border-b border-gray-200/50 pb-3">
        下载队列 ({tasks.length})
      </h3>

      <div className="flex flex-col gap-3 max-h-72 overflow-y-auto pr-2 custom-scrollbar">
        {tasks.map((task) => {
          const cfg = statusConfig[task.status] || statusConfig.downloading;

          return (
            <div
              key={task.id}
              className="bg-white/40 border border-white/60 rounded-xl p-4 flex flex-col gap-3 relative group shadow-sm backdrop-blur-sm"
            >
              {/* 顶部：标题 + 状态 */}
              <div className="flex justify-between items-start gap-4">
                <div className="flex items-center gap-2 min-w-0">
                  <span className={cfg.color}>{cfg.icon}</span>
                  <span className="text-sm font-bold text-gray-800 truncate">{task.title}</span>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <span className={`text-xs font-bold font-mono ${cfg.color}`}>
                    {task.status === 'downloading' ? `${Math.round(task.progress)}%` :
                     task.status === 'completed' ? '完成' :
                     task.status === 'error' ? '失败' : '已取消'}
                  </span>
                  {/* Remove/Cancel button */}
                  {task.status === 'downloading' && onCancel ? (
                    <button
                      onClick={() => onCancel(task.id)}
                      className="w-5 h-5 rounded-full bg-white/70 border border-gray-200 flex items-center justify-center text-gray-400 hover:text-orange-500 transition-colors"
                      title="取消"
                    >
                      <PauseCircle size={12} />
                    </button>
                  ) : (
                    <button
                      onClick={() => onRemove(task.id)}
                      className="w-5 h-5 rounded-full bg-white/70 border border-gray-200 flex items-center justify-center text-gray-400 hover:text-red-500 transition-colors opacity-0 group-hover:opacity-100"
                      title="移除"
                    >
                      <X size={12} />
                    </button>
                  )}
                </div>
              </div>

              {/* 进度条 */}
              <div className="flex flex-col gap-1.5">
                <div className="w-full h-2 bg-gray-200/50 rounded-full overflow-hidden shadow-inner border border-white/50">
                  <div
                    className={`h-full transition-all duration-300 ${cfg.bg}`}
                    style={{ width: `${task.progress}%` }}
                  />
                </div>
                <div className="flex items-center justify-between text-[10px] text-gray-500 font-medium px-1">
                  <span>{task.speed || (task.status === 'downloading' ? `${(Math.random() * 8 + 2).toFixed(1)} MB/s` : '')}</span>
                  <span>{task.quality} · {task.format?.toUpperCase()}</span>
                </div>
              </div>

              {/* 错误信息 */}
              {task.status === 'error' && task.error_message && (
                <div className="text-xs text-red-500 bg-red-50 rounded-lg px-3 py-1.5 border border-red-100">
                  {task.error_message}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
