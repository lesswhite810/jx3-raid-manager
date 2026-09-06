import React, { useCallback, useEffect, useState } from 'react';
import type { UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus, Season } from '../types';
import { Palette, Download, FolderOpen, RotateCcw, MessageCircle } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { db } from '../services/db';
import { appConfigService } from '../services/appConfig';
import { toast } from '../utils/toastManager';
import { charSyncService } from '../services/charSync';
import { ConfigThemePanel } from './config-panels/ConfigThemePanel';
import { ConfigUpdatePanel } from './config-panels/ConfigUpdatePanel';
import { ConfigGamePanel } from './config-panels/ConfigGamePanel';
import { ConfigAdvancedPanel } from './config-panels/ConfigAdvancedPanel';
import { ConfigFeedbackPanel } from './config-panels/ConfigFeedbackPanel';

interface ConfigManagerProps {
    updateRuntimeInfo: UpdateRuntimeInfo | null;
    updateStatus: UpdateStatus;
    updateCheckResult: UpdateCheckResult | null;
    onCheckForUpdates: () => Promise<void>;
}

type ConfigCategoryId = 'theme' | 'update' | 'game' | 'advanced' | 'feedback';

interface ConfigCategoryDef {
    id: ConfigCategoryId;
    label: string;
    icon: React.ReactNode;
}

const CONFIG_CATEGORIES: ConfigCategoryDef[] = [
    { id: 'theme', label: '主题与外观', icon: <Palette className="w-4 h-4" /> },
    { id: 'update', label: '版本与更新', icon: <Download className="w-4 h-4" /> },
    { id: 'game', label: '游戏配置', icon: <FolderOpen className="w-4 h-4" /> },
    { id: 'advanced', label: '自定义配置', icon: <RotateCcw className="w-4 h-4" /> },
    { id: 'feedback', label: '问题反馈', icon: <MessageCircle className="w-4 h-4" /> },
];

export const ConfigManager: React.FC<ConfigManagerProps> = ({
    updateRuntimeInfo,
    updateStatus,
    updateCheckResult,
    onCheckForUpdates,
}) => {
    const [activeCategory, setActiveCategory] = useState<ConfigCategoryId>('theme');
    const [currentSeason, setCurrentSeason] = useState<Season | null>(null);
    const [seasonLoaded, setSeasonLoaded] = useState(false);
    const [showResetConfirm, setShowResetConfirm] = useState(false);
    const [resetting, setResetting] = useState(false);
    const [showPathError, setShowPathError] = useState(false);
    const [pendingDirConflict, setPendingDirConflict] = useState<{
        path: string;
        action: 'set' | 'reset';
    } | null>(null);
    const [switchingDir, setSwitchingDir] = useState(false);

    const loadCurrentSeason = useCallback(async () => {
        try {
            const season = await db.getCurrentSeason();
            setCurrentSeason(season);
        } catch (error) {
            console.error('Failed to load current season:', error);
        } finally {
            setSeasonLoaded(true);
        }
    }, []);

    useEffect(() => {
        loadCurrentSeason();
    }, [loadCurrentSeason]);

    const applySwitchDir = useCallback(async (
        path: string,
        action: 'set' | 'reset',
        forceOverwrite: boolean,
    ) => {
        setSwitchingDir(true);
        try {
            if (action === 'set') {
                await db.setCustomDataDir(path, forceOverwrite);
                toast.success(`已将数据目录修改为: ${path}`);
            } else {
                await db.resetCustomDataDir(forceOverwrite);
                toast.success(`已恢复默认数据目录: ${path}`);
            }
            const overwriteMsg = forceOverwrite ? '，重启时会覆盖目标目录已有的数据库文件' : '';
            toast.info(`修改将在重启应用后生效，重启时会自动迁移数据库和日志文件${overwriteMsg}`);
        } catch (error) {
            console.error('Failed to apply switch dir:', error);
            toast.error(
                (action === 'set' ? '设置自定义目录失败: ' : '恢复默认目录失败: ') +
                (error instanceof Error ? error.message : String(error)),
            );
        } finally {
            setSwitchingDir(false);
            setPendingDirConflict(null);
        }
    }, []);

    const handleReset = useCallback(async () => {
        setResetting(true);
        try {
            // 显式清空跨角色同步的回滚标记（数据库重建后表虽为空，
            // 但本调用便于按角色/账号粒度重置时复用）
            try {
                await charSyncService.rollbackMarks.clear();
            } catch (e) {
                console.warn('[ConfigManager] 清空回滚标记失败（可忽略）:', e);
            }
            await appConfigService.resetSetup();
            window.location.reload();
        } catch (error) {
            console.error('重新初始化失败:', error);
            setResetting(false);
            setShowResetConfirm(false);
            toast.error('重新初始化失败: ' + (error instanceof Error ? error.message : String(error)));
        }
    }, []);

    const handleShowPathError = useCallback(() => setShowPathError(true), []);

    useEffect(() => {
        const handler = (e: Event) => {
            const detail = (e as CustomEvent).detail as { path: string; action: 'set' | 'reset' };
            setPendingDirConflict(detail);
        };
        window.addEventListener('config:dir-conflict', handler);
        return () => window.removeEventListener('config:dir-conflict', handler);
    }, []);

    const renderPanel = () => {
        switch (activeCategory) {
            case 'theme':
                return <ConfigThemePanel />;
            case 'update':
                return (
                    <ConfigUpdatePanel
                        updateRuntimeInfo={updateRuntimeInfo}
                        updateStatus={updateStatus}
                        updateCheckResult={updateCheckResult}
                        onCheckForUpdates={onCheckForUpdates}
                    />
                );
            case 'game':
                return (
                    <ConfigGamePanel
                        currentSeason={currentSeason}
                        seasonLoaded={seasonLoaded}
                        onShowPathError={handleShowPathError}
                    />
                );
            case 'advanced':
                return (
                    <ConfigAdvancedPanel
                        onRequestReset={() => setShowResetConfirm(true)}
                        resetting={resetting}
                    />
                );
            case 'feedback':
                return <ConfigFeedbackPanel />;
        }
    };

    return (
        <>
            <div className="flex gap-4 h-full">
                <nav
                    className="w-56 flex-shrink-0 bg-base/30 rounded-lg p-2 space-y-1 self-stretch"
                    aria-label="配置分类"
                >
                    {CONFIG_CATEGORIES.map((cat) => {
                        const isActive = activeCategory === cat.id;
                        return (
                            <button
                                key={cat.id}
                                onClick={() => setActiveCategory(cat.id)}
                                className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                                    isActive
                                        ? 'bg-primary text-primary-foreground shadow-sm'
                                        : 'text-muted hover:text-main hover:bg-surface'
                                }`}
                                aria-current={isActive ? 'page' : undefined}
                            >
                                {cat.icon}
                                <span className="truncate">{cat.label}</span>
                            </button>
                        );
                    })}
                </nav>

                <div className="flex-1 min-w-0 overflow-y-auto pr-1">
                    {renderPanel()}
                </div>
            </div>

            {showResetConfirm && (
                <>
                    <div
                        className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]"
                        onClick={() => !resetting && setShowResetConfirm(false)}
                    />
                    <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
                        <div className="bg-surface rounded-xl border border-base shadow-ds-stack p-6 max-w-sm w-full pointer-events-auto">
                            <h3 className="text-base font-bold text-main mb-2">确认重新初始化</h3>
                            <p className="text-sm text-muted mb-5">
                                此操作将清空游戏目录和账号配置，应用重载后回到引导界面。副本记录和 AI 配置等数据不受影响。
                            </p>
                            <div className="flex justify-end gap-2">
                                <button onClick={() => setShowResetConfirm(false)} disabled={resetting} className="btn btn-secondary text-sm">取消</button>
                                <button onClick={handleReset} disabled={resetting} className="px-4 py-2 text-sm bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors disabled:opacity-50">
                                    {resetting ? '正在重置...' : '确认重置'}
                                </button>
                            </div>
                        </div>
                    </div>
                </>
            )}

            {showPathError && (
                <>
                    <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]" onClick={() => setShowPathError(false)} />
                    <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
                        <div className="bg-surface rounded-xl border border-base shadow-ds-stack p-6 max-w-sm w-full pointer-events-auto">
                            <h3 className="text-base font-bold text-main mb-2">游戏目录无效</h3>
                            <p className="text-sm text-muted mb-5">
                                所选目录无法识别为剑网三安装目录。请选择包含 SeasunGame 文件夹的根目录，例如 <span className="font-mono text-main">E:\Game\SeasunGame</span>，系统会自动补全到游戏运行目录。
                            </p>
                            <div className="flex justify-end gap-2">
                                <button onClick={() => setShowPathError(false)} className="btn btn-secondary text-sm">知道了</button>
                                <button
                                    onClick={async () => {
                                        setShowPathError(false);
                                        try {
                                            const selected = await open({ directory: true, multiple: false, title: '选择剑网三安装目录' });
                                            if (typeof selected === 'string') {
                                                toast.info('请在右侧游戏配置面板中点击"切换"完成设置');
                                            }
                                        } catch (error) { console.error(error); }
                                    }}
                                    className="btn btn-primary text-sm"
                                >重新选择</button>
                            </div>
                        </div>
                    </div>
                </>
            )}

            {pendingDirConflict && (
                <>
                    <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]" onClick={() => !switchingDir && setPendingDirConflict(null)} />
                    <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
                        <div className="bg-surface rounded-xl border border-base shadow-ds-stack p-6 max-w-md w-full pointer-events-auto">
                            <div className="flex items-start gap-3 mb-3">
                                <div className="w-9 h-9 bg-ds-warning-soft text-ds-warning-strong rounded-lg flex items-center justify-center flex-shrink-0">
                                    <svg xmlns="http://www.w3.org/2000/svg" className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                        <path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                                        <line x1="12" y1="9" x2="12" y2="13" />
                                        <line x1="12" y1="17" x2="12.01" y2="17" />
                                    </svg>
                                </div>
                                <div className="min-w-0 flex-1">
                                    <h3 className="text-base font-bold text-main mb-1">目标目录已有数据库文件</h3>
                                    <p className="text-sm text-muted"><span className="font-mono text-xs break-all">{pendingDirConflict.path}</span></p>
                                </div>
                            </div>
                            <div className="text-sm text-muted mb-5 space-y-2">
                                <p>该目录中已存在 <span className="font-mono text-main">jx3-raid-manager.db</span> 文件。请选择处理方式：</p>
                                <div className="bg-base/30 rounded-lg p-3 space-y-1.5">
                                    <p className="text-xs text-main"><span className="font-semibold text-ds-warning">覆盖目标文件：</span>用当前数据替换目标目录中的旧数据库，目标原有数据将丢失。</p>
                                    <p className="text-xs text-main"><span className="font-semibold text-muted">取消：</span>不切换目录，当前数据不受影响。</p>
                                </div>
                            </div>
                            <div className="flex justify-end gap-2">
                                <button onClick={() => setPendingDirConflict(null)} disabled={switchingDir} className="btn btn-secondary text-sm">取消</button>
                                <button onClick={() => applySwitchDir(pendingDirConflict.path, pendingDirConflict.action, true)} disabled={switchingDir} className="px-4 py-2 text-sm bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors disabled:opacity-50">
                                    {switchingDir ? '处理中...' : '覆盖目标文件'}
                                </button>
                            </div>
                        </div>
                    </div>
                </>
            )}
        </>
    );
};