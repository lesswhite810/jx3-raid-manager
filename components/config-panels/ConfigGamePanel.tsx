import React, { useCallback, useEffect, useState } from 'react';
import { Check, AlertTriangle, FolderOpen, Search, Monitor, ExternalLink, RefreshCw, Info } from 'lucide-react';
import type { Season } from '../../types';
import { isValidGamePath } from '../../utils/configUtils';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from '../../utils/toastManager';
import { scanJx3Clients, Jx3ClientInfo } from '../../services/gameDirectoryScanner';
import { useAppConfig } from '../../contexts/AppConfigContext';

export interface ConfigGamePanelProps {
    /** 加载好的当前赛季（由 ConfigManager 拉取后传入） */
    currentSeason: Season | null;
    seasonLoaded: boolean;
    /** 路径无效时由 ConfigManager 弹窗提示（统一处理） */
    onShowPathError: () => void;
}

/**
 * 系统配置 · 游戏配置
 *
 * 包含：当前赛季、游戏目录选择与扫描客户端、自动扫描、试炼截图识别、启动刷新装分开关。
 */
export const ConfigGamePanel: React.FC<ConfigGamePanelProps> = ({
    currentSeason,
    seasonLoaded,
    onShowPathError,
}) => {
    const {
        appConfig,
        updateGameDirectory,
        setAutoScanEnabled,
        setTrialScreenshotRecognitionEnabled,
        setAutoRefreshEquipScoreEnabled,
    } = useAppConfig();

    const [gameDirectory, setGameDirectory] = useState<string>(appConfig?.gameDirectory ?? '');
    const [pathValid, setPathValid] = useState<boolean | null>(null);
    const [scanningClients, setScanningClients] = useState(false);
    const [scanResults, setScanResults] = useState<Jx3ClientInfo[]>([]);
    const [showScanResults, setShowScanResults] = useState(false);
    const [savingTrialScreenshotRecognition, setSavingTrialScreenshotRecognition] = useState(false);

    useEffect(() => {
        setGameDirectory(appConfig?.gameDirectory ?? '');
    }, [appConfig?.gameDirectory]);

    useEffect(() => {
        if (!gameDirectory.trim()) {
            setPathValid(null);
            return;
        }
        isValidGamePath(gameDirectory).then(result => {
            setPathValid(result.isValid);
        });
    }, [gameDirectory]);

    const handleScanClients = useCallback(async () => {
        setScanningClients(true);
        setShowScanResults(false);
        try {
            const result = await scanJx3Clients();
            if (result.success && result.clients.length > 0) {
                setScanResults(result.clients);
                setShowScanResults(true);
                if (result.clients.length === 1 && !gameDirectory) {
                    const client = result.clients[0];
                    await updateGameDirectory(client.workDirectory);
                    toast.success(`已自动填入 ${client.displayName} 的安装目录`);
                } else if (result.clients.length > 1) {
                    toast.info(`检测到 ${result.clients.length} 个客户端，请选择`);
                }
            } else {
                toast.error(result.error || '未检测到剑网3客户端，请确认游戏已安装');
            }
        } catch (error) {
            console.error('扫描客户端失败:', error);
            toast.error('扫描失败: ' + (error instanceof Error ? error.message : String(error)));
        } finally {
            setScanningClients(false);
        }
    }, [gameDirectory, updateGameDirectory]);

    const handleSelectClient = useCallback(async (client: Jx3ClientInfo) => {
        await updateGameDirectory(client.workDirectory);
        setShowScanResults(false);
        toast.success(`已选择 ${client.displayName}`);
    }, [updateGameDirectory]);

    const handleBrowseGameDirectory = async () => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: '选择剑网三安装目录',
            });
            if (typeof selected === 'string') {
                await updateGameDirectory(selected);
                setGameDirectory(selected);
                const result = await isValidGamePath(selected);
                setPathValid(result.isValid);
                if (!result.isValid) {
                    onShowPathError();
                }
            }
        } catch (error) {
            console.error('选择游戏目录失败:', error);
            toast.error('选择目录失败: ' + (error instanceof Error ? error.message : String(error)));
        }
    };

    const handleAutoScanToggle = async (enabled: boolean) => {
        if (enabled) {
            toast.info('请确保已在茗伊插件中开启：插件集 → 团队 → 团队工具 → 勾选"战斗事件记录"并启用秘境保存');
        }
        await setAutoScanEnabled(enabled);
    };

    const handleAutoRefreshEquipScoreToggle = async (enabled: boolean) => {
        await setAutoRefreshEquipScoreEnabled(enabled);
    };

    const handleTrialScreenshotRecognitionToggle = async (enabled: boolean) => {
        // 保存期间禁用控件，避免连续切换使界面状态与已持久化状态错位。
        if (savingTrialScreenshotRecognition) return;

        setSavingTrialScreenshotRecognition(true);
        try {
            await setTrialScreenshotRecognitionEnabled(enabled);
        } catch {
            // Context 已给出保存失败提示，界面保持上一次成功保存的状态。
        } finally {
            setSavingTrialScreenshotRecognition(false);
        }
    };

    return (
        <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
            <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
                    <FolderOpen className="w-5 h-5" />
                </div>
                <h3 className="text-lg font-bold text-main">游戏配置</h3>
            </div>

            <div className="space-y-4">
                <div className="flex items-center gap-4">
                    <label className="text-sm font-medium text-muted whitespace-nowrap w-20">当前赛季</label>
                    <span className="text-sm font-medium text-main">{!seasonLoaded ? '加载中...' : currentSeason ? currentSeason.name : '暂无赛季'}</span>
                </div>

                <div className="flex items-center gap-4 pt-4 border-t border-base">
                    <label className="text-sm font-medium text-muted whitespace-nowrap w-20">游戏目录</label>
                    <div className="flex-1 flex items-center gap-2">
                        <p className="text-sm text-main break-all font-mono select-all flex-1 px-3 py-2 bg-base/50 border border-base rounded-lg truncate">
                            {gameDirectory || '未设置'}
                        </p>
                        {pathValid === true && <Check className="w-4 h-4 text-ds-success-strong flex-shrink-0" />}
                        {pathValid === false && <AlertTriangle className="w-4 h-4 text-red-500 flex-shrink-0" />}
                        <button
                            onClick={handleBrowseGameDirectory}
                            className="btn btn-secondary flex items-center gap-1.5 text-sm whitespace-nowrap"
                        >
                            <ExternalLink className="w-3.5 h-3.5" />
                            切换
                        </button>
                        <button
                            onClick={handleScanClients}
                            disabled={scanningClients}
                            className="btn btn-secondary flex items-center gap-1.5 text-sm whitespace-nowrap"
                            title="从注册表扫描剑网3客户端"
                        >
                            {scanningClients ? (
                                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                            ) : (
                                <Search className="w-3.5 h-3.5" />
                            )}
                            {scanningClients ? '扫描中...' : '扫描'}
                        </button>
                    </div>
                </div>

                {pathValid === false && (
                    <div className="flex items-center gap-2 pl-24">
                        <div className="flex items-center gap-1.5 text-xs text-red-500">
                            <AlertTriangle className="w-3.5 h-3.5" />
                            <span>目录无效，请选择包含 SeasunGame 的安装根目录</span>
                        </div>
                    </div>
                )}

                {showScanResults && scanResults.length > 0 && (
                    <div className="p-4 bg-base/30 rounded-lg border border-base">
                        <div className="flex items-center gap-2 mb-3">
                            <Monitor className="w-4 h-4 text-primary" />
                            <span className="text-sm font-medium text-main">已检测到的客户端</span>
                            <button
                                onClick={() => setShowScanResults(false)}
                                className="ml-auto text-xs text-muted hover:text-main transition-colors"
                            >
                                收起
                            </button>
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2">
                            {scanResults.map((client, index) => (
                                <div
                                    key={index}
                                    className="p-3 rounded-lg border border-base hover:border-primary/50 hover:bg-surface/50 transition-all cursor-pointer group"
                                    onClick={() => handleSelectClient(client)}
                                >
                                    <div className="text-sm font-medium text-main group-hover:text-primary">
                                        {client.displayName}
                                    </div>
                                    <div className="text-xs text-muted truncate mt-0.5">{client.workDirectory}</div>
                                    {client.version && (
                                        <div className="text-xs text-muted/70 mt-0.5">版本: {client.version}</div>
                                    )}
                                </div>
                            ))}
                        </div>
                    </div>
                )}

                <div className="flex items-center justify-between gap-4 pt-4 border-t border-base">
                    <div className="flex items-center gap-4">
                        <label className="text-sm font-medium text-muted whitespace-nowrap w-20">自动扫描</label>
                        <p className="text-xs text-muted">
                            {appConfig?.autoScanEnabled
                                ? '已开启，游戏运行时自动记录副本掉落和试炼通关'
                                : '已关闭，仅手动录入副本记录'}
                        </p>
                    </div>
                    <button
                        onClick={() => handleAutoScanToggle(!appConfig?.autoScanEnabled)}
                        className={`relative w-11 h-6 rounded-full transition-colors flex-shrink-0 ${
                            appConfig?.autoScanEnabled ? 'bg-ds-success' : 'bg-slate-300 dark:bg-slate-600'
                        }`}
                    >
                        <span
                            className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform ${
                                appConfig?.autoScanEnabled ? 'translate-x-5' : ''
                            }`}
                        />
                    </button>
                </div>

                {appConfig?.autoScanEnabled && (
                    <div className="p-3 bg-ds-warning-soft dark:bg-ds-warning-soft/20 border border-ds-warning-soft dark:border-ds-warning-soft rounded-lg">
                        <div className="flex items-start gap-2">
                            <Info className="w-4 h-4 text-ds-warning-strong flex-shrink-0 mt-0.5" />
                            <p className="text-xs text-ds-warning-strong dark:text-ds-warning-strong">
                                使用前请确认茗伊插件已开启：插件集 → 团队 → 团队工具 → 勾选"战斗事件记录"并启用秘境保存，否则无法扫描到副本数据。
                            </p>
                        </div>
                    </div>
                )}

                <div className="flex items-center justify-between gap-4 pt-4 border-t border-base">
                    <div className="flex items-center gap-4">
                        <label className="text-sm font-medium text-muted whitespace-nowrap w-20">试炼截图识别</label>
                        <p id="trial-screenshot-recognition-description" className="text-xs text-muted">
                            {appConfig?.trialScreenshotRecognitionEnabled
                                ? appConfig.autoScanEnabled
                                    ? '已开启，自动识别翻牌位和装备'
                                    : '已保存，开启自动扫描后识别翻牌位和装备'
                                : '已关闭，不读取试炼结算截图'}
                        </p>
                    </div>
                    <button
                        type="button"
                        role="switch"
                        onClick={() => void handleTrialScreenshotRecognitionToggle(!(appConfig?.trialScreenshotRecognitionEnabled ?? false))}
                        disabled={savingTrialScreenshotRecognition}
                        aria-checked={appConfig?.trialScreenshotRecognitionEnabled ?? false}
                        aria-busy={savingTrialScreenshotRecognition}
                        aria-label="试炼截图识别"
                        aria-describedby="trial-screenshot-recognition-description"
                        className={`relative w-11 h-6 rounded-full transition-colors flex-shrink-0 disabled:cursor-not-allowed disabled:opacity-60 ${
                            appConfig?.trialScreenshotRecognitionEnabled ? 'bg-ds-success' : 'bg-slate-300 dark:bg-slate-600'
                        }`}
                    >
                        <span
                            className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform ${
                                appConfig?.trialScreenshotRecognitionEnabled ? 'translate-x-5' : ''
                            }`}
                        />
                    </button>
                </div>

                <div className="p-3 bg-base/30 border border-base rounded-lg">
                    <div className="flex items-start gap-2">
                        <Info className="w-4 h-4 text-muted flex-shrink-0 mt-0.5" />
                        <p className="text-xs text-muted leading-relaxed">
                            识别已选翻牌位及鼠标悬停牌的装备。未悬停的牌不会推断为无装备，画面仅在本机内存中处理。
                        </p>
                    </div>
                </div>

                <div className="flex items-center justify-between gap-4 pt-4 border-t border-base">
                    <div className="flex items-center gap-4">
                        <label className="text-sm font-medium text-muted whitespace-nowrap w-20">启动刷新装分</label>
                        <p className="text-xs text-muted">
                            {appConfig?.autoRefreshEquipScore
                                ? '已开启，每次启动应用时自动刷新已导入角色的装分'
                                : '已关闭，仅手动在导入角色窗口刷新装分'}
                        </p>
                    </div>
                    <button
                        onClick={() => handleAutoRefreshEquipScoreToggle(!appConfig?.autoRefreshEquipScore)}
                        className={`relative w-11 h-6 rounded-full transition-colors flex-shrink-0 ${
                            appConfig?.autoRefreshEquipScore ? 'bg-ds-success' : 'bg-slate-300 dark:bg-slate-600'
                        }`}
                    >
                        <span
                            className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow-sm transition-transform ${
                                appConfig?.autoRefreshEquipScore ? 'translate-x-5' : ''
                            }`}
                        />
                    </button>
                </div>
            </div>
        </div>
    );
};
