import React, { useCallback, useEffect, useState } from 'react';
import { Check, AlertTriangle, FolderOpen, Search, Monitor, ExternalLink, RefreshCw, Info, RotateCcw, Trash2 } from 'lucide-react';
import type { CloseBehavior, RaidViewMode, Season } from '../../types';
import { isValidGamePath } from '../../utils/configUtils';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from '../../utils/toastManager';
import { scanJx3Clients, Jx3ClientInfo } from '../../services/gameDirectoryScanner';
import { db } from '../../services/db';
import { dropScannerService } from '../../services/dropScanner';
import { useAppConfig } from '../../contexts/AppConfigContext';
import { useDebug } from '../../contexts/DebugContext';
import { ConfigSection, ConfigToggleRow } from './ConfigSection';
import {
    ROW_CLASS,
    ROW_HINT_CLASS,
    ROW_LABEL_CLASS,
    SEGMENTED_CLASS,
    SEGMENTED_ITEM_ACTIVE_CLASS,
    SEGMENTED_ITEM_BASE_CLASS,
    SEGMENTED_ITEM_INACTIVE_CLASS,
} from './configPanelStyles';

/** 关闭按钮行为选项：值、标签与说明 */
const CLOSE_BEHAVIOR_OPTIONS: Array<{ value: CloseBehavior; label: string; description: string }> = [
    { value: 'ask', label: '每次询问', description: '点击关闭时询问最小化到托盘还是直接退出' },
    { value: 'minimize', label: '最小化到托盘', description: '点击关闭时隐藏到托盘，后台继续运行' },
    { value: 'exit', label: '直接退出', description: '点击关闭时结束程序' },
];

/** 副本管理显示模式选项：值、标签与说明 */
const RAID_VIEW_MODE_OPTIONS: Array<{ value: RaidViewMode; label: string; description: string }> = [
    { value: 'current', label: '新版', description: '进入副本管理直接显示 CD 概览表，一眼看出各角色进度' },
    { value: 'history', label: '旧版', description: '恢复页签（团队副本 / 试炼之地 / 百战），团队副本默认显示收藏界面，角色卡片保留账号与密码复制' },
];

export interface ConfigGameAppPanelProps {
    /** 加载好的当前赛季（由 ConfigManager 拉取后传入） */
    currentSeason: Season | null;
    seasonLoaded: boolean;
    /** 路径无效时由 ConfigManager 弹窗提示（统一处理） */
    onShowPathError: () => void;
    /** 由 ConfigManager 控制"重新初始化"确认弹窗（统一处理） */
    onRequestReset: () => void;
    resetting: boolean;
}

/**
 * 系统配置 · 游戏与应用
 *
 * 原「游戏配置」与「自定义配置」合并而来：两者都是「这台机器上的运行环境与
 * 应用自身行为」，拆成两个面板反而让用户分不清某个开关该去哪找。合并后按
 * 归属分成四组：游戏 / 应用行为 / 界面 / 数据与维护。
 */
export const ConfigGameAppPanel: React.FC<ConfigGameAppPanelProps> = ({
    currentSeason,
    seasonLoaded,
    onShowPathError,
    onRequestReset,
    resetting,
}) => {
    const {
        appConfig,
        updateGameDirectory,
        setAutoScanEnabled,
        setTrialScreenshotRecognitionEnabled,
        setAutoRefreshEquipScoreEnabled,
        setCloseBehavior,
        setRaidViewMode,
        setShowWeeklyRaidsEnabled,
        setHideNonMaxLevelRolesEnabled,
    } = useAppConfig();
    const { debugEnabled, setDebugEnabled } = useDebug();

    const [gameDirectory, setGameDirectory] = useState<string>(appConfig?.gameDirectory ?? '');
    const [pathValid, setPathValid] = useState<boolean | null>(null);
    const [scanningClients, setScanningClients] = useState(false);
    const [scanResults, setScanResults] = useState<Jx3ClientInfo[]>([]);
    const [showScanResults, setShowScanResults] = useState(false);
    const [savingTrialScreenshotRecognition, setSavingTrialScreenshotRecognition] = useState(false);
    const [savingCloseBehavior, setSavingCloseBehavior] = useState(false);
    const [savingRaidViewMode, setSavingRaidViewMode] = useState(false);
    const [savingShowWeeklyRaids, setSavingShowWeeklyRaids] = useState(false);
    const [savingHideNonMaxLevelRoles, setSavingHideNonMaxLevelRoles] = useState(false);

    const [dataDirInfo, setDataDirInfo] = useState<{
        currentPath: string;
        location: 'custom' | 'install' | 'user_home';
        isInstallMode: boolean;
        customDirConfigured: boolean;
    } | null>(null);
    const [clearingJclCache, setClearingJclCache] = useState(false);

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

    const handleCloseDebug = () => {
        setDebugEnabled(false);
        toast.info('已关闭调试模式');
    };

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

    const handleCloseBehaviorChange = async (behavior: CloseBehavior) => {
        if (appConfig?.closeBehavior === behavior || savingCloseBehavior) return;
        setSavingCloseBehavior(true);
        try {
            await setCloseBehavior(behavior);
            toast.success(`关闭按钮行为已改为「${CLOSE_BEHAVIOR_OPTIONS.find(item => item.value === behavior)?.label ?? behavior}」`);
        } catch (error) {
            console.error('设置关闭按钮行为失败:', error);
        } finally {
            setSavingCloseBehavior(false);
        }
    };

    const handleRaidViewModeChange = async (mode: RaidViewMode) => {
        if ((appConfig?.raidViewMode ?? 'current') === mode || savingRaidViewMode) return;
        setSavingRaidViewMode(true);
        try {
            await setRaidViewMode(mode);
            toast.success(`副本管理已切换为「${RAID_VIEW_MODE_OPTIONS.find(item => item.value === mode)?.label ?? mode}」`);
        } catch (error) {
            console.error('设置副本管理显示模式失败:', error);
        } finally {
            setSavingRaidViewMode(false);
        }
    };

    const raidViewModeDescription =
        RAID_VIEW_MODE_OPTIONS.find(item => item.value === (appConfig?.raidViewMode ?? 'current'))?.description
        ?? '副本管理页面的展示方式';

    const handleShowWeeklyRaidsToggle = async (enabled: boolean) => {
        if (savingShowWeeklyRaids) return;
        setSavingShowWeeklyRaids(true);
        try {
            await setShowWeeklyRaidsEnabled(enabled);
        } catch (error) {
            console.error('设置展示周常开关失败:', error);
        } finally {
            setSavingShowWeeklyRaids(false);
        }
    };

    const handleHideNonMaxLevelRolesToggle = async (enabled: boolean) => {
        if (savingHideNonMaxLevelRoles) return;
        setSavingHideNonMaxLevelRoles(true);
        try {
            await setHideNonMaxLevelRolesEnabled(enabled);
        } catch (error) {
            console.error('设置隐藏非满级角色开关失败:', error);
        } finally {
            setSavingHideNonMaxLevelRoles(false);
        }
    };

    const closeBehaviorDescription =
        CLOSE_BEHAVIOR_OPTIONS.find(item => item.value === (appConfig?.closeBehavior ?? 'ask'))?.description
        ?? '点击关闭按钮时的行为';

    return (
        <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
            <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
                    <FolderOpen className="w-5 h-5" />
                </div>
                <div className="min-w-0">
                    <h3 className="text-lg font-bold text-main">游戏与应用</h3>
                    <p className="text-xs text-muted mt-0.5">游戏目录、自动识别、界面与数据存放位置</p>
                </div>
            </div>

            <div className="space-y-6">
                <ConfigSection title="游戏">
                    <div className={ROW_CLASS}>
                        <span className={ROW_LABEL_CLASS}>当前赛季</span>
                        <span className="text-sm font-medium text-main">
                            {!seasonLoaded ? '加载中...' : currentSeason ? currentSeason.name : '暂无赛季'}
                        </span>
                    </div>

                    <div className={ROW_CLASS}>
                        <span className={ROW_LABEL_CLASS}>游戏目录</span>
                        <div className="flex-1 flex items-center gap-2 min-w-0">
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
                        <div className={`${ROW_CLASS} pt-0`}>
                            <div className="flex items-center gap-1.5 text-xs text-red-500 pl-24">
                                <AlertTriangle className="w-3.5 h-3.5" />
                                <span>目录无效，请选择包含 SeasunGame 的安装根目录</span>
                            </div>
                        </div>
                    )}

                    {showScanResults && scanResults.length > 0 && (
                        <div className={`${ROW_CLASS} block`}>
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
                </ConfigSection>

                <ConfigSection title="应用行为" hint="这些开关只影响应用本身如何读取与记录，不改变游戏内设置。">
                    <ConfigToggleRow
                        label="自动扫描"
                        description={appConfig?.autoScanEnabled
                            ? '已开启，游戏运行时自动记录副本掉落和试炼通关'
                            : '已关闭，仅手动录入副本记录'}
                        checked={appConfig?.autoScanEnabled ?? false}
                        onChange={handleAutoScanToggle}
                    />

                    {appConfig?.autoScanEnabled && (
                        <div className={`${ROW_CLASS} block`}>
                            <div className="p-3 bg-ds-warning-soft dark:bg-ds-warning-soft/20 border border-ds-warning-soft rounded-lg">
                                <div className="flex items-start gap-2">
                                    <Info className="w-4 h-4 text-ds-warning-strong flex-shrink-0 mt-0.5" />
                                    <p className="text-xs text-ds-warning-strong">
                                        使用前请确认茗伊插件已开启：插件集 → 团队 → 团队工具 → 勾选"战斗事件记录"并启用秘境保存，否则无法扫描到副本数据。
                                    </p>
                                </div>
                            </div>
                        </div>
                    )}

                    <ConfigToggleRow
                        label="试炼截图识别"
                        descriptionId="trial-screenshot-recognition-description"
                        description={appConfig?.trialScreenshotRecognitionEnabled
                            ? appConfig.autoScanEnabled
                                ? '已开启，自动识别翻牌位和装备'
                                : '已保存，开启自动扫描后识别翻牌位和装备'
                            : '已关闭，不读取试炼结算截图'}
                        checked={appConfig?.trialScreenshotRecognitionEnabled ?? false}
                        onChange={handleTrialScreenshotRecognitionToggle}
                        busy={savingTrialScreenshotRecognition}
                    />

                    <div className={`${ROW_CLASS} block`}>
                        <div className="p-3 bg-base/30 border border-base rounded-lg">
                            <div className="flex items-start gap-2">
                                <Info className="w-4 h-4 text-muted flex-shrink-0 mt-0.5" />
                                <p className={`${ROW_HINT_CLASS} leading-relaxed`}>
                                    识别已选翻牌位及鼠标悬停牌的装备。未悬停的牌不会推断为无装备，画面仅在本机内存中处理。
                                </p>
                            </div>
                        </div>
                    </div>

                    <ConfigToggleRow
                        label="启动刷新装分"
                        description={appConfig?.autoRefreshEquipScore
                            ? '已开启，每次启动应用时自动刷新已导入角色的装分'
                            : '已关闭，仅手动在导入角色窗口刷新装分'}
                        checked={appConfig?.autoRefreshEquipScore ?? false}
                        onChange={setAutoRefreshEquipScoreEnabled}
                    />
                </ConfigSection>

                <ConfigSection title="界面" hint="只改变显示方式，不影响已有数据。">
                    <div className={ROW_CLASS}>
                        <span className={ROW_LABEL_CLASS}>副本管理</span>
                        <div className="flex-1 flex items-center justify-between gap-4 min-w-0">
                            <p className={`${ROW_HINT_CLASS} min-w-0`}>{raidViewModeDescription}</p>
                            <div className={SEGMENTED_CLASS}>
                                {RAID_VIEW_MODE_OPTIONS.map(option => {
                                    const active = (appConfig?.raidViewMode ?? 'current') === option.value;
                                    return (
                                        <button
                                            key={option.value}
                                            type="button"
                                            onClick={() => handleRaidViewModeChange(option.value)}
                                            disabled={savingRaidViewMode}
                                            aria-pressed={active}
                                            className={`${SEGMENTED_ITEM_BASE_CLASS} ${active ? SEGMENTED_ITEM_ACTIVE_CLASS : SEGMENTED_ITEM_INACTIVE_CLASS}`}
                                        >
                                            {option.label}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    </div>

                    {appConfig?.raidViewMode !== 'history' && (
                        <ConfigToggleRow
                            label="展示周常"
                            description={appConfig?.showWeeklyRaids ?? true
                                ? 'CD 概览展示全部周常副本（含历史赛季 10 人难度）'
                                : 'CD 概览仅展示当前赛季的周常副本，收藏副本不受影响'}
                            checked={appConfig?.showWeeklyRaids ?? true}
                            onChange={handleShowWeeklyRaidsToggle}
                            busy={savingShowWeeklyRaids}
                        />
                    )}

                    <ConfigToggleRow
                        label="隐藏非满级角色"
                        description={appConfig?.hideNonMaxLevelRoles ?? false
                            ? '仅显示等级达到当前版本上限的角色'
                            : '显示全部角色（默认）'}
                        checked={appConfig?.hideNonMaxLevelRoles ?? false}
                        onChange={handleHideNonMaxLevelRolesToggle}
                        busy={savingHideNonMaxLevelRoles}
                    />

                    <div className={ROW_CLASS}>
                        <span className={ROW_LABEL_CLASS}>关闭按钮</span>
                        <div className="flex-1 flex items-center justify-between gap-4 min-w-0">
                            <p className={`${ROW_HINT_CLASS} min-w-0`}>{closeBehaviorDescription}</p>
                            <div className={SEGMENTED_CLASS}>
                                {CLOSE_BEHAVIOR_OPTIONS.map(option => {
                                    const active = (appConfig?.closeBehavior ?? 'ask') === option.value;
                                    return (
                                        <button
                                            key={option.value}
                                            type="button"
                                            onClick={() => handleCloseBehaviorChange(option.value)}
                                            disabled={savingCloseBehavior}
                                            aria-pressed={active}
                                            className={`${SEGMENTED_ITEM_BASE_CLASS} ${active ? SEGMENTED_ITEM_ACTIVE_CLASS : SEGMENTED_ITEM_INACTIVE_CLASS}`}
                                        >
                                            {option.label}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    </div>
                </ConfigSection>

                <ConfigSection title="数据与维护" hint="改动数据目录或重新初始化后需要重启应用。">
                    <div className={ROW_CLASS}>
                        <span className={ROW_LABEL_CLASS}>数据目录</span>
                        <div className="flex-1 flex items-center gap-2 min-w-0">
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

                    <div className={ROW_CLASS}>
                        <span className={ROW_LABEL_CLASS}>重新初始化</span>
                        <div className="flex-1 flex items-center justify-between gap-4 min-w-0">
                            <p className={`${ROW_HINT_CLASS} min-w-0`}>清空游戏目录和账号配置，应用重载后回到引导界面</p>
                            <button
                                onClick={onRequestReset}
                                disabled={resetting}
                                className="px-3 py-1.5 text-xs bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors flex items-center gap-1.5 whitespace-nowrap"
                            >
                                <RotateCcw className="w-3.5 h-3.5" />
                                重新初始化
                            </button>
                        </div>
                    </div>

                    {debugEnabled && (
                        <div className={ROW_CLASS}>
                            <span className={`${ROW_LABEL_CLASS} text-ds-warning-strong`}>调试模式</span>
                            <div className="flex-1 flex items-center justify-between gap-4 min-w-0">
                                <p className="text-xs text-ds-warning-strong min-w-0">
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

                    {debugEnabled && (
                        <div className={ROW_CLASS}>
                            <span className={ROW_LABEL_CLASS}>JCL 缓存</span>
                            <div className="flex-1 flex items-center justify-between gap-4 min-w-0">
                                <p className={`${ROW_HINT_CLASS} min-w-0`}>
                                    清空副本战斗日志解析缓存，下次扫描会重新解析所有 JCL 文件
                                </p>
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
                </ConfigSection>
            </div>
        </div>
    );
};
