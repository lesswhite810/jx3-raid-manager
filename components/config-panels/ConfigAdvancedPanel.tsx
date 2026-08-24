import React, { useCallback, useEffect, useState } from 'react';
import { RotateCcw, Trash2, ExternalLink } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from '../../utils/toastManager';
import { db } from '../../services/db';
import { dropScannerService } from '../../services/dropScanner';
import { useDebug } from '../../contexts/DebugContext';

export interface ConfigAdvancedPanelProps {
    /** 由 ConfigManager 控制"重新初始化"确认弹窗（统一处理） */
    onRequestReset: () => void;
    resetting: boolean;
}

/**
 * 系统配置 · 自定义配置
 *
 * 包含：数据目录切换、调试模式（仅 debug 模式可见）、JCL 缓存清理、重新初始化。
 */
export const ConfigAdvancedPanel: React.FC<ConfigAdvancedPanelProps> = ({
    onRequestReset,
    resetting,
}) => {
    const { debugEnabled, setDebugEnabled } = useDebug();

    const [dataDirInfo, setDataDirInfo] = useState<{
        currentPath: string;
        location: 'custom' | 'install' | 'user_home';
        isInstallMode: boolean;
        customDirConfigured: boolean;
    } | null>(null);
    const [clearingJclCache, setClearingJclCache] = useState(false);

    const loadDataDirInfo = useCallback(async () => {
        try {
            const info = await db.getDataDirInfo();
            setDataDirInfo(info);
        } catch (error) {
            console.error('Failed to load data dir info:', error);
        }
    }, []);

    useEffect(() => {
        loadDataDirInfo();
    }, [loadDataDirInfo]);

    const handleSelectCustomDataDir = async () => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: '选择数据存储目录',
            });
            if (typeof selected === 'string') {
                const customPath = selected;
                const hasDb = await db.checkTargetDirHasDb(customPath);
                if (hasDb) {
                    window.dispatchEvent(new CustomEvent('config:dir-conflict', {
                        detail: { path: customPath, action: 'set' },
                    }));
                    return;
                }
                await db.setCustomDataDir(customPath, false);
                toast.success(`已将数据目录修改为: ${customPath}`);
                toast.info('修改将在重启应用后生效，重启时会自动迁移数据库和日志文件');
                await loadDataDirInfo();
            }
        } catch (error) {
            console.error('Failed to set custom data dir:', error);
            toast.error('设置自定义目录失败: ' + (error instanceof Error ? error.message : String(error)));
        }
    };

    const handleResetCustomDataDir = async () => {
        try {
            const defaultPath = await db.getDefaultDataDir();
            const hasDb = await db.checkTargetDirHasDb(defaultPath);
            if (hasDb) {
                window.dispatchEvent(new CustomEvent('config:dir-conflict', {
                    detail: { path: defaultPath, action: 'reset' },
                }));
                return;
            }
            await db.resetCustomDataDir(false);
            toast.success(`已恢复默认数据目录: ${defaultPath}`);
            toast.info('修改将在重启应用后生效，重启时会自动迁移数据库和日志文件');
            await loadDataDirInfo();
        } catch (error) {
            console.error('Failed to reset custom data dir:', error);
            toast.error('恢复默认目录失败: ' + (error instanceof Error ? error.message : String(error)));
        }
    };

    /**
     * 关闭调试模式（仅本次会话有效，不持久化）
     */
    const handleCloseDebug = () => {
        setDebugEnabled(false);
        toast.info('已关闭调试模式');
    };

    /**
     * 清理 JCL 缓存（仅 debug 模式可见）
     */
    const handleClearJclCache = async () => {
        setClearingJclCache(true);
        try {
            const deleted = await dropScannerService.clearJclCache();
            toast.success(`已清空 JCL 缓存（${deleted} 条）`);
            toast.info('下次扫描副本时会重新解析所有 JCL 文件');
        } catch (error) {
            console.error('清空 JCL 缓存失败:', error);
            toast.error('清空 JCL 缓存失败: ' + (error instanceof Error ? error.message : String(error)));
        } finally {
            setClearingJclCache(false);
        }
    };

    // 委托给父组件弹窗
    const handleRequestReset = () => {
        onRequestReset();
    };

    return (
        <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
            <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
                    <RotateCcw className="w-5 h-5" />
                </div>
                <h3 className="text-lg font-bold text-main">自定义配置</h3>
            </div>

            <div className="space-y-4">
                {/* 数据目录 */}
                <div className="flex items-center gap-4">
                    <label className="text-sm font-medium text-muted whitespace-nowrap w-20">数据目录</label>
                    <div className="flex-1 flex items-center gap-2">
                        <p className="text-sm text-main break-all font-mono select-all flex-1 px-3 py-2 bg-base/50 border border-base rounded-lg truncate">
                            {dataDirInfo?.currentPath ?? '加载中...'}
                        </p>
                        <button
                            onClick={handleSelectCustomDataDir}
                            className="btn btn-secondary flex items-center gap-1.5 text-sm whitespace-nowrap"
                        >
                            <ExternalLink className="w-3.5 h-3.5" />
                            切换
                        </button>
                        {dataDirInfo?.customDirConfigured && (
                            <button
                                onClick={handleResetCustomDataDir}
                                className="btn btn-secondary text-sm whitespace-nowrap"
                            >
                                还原
                            </button>
                        )}
                    </div>
                </div>

                {/* 调试模式（仅 debug 模式开启时可见） */}
                {debugEnabled && (
                    <div className="flex items-center gap-4 pt-4 border-t border-base">
                        <label className="text-sm font-medium text-ds-warning-strong dark:text-ds-warning-strong whitespace-nowrap w-20">调试模式</label>
                        <div className="flex-1 flex items-center justify-between">
                            <p className="text-xs text-ds-warning-strong dark:text-ds-warning-strong">
                                调试模式已开启（仅本次会话有效，重启后自动关闭）
                            </p>
                            <button
                                onClick={handleCloseDebug}
                                className="btn btn-secondary text-xs whitespace-nowrap"
                                type="button"
                            >
                                关闭
                            </button>
                        </div>
                    </div>
                )}

                {/* 清理 JCL 缓存（仅 debug 模式可见） */}
                {debugEnabled && (
                    <div className="flex items-center gap-4 pt-4 border-t border-base">
                        <label className="text-sm font-medium text-muted whitespace-nowrap w-20">JCL 缓存</label>
                        <div className="flex-1 flex items-center justify-between">
                            <p className="text-xs text-muted">清空副本战斗日志解析缓存，下次扫描会重新解析所有 JCL 文件</p>
                            <button
                                onClick={handleClearJclCache}
                                disabled={clearingJclCache}
                                className="btn btn-secondary flex items-center gap-1.5 text-xs whitespace-nowrap"
                            >
                                <Trash2 className="w-3.5 h-3.5" />
                                {clearingJclCache ? '清理中...' : '清理 JCL 缓存'}
                            </button>
                        </div>
                    </div>
                )}

                {/* 重新初始化 */}
                <div className="flex items-center gap-4 pt-4 border-t border-base">
                    <label className="text-sm font-medium text-muted whitespace-nowrap w-20">重置应用</label>
                    <div className="flex-1 flex items-center justify-between">
                        <p className="text-xs text-muted">清空游戏目录和账号配置，应用重载后回到引导界面</p>
                        <button
                            onClick={handleRequestReset}
                            disabled={resetting}
                            className="px-3 py-1.5 text-xs bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors flex items-center gap-1.5 whitespace-nowrap"
                        >
                            <RotateCcw className="w-3.5 h-3.5" />
                            重新初始化
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
};