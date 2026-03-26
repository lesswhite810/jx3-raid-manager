import React from 'react';
import { createPortal } from 'react-dom';
import { Download, ExternalLink, RefreshCw, X } from 'lucide-react';
import { UpdateCheckResult, UpdateStatus } from '../types';
import { formatUpdatePubDate } from '../utils/updaterUtils';

interface UpdateDialogProps {
  isOpen: boolean;
  updateInfo: UpdateCheckResult | null;
  updateStatus: UpdateStatus;
  downloadedBytes: number;
  totalBytes?: number;
  onClose: () => void;
  onConfirm: () => void;
}

export const UpdateDialog: React.FC<UpdateDialogProps> = ({
  isOpen,
  updateInfo,
  updateStatus,
  downloadedBytes,
  totalBytes,
  onClose,
  onConfirm
}) => {
  if (!isOpen || !updateInfo) {
    return null;
  }

  const publishedAtText = formatUpdatePubDate(updateInfo.pubDate);
  const percent = totalBytes && totalBytes > 0
    ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
    : null;
  const isBusy = updateStatus === 'downloading' || updateStatus === 'installing';
  const isPortable = updateInfo.isPortable;

  return createPortal(
    <>
      <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]" onClick={isBusy ? undefined : onClose} />
      <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
        <div className="w-full max-w-xl bg-surface border border-base rounded-xl shadow-2xl overflow-hidden pointer-events-auto">
          <div className="px-6 py-4 border-b border-base flex items-center justify-between">
            <div>
              <h2 className="text-lg font-bold text-main">发现新版本</h2>
              <p className="text-xs text-muted mt-0.5">
                当前版本 v{updateInfo.currentVersion}，最新版本 v{updateInfo.version}
              </p>
            </div>
            <button
              onClick={onClose}
              disabled={isBusy}
              className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50 disabled:opacity-50"
            >
              <X className="w-5 h-5" />
            </button>
          </div>

          <div className="p-6 space-y-5">
            <div className="rounded-lg border border-base bg-base/40 p-4">
              <div className="flex flex-wrap items-center gap-3 text-sm">
                <span className="px-2.5 py-1 rounded-full bg-surface border border-base text-main">
                  {isPortable ? '便携版' : '安装版'}
                </span>
                {publishedAtText && (
                  <span className="text-muted">
                    发布时间：{publishedAtText}
                  </span>
                )}
              </div>
              <p className="text-sm text-main mt-3">
                {isPortable
                  ? '当前运行的是便携版，应用不会自动安装更新，将为你打开 GitHub Release 下载页面。'
                  : '确认后将下载安装更新，并按当前安装路径执行升级安装。安装程序启动后，应用会退出并交由安装器完成更新；升级完成后可在完成页选择是否立即运行，且默认已勾选。'}
              </p>
            </div>

            <div>
              <h3 className="text-sm font-semibold text-main mb-2">更新说明</h3>
              <div className="max-h-64 overflow-y-auto rounded-lg border border-base bg-base/30 p-4 whitespace-pre-wrap text-sm text-main leading-6">
                {updateInfo.body}
              </div>
            </div>

            {isBusy && (
              <div className="rounded-lg border border-base bg-base/30 p-4">
                <div className="flex items-center justify-between text-sm text-main mb-2">
                  <span>{updateStatus === 'installing' ? '正在安装更新' : '正在下载更新'}</span>
                  <span>{percent !== null ? `${percent}%` : '准备中'}</span>
                </div>
                <div className="w-full h-2 rounded-full bg-base overflow-hidden">
                  <div
                    className="h-full bg-emerald-500 transition-all duration-300"
                    style={{ width: `${percent ?? 8}%` }}
                  />
                </div>
              </div>
            )}
          </div>

          <div className="px-6 py-4 border-t border-base flex items-center justify-end gap-3 bg-base/20">
            <button
              onClick={onClose}
              disabled={isBusy}
              className="px-4 py-2 text-sm text-muted hover:text-main transition-colors disabled:opacity-50"
            >
              稍后再说
            </button>
            <button
              onClick={onConfirm}
              disabled={isBusy}
              className="btn btn-primary flex items-center gap-2 disabled:opacity-60 disabled:cursor-not-allowed"
            >
              {isBusy ? (
                <RefreshCw className="w-4 h-4 animate-spin" />
              ) : isPortable ? (
                <ExternalLink className="w-4 h-4" />
              ) : (
                <Download className="w-4 h-4" />
              )}
              <span>{isBusy ? '处理中...' : isPortable ? '前往下载' : '立即更新'}</span>
            </button>
          </div>
        </div>
      </div>
    </>,
    document.body
  );
};
