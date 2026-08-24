import React, { useCallback, useRef } from 'react';
import { Download, RefreshCw } from 'lucide-react';
import type { UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus } from '../../types';
import { useDebug } from '../../contexts/DebugContext';
import { toast } from '../../utils/toastManager';

/** Debug 模式：连续点击次数阈值，达到后切换 */
const DEBUG_TOGGLE_CLICK_COUNT = 7;
/** 连续点击的有效间隔（毫秒），超时重置计数 */
const DEBUG_TOGGLE_CLICK_WINDOW_MS = 1500;

/**
 * 格式化发布日期为本地化短日期字符串（中文 zh-CN）
 */
function formatUpdatePubDate(pubDate: string | undefined): string | null {
    if (!pubDate) return null;
    try {
        const date = new Date(pubDate);
        return date.toLocaleDateString('zh-CN');
    } catch {
        return null;
    }
}

export interface ConfigUpdatePanelProps {
    updateRuntimeInfo: UpdateRuntimeInfo | null;
    updateStatus: UpdateStatus;
    updateCheckResult: UpdateCheckResult | null;
    onCheckForUpdates: () => Promise<void>;
}

/**
 * 系统配置 · 版本与更新
 *
 * 显示当前版本、运行形态（便携版/安装版）、更新状态，并提供检查更新入口。
 * 隐藏入口：连续点击 7 次版本号切换 debug 模式（仅本次会话有效）。
 */
export const ConfigUpdatePanel: React.FC<ConfigUpdatePanelProps> = ({
    updateRuntimeInfo,
    updateStatus,
    updateCheckResult,
    onCheckForUpdates,
}) => {
    const publishedAtText = formatUpdatePubDate(updateCheckResult?.pubDate);
    const { debugEnabled, setDebugEnabled } = useDebug();
    const versionClickCountRef = useRef(0);
    const versionClickTimerRef = useRef<number | null>(null);

    const getUpdateStatusText = (): string => {
        switch (updateStatus) {
            case 'checking': return '正在检查更新';
            case 'available': return '发现新版本';
            case 'downloading': return '正在下载更新';
            case 'installing': return '正在安装更新';
            case 'upToDate': return '当前已是最新版本';
            case 'portableManualOnly': return '便携版需手动下载更新';
            case 'error': return '检查更新失败';
            default: return updateRuntimeInfo?.updaterConfigured ? '尚未检查更新' : '当前构建未启用自动更新';
        }
    };

    /**
     * 连续点击版本号 7 次切换 debug 模式（用户不可见的隐藏入口）。
     */
    const handleVersionClick = useCallback(() => {
        versionClickCountRef.current += 1;

        if (versionClickTimerRef.current !== null) {
            window.clearTimeout(versionClickTimerRef.current);
        }

        if (versionClickCountRef.current < DEBUG_TOGGLE_CLICK_COUNT) {
            versionClickTimerRef.current = window.setTimeout(() => {
                versionClickCountRef.current = 0;
            }, DEBUG_TOGGLE_CLICK_WINDOW_MS);
            return;
        }

        versionClickCountRef.current = 0;
        if (debugEnabled) {
            setDebugEnabled(false);
            toast.info('已关闭调试模式');
        } else {
            setDebugEnabled(true);
            toast.info('已启用调试模式');
        }
    }, [debugEnabled, setDebugEnabled]);

    const isUpdateBusy = updateStatus === 'checking' || updateStatus === 'downloading' || updateStatus === 'installing';
    const currentVersion = updateRuntimeInfo?.currentVersion ?? updateCheckResult?.currentVersion ?? '未知';
    const isPortable = updateRuntimeInfo?.isPortable;
    const hasNewVersion = updateCheckResult?.available && updateCheckResult.version;

    return (
        <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
            <div className="flex items-center justify-between gap-4 mb-4">
                <div className="flex items-center gap-3">
                    <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
                        <Download className="w-5 h-5" />
                    </div>
                    <h3 className="text-lg font-bold text-main">版本与更新</h3>
                </div>
                <button
                    onClick={onCheckForUpdates}
                    disabled={isUpdateBusy}
                    className="btn btn-secondary flex items-center gap-2 text-sm"
                >
                    <RefreshCw className={`w-4 h-4 ${updateStatus === 'checking' ? 'animate-spin' : ''}`} />
                    {updateStatus === 'checking' ? '检查中...' : '检查更新'}
                </button>
            </div>

            <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
                {/* onClick 绑定到整个卡片：连续点击 7 次切换 debug 模式 */}
                <div
                    className="p-3 bg-base/50 rounded-lg border border-base select-none"
                    onClick={handleVersionClick}
                >
                    <div className="text-xs text-muted mb-1">当前版本</div>
                    <div className="text-sm font-medium text-main">
                        v{currentVersion}
                    </div>
                </div>
                <div className="p-3 bg-base/50 rounded-lg border border-base">
                    <div className="text-xs text-muted mb-1">运行形态</div>
                    <div className="text-sm font-medium text-main">{isPortable ? '便携版' : '安装版'}</div>
                </div>
                <div className="p-3 bg-base/50 rounded-lg border border-base">
                    <div className="text-xs text-muted mb-1">更新状态</div>
                    <div className="text-sm font-medium text-main">{getUpdateStatusText()}</div>
                </div>
            </div>

            {hasNewVersion && (
                <div className="mt-3 p-3 bg-ds-success-soft dark:bg-ds-success-soft/20 border border-ds-success-soft dark:border-ds-success-soft rounded-lg">
                    <div className="flex items-center gap-3">
                        <div className="flex-1">
                            <span className="text-sm font-medium text-ds-success-strong dark:text-ds-success-strong">
                                发现新版本 v{updateCheckResult!.version}
                            </span>
                            {publishedAtText && (
                                <span className="text-xs text-muted ml-2">发布时间：{publishedAtText}</span>
                            )}
                        </div>
                        <span className="text-xs text-muted">
                            {updateCheckResult!.isPortable
                                ? '便携版请前往 GitHub 下载'
                                : '确认更新后将自动下载安装'}
                        </span>
                    </div>
                </div>
            )}
        </div>
    );
};
