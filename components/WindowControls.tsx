import React from 'react';
import { Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** 是否为 Tauri 环境（非浏览器/PWA 模式） */
const isTauriEnv = (): boolean =>
  typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;

/**
 * 自定义标题栏的窗口控制按钮（最小化 / 最大化 / 关闭）。
 *
 * 仅在 Tauri 环境下显示（浏览器/PWA 模式返回 null）。
 * 样式贴近 Windows 原生标题栏按钮：占满标题栏高度、紧密排列、无圆角。
 * 关闭按钮悬停仅图标变红 + 轻微红色底色，不使用实心色块。
 */
export const WindowControls: React.FC = () => {
  if (!isTauriEnv()) return null;

  const appWindow = getCurrentWindow();

  // 显式区分最大化/还原：部分 Windows 环境下 toggleMaximize 不可靠
  const handleToggleMaximize = async () => {
    try {
      if (await appWindow.isMaximized()) {
        await appWindow.unmaximize();
      } else {
        await appWindow.maximize();
      }
    } catch (error) {
      console.error('切换最大化状态失败:', error);
    }
  };

  return (
    <div className="flex items-stretch h-full app-region-no-drag border-l border-border">
      <button
        onClick={() => appWindow.minimize()}
        title="最小化"
        className="w-11 h-full flex items-center justify-center text-muted hover:bg-base hover:text-main transition-colors"
      >
        <Minus size={15} />
      </button>
      <button
        onClick={handleToggleMaximize}
        title="最大化 / 还原"
        className="w-11 h-full flex items-center justify-center text-muted hover:bg-base hover:text-main transition-colors"
      >
        <Square size={12} />
      </button>
      <button
        onClick={() => appWindow.close()}
        title="关闭"
        className="w-11 h-full flex items-center justify-center text-muted hover:bg-red-500/10 hover:text-red-500 transition-colors"
      >
        <X size={16} />
      </button>
    </div>
  );
};
