import React, { useEffect, useState } from 'react';
import { LogOut, Minus } from 'lucide-react';
import type { CloseBehavior } from '../types';

export interface CloseConfirmDialogProps {
  isOpen: boolean;
  /** 用户确认关闭方式；remember 为 true 表示记住选择，后续不再询问 */
  onConfirm: (behavior: Exclude<CloseBehavior, 'ask'>, remember: boolean) => void;
  /** 取消关闭，回到主界面 */
  onCancel: () => void;
  /** 保存选择中，避免重复提交 */
  submitting?: boolean;
}

/**
 * 关闭确认弹窗：首次点击关闭按钮时询问「最小化到托盘」还是「直接退出」。
 *
 * 勾选「记住我的选择」后会把行为写入配置，后续点击关闭不再询问；
 * 用户可随时在「设置 - 自定义配置」中改回每次询问。
 */
export const CloseConfirmDialog: React.FC<CloseConfirmDialogProps> = ({
  isOpen,
  onConfirm,
  onCancel,
  submitting = false,
}) => {
  const [remember, setRemember] = useState(true);

  // 每次重新打开都回到默认勾选状态，避免上一次的取消动作影响下一次判断
  useEffect(() => {
    if (isOpen) {
      setRemember(true);
    }
  }, [isOpen]);

  // ESC 取消关闭，与配置弹窗保持一致的键盘习惯
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !submitting) {
        onCancel();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, submitting, onCancel]);

  if (!isOpen) return null;

  return (
    <>
      <div className="fixed inset-0 bg-ds-scrim backdrop-blur-sm z-[130]" onClick={submitting ? undefined : onCancel} />
      <div className="fixed inset-0 z-[131] flex items-center justify-center p-4 pointer-events-none">
        <div
          className="bg-surface rounded-xl border border-base shadow-ds-stack p-6 max-w-sm w-full pointer-events-auto"
          role="dialog"
          aria-modal="true"
          aria-labelledby="close-confirm-title"
        >
          <h3 id="close-confirm-title" className="text-base font-bold text-main mb-2">
            要如何关闭剑网三副本管家？
          </h3>
          <p className="text-sm text-muted mb-5">
            最小化到托盘后程序在后台继续运行，点击托盘图标即可恢复窗口。
          </p>

          <div className="space-y-2 mb-5">
            <button
              type="button"
              disabled={submitting}
              onClick={() => onConfirm('minimize', remember)}
              className="w-full flex items-center gap-3 p-3 rounded-card border border-ds-border hover:border-ds-accent bg-ds-surface text-left transition-colors disabled:opacity-50"
            >
              <span className="w-9 h-9 flex items-center justify-center rounded-card bg-ds-accent-soft text-ds-accent-strong flex-shrink-0">
                <Minus className="w-4 h-4" />
              </span>
              <span className="flex-1 min-w-0">
                <span className="block text-sm font-medium text-ds-fg">最小化到托盘</span>
                <span className="block text-xs text-ds-fg-mute mt-0.5">后台继续运行，托盘图标可恢复</span>
              </span>
            </button>

            <button
              type="button"
              disabled={submitting}
              onClick={() => onConfirm('exit', remember)}
              className="w-full flex items-center gap-3 p-3 rounded-card border border-ds-border hover:border-ds-accent bg-ds-surface text-left transition-colors disabled:opacity-50"
            >
              <span className="w-9 h-9 flex items-center justify-center rounded-card bg-ds-warning-soft text-ds-warning-strong flex-shrink-0">
                <LogOut className="w-4 h-4" />
              </span>
              <span className="flex-1 min-w-0">
                <span className="block text-sm font-medium text-ds-fg">直接退出</span>
                <span className="block text-xs text-ds-fg-mute mt-0.5">结束程序，后台扫描同时停止</span>
              </span>
            </button>
          </div>

          <label className="flex items-start gap-2 text-xs text-muted mb-5 cursor-pointer">
            <input
              type="checkbox"
              checked={remember}
              disabled={submitting}
              onChange={event => setRemember(event.target.checked)}
              className="mt-0.5 w-3.5 h-3.5 accent-ds-accent-strong flex-shrink-0"
            />
            <span>记住我的选择，下次不再询问（可在「设置 - 自定义配置」改回每次询问）</span>
          </label>

          <div className="flex justify-end gap-2">
            <button type="button" className="btn btn-secondary text-sm" onClick={onCancel} disabled={submitting}>
              取消
            </button>
          </div>
        </div>
      </div>
    </>
  );
};
