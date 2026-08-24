import React, { useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { Settings, X } from 'lucide-react';
import { ConfigManager } from './ConfigManager';
import { UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus } from '../types';

/**
 * 配置弹窗壳
 *
 * 设计要点：
 * - **完全固定尺寸**：弹窗宽度 896px、高度 640px，切换分类时大小零跳变
 * - 小屏幕兜底：max-h-[90vh] / max-w-[calc(100vw-2rem)] 防止溢出
 * - 标题（"系统配置"）与下方内容分开布局，标题固定不滚动
 * - 不锁定 body 滚动，避免 Tauri Webview viewport 重算导致的应用顶部标题栏抖动
 * - 背景遮罩层接住 wheel/touchmove 事件，阻止滚动穿透到下方页面
 * - 弹窗内容超出固定高度时仅主体部分滚动（flex 布局天然支持）
 *
 * @see ConfigManager.tsx 当前完整功能
 * @see docs/plans/2026-07-13-config-ui-simplification-design.md 拆分设计稿（待落地）
 */
export interface ConfigManagerModalProps {
    isOpen: boolean;
    onClose: () => void;

    // 与 ConfigManager 完全相同的 props（透传）
    updateRuntimeInfo: UpdateRuntimeInfo | null;
    updateStatus: UpdateStatus;
    updateCheckResult: UpdateCheckResult | null;
    onCheckForUpdates: () => Promise<void>;
}

export const ConfigManagerModal: React.FC<ConfigManagerModalProps> = ({
    isOpen,
    onClose,
    updateRuntimeInfo,
    updateStatus,
    updateCheckResult,
    onCheckForUpdates,
}) => {
    const overlayRef = useRef<HTMLDivElement>(null);

    // ESC 键关闭
    useEffect(() => {
        if (!isOpen) return;
        const handleEsc = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                onClose();
            }
        };
        window.addEventListener('keydown', handleEsc);
        return () => window.removeEventListener('keydown', handleEsc);
    }, [isOpen, onClose]);

    /**
     * 阻止滚轮事件穿透到 body（避免背景页面被滚动）
     *
     * 不直接锁定 body 滚动 — Tauri Webview 中 body { overflow: hidden }
     * 会触发 viewport 重算，可能导致应用顶部 sticky 标题栏抖动。
     * 仅在遮罩层上 preventDefault 即可阻止穿透，且不改变页面布局。
     */
    useEffect(() => {
        if (!isOpen) return;
        const overlay = overlayRef.current;
        if (!overlay) return;

        const stopWheel = (e: WheelEvent) => {
            // 仅阻止事件冒泡，不阻止弹窗内部滚动
            e.stopPropagation();
        };
        const stopTouch = (e: TouchEvent) => {
            e.stopPropagation();
        };

        overlay.addEventListener('wheel', stopWheel, { passive: true });
        overlay.addEventListener('touchmove', stopTouch, { passive: true });

        return () => {
            overlay.removeEventListener('wheel', stopWheel);
            overlay.removeEventListener('touchmove', stopTouch);
        };
    }, [isOpen]);

    if (!isOpen) return null;

    return createPortal(
        <div
            ref={overlayRef}
            className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm"
            onClick={onClose}
        >
            <div
                className="bg-surface border border-base rounded-card shadow-ds-modal w-full max-w-[min(896px,calc(100vw-2rem))] h-[640px] max-h-[90vh] flex flex-col overflow-hidden"
                onClick={(e) => e.stopPropagation()}
            >
                {/* 标题区：固定不滚动，与下方内容明确分开 */}
                <div className="px-6 py-4 flex items-center justify-between border-b border-base flex-shrink-0">
                    <div className="flex items-center gap-2">
                        <Settings className="w-5 h-5 text-primary" />
                        <h2 className="text-lg font-semibold text-main">系统配置</h2>
                    </div>
                    <button
                        onClick={onClose}
                        className="text-muted hover:text-main hover:bg-base p-1.5 rounded-lg transition-colors"
                        aria-label="关闭"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* 内容区：独立滚动，标题保持不动 */}
                <div className="flex-1 overflow-y-auto px-6 py-4 min-h-0">
                    <ConfigManager
                        updateRuntimeInfo={updateRuntimeInfo}
                        updateStatus={updateStatus}
                        updateCheckResult={updateCheckResult}
                        onCheckForUpdates={onCheckForUpdates}
                    />
                </div>
            </div>
        </div>,
        document.body,
    );
};
