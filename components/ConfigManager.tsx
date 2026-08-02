import React, { useCallback, useEffect, useRef, useState } from 'react';
import { UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus, Season } from '../types';
import { Check, AlertTriangle, FolderOpen, Download, RefreshCw, ExternalLink, Search, Monitor, RotateCcw, MessageCircle, Info, Trash2 } from 'lucide-react';
import { isValidGamePath } from '../utils/configUtils';
import { db } from '../services/db';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from '../utils/toastManager';
import { scanJx3Clients, Jx3ClientInfo } from '../services/gameDirectoryScanner';
import { dropScannerService } from '../services/dropScanner';
import { useAppConfig } from '../contexts/AppConfigContext';
import { useDebug } from '../contexts/DebugContext';

/** Debug 模式：连续点击次数阈值，达到后切换 */
const DEBUG_TOGGLE_CLICK_COUNT = 7;
/** 连续点击的有效间隔（毫秒），超时重置计数 */
const DEBUG_TOGGLE_CLICK_WINDOW_MS = 1500;

interface ConfigManagerProps {
  updateRuntimeInfo: UpdateRuntimeInfo | null;
  updateStatus: UpdateStatus;
  updateCheckResult: UpdateCheckResult | null;
  onCheckForUpdates: () => Promise<void>;
}

export const ConfigManager: React.FC<ConfigManagerProps> = ({
  updateRuntimeInfo,
  updateStatus,
  updateCheckResult,
  onCheckForUpdates
}) => {
  const publishedAtText = formatUpdatePubDate(updateCheckResult?.pubDate);
  const [pathValid, setPathValid] = useState<boolean | null>(null);
  const [dataDirInfo, setDataDirInfo] = useState<{
    currentPath: string;
    location: 'custom' | 'install' | 'user_home';
    isInstallMode: boolean;
    customDirConfigured: boolean;
  } | null>(null);
  const [scanningClients, setScanningClients] = useState(false);
  const [scanResults, setScanResults] = useState<Jx3ClientInfo[]>([]);
  const [showScanResults, setShowScanResults] = useState(false);
  const [currentSeason, setCurrentSeason] = useState<Season | null>(null);
  const [seasonLoaded, setSeasonLoaded] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [showPathError, setShowPathError] = useState(false);
  const [clearingJclCache, setClearingJclCache] = useState(false);
  const [showClearJclCacheConfirm, setShowClearJclCacheConfirm] = useState(false);
  // 切换数据目录时目标已存在数据库文件的冲突提示
  // pendingDirConflict.path = 用户选择的目标目录
  // pendingDirConflict.action = 'set' | 'reset'，决定确认后调用哪个 API
  const [pendingDirConflict, setPendingDirConflict] = useState<{
    path: string;
    action: 'set' | 'reset';
  } | null>(null);
  const [switchingDir, setSwitchingDir] = useState(false);
  // Debug 模式：从全局 DebugContext 获取，仅本次会话有效不持久化
  const { debugEnabled, setDebugEnabled } = useDebug();
  const versionClickCountRef = useRef(0);
  const versionClickTimerRef = useRef<number | null>(null);

  const { appConfig, updateGameDirectory, resetAll, setAutoScanEnabled, setAutoRefreshEquipScoreEnabled } = useAppConfig();

  const [gameDirectory, setGameDirectory] = useState<string>('');

  useEffect(() => {
    setGameDirectory(appConfig?.gameDirectory ?? '');
  }, [appConfig?.gameDirectory]);

  const loadDataDirInfo = useCallback(async () => {
    try {
      const info = await db.getDataDirInfo();
      setDataDirInfo(info);
    } catch (error) {
      console.error('Failed to load data dir info:', error);
    }
  }, []);

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
    loadDataDirInfo();
  }, [loadDataDirInfo]);

  useEffect(() => {
    loadCurrentSeason();
  }, [loadCurrentSeason]);

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
          setShowPathError(true);
        }
      }
    } catch (error) {
      console.error('选择游戏目录失败:', error);
      toast.error('选择目录失败: ' + (error instanceof Error ? error.message : String(error)));
    }
  };

  const getUpdateStatusText = () => {
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

  const handleSelectCustomDataDir = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择数据存储目录',
      });
      if (typeof selected === 'string') {
        const customPath = selected;
        // 先检查目标目录是否已存在数据库文件
        const hasDb = await db.checkTargetDirHasDb(customPath);
        if (hasDb) {
          setPendingDirConflict({ path: customPath, action: 'set' });
          return;
        }
        await applySwitchDir(customPath, 'set', false);
      }
    } catch (error) {
      console.error('Failed to set custom data dir:', error);
      toast.error('设置自定义目录失败: ' + (error instanceof Error ? error.message : String(error)));
    }
  };

  const handleResetCustomDataDir = async () => {
    try {
      // 先获取默认目录路径（不写入配置），检查是否已有 db 文件
      const defaultPath = await db.getDefaultDataDir();
      const hasDb = await db.checkTargetDirHasDb(defaultPath);
      if (hasDb) {
        setPendingDirConflict({ path: defaultPath, action: 'reset' });
        return;
      }
      await applySwitchDir(defaultPath, 'reset', false);
    } catch (error) {
      console.error('Failed to reset custom data dir:', error);
      toast.error('恢复默认目录失败: ' + (error instanceof Error ? error.message : String(error)));
    }
  };

  // 应用目录切换：根据 action 调用对应 API，forceOverwrite 控制是否覆盖目标已有数据库
  const applySwitchDir = async (
    path: string,
    action: 'set' | 'reset',
    forceOverwrite: boolean
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
      await loadDataDirInfo();
    } catch (error) {
      console.error('Failed to apply switch dir:', error);
      toast.error(
        action === 'set' ? '设置自定义目录失败: ' : '恢复默认目录失败: ' +
        (error instanceof Error ? error.message : String(error))
      );
    } finally {
      setSwitchingDir(false);
      setPendingDirConflict(null);
    }
  };

  const handleReset = async () => {
    setResetting(true);
    try {
      await resetAll();
    } catch (error) {
      console.error('重新初始化失败:', error);
      setResetting(false);
      setShowResetConfirm(false);
    }
  };

  const handleClearJclCache = async () => {
    setClearingJclCache(true);
    try {
      const deleted = await dropScannerService.clearJclCache();
      toast.success(`已清空 JCL 缓存（${deleted} 条）`);
      toast.info('下次扫描副本时会重新解析所有 JCL 文件');
      setShowClearJclCacheConfirm(false);
    } catch (error) {
      console.error('清空 JCL 缓存失败:', error);
      toast.error('清空 JCL 缓存失败: ' + (error instanceof Error ? error.message : String(error)));
    } finally {
      setClearingJclCache(false);
    }
  };

  /**
   * 处理版本号点击：连续点击 7 次切换 debug 模式。
   * 用户不可见的隐藏入口，仅本次会话有效不持久化。
   */
  const handleVersionClick = useCallback(() => {
    versionClickCountRef.current += 1;

    if (versionClickTimerRef.current !== null) {
      window.clearTimeout(versionClickTimerRef.current);
    }

    // 未达阈值，设置超时重置
    if (versionClickCountRef.current < DEBUG_TOGGLE_CLICK_COUNT) {
      versionClickTimerRef.current = window.setTimeout(() => {
        versionClickCountRef.current = 0;
      }, DEBUG_TOGGLE_CLICK_WINDOW_MS);
      return;
    }

    // 达到阈值，切换 debug 模式
    versionClickCountRef.current = 0;
    if (debugEnabled) {
      setDebugEnabled(false);
      toast.info('已关闭调试模式');
      // 关闭 debug 模式时同时关闭已打开的确认弹窗
      setShowClearJclCacheConfirm(false);
    } else {
      setDebugEnabled(true);
      toast.info('已启用调试模式');
    }
  }, [debugEnabled, setDebugEnabled]);

  const handleAutoScanToggle = async (enabled: boolean) => {
    if (enabled) {
      toast.info('请确保已在茗伊插件中开启：插件集 → 团队 → 团队工具 → 勾选"战斗事件记录"并启用秘境保存');
    }
    await setAutoScanEnabled(enabled);
  };

  const handleAutoRefreshEquipScoreToggle = async (enabled: boolean) => {
    await setAutoRefreshEquipScoreEnabled(enabled);
  };

  const isUpdateBusy = updateStatus === 'checking' || updateStatus === 'downloading' || updateStatus === 'installing';
  const currentVersion = updateRuntimeInfo?.currentVersion ?? updateCheckResult?.currentVersion ?? '未知';
  const isPortable = updateRuntimeInfo?.isPortable;
  const hasNewVersion = updateCheckResult?.available && updateCheckResult.version;

  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-bold text-main">系统配置</h2>

      {/* 版本与更新 */}
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
          <div className="mt-3 p-3 bg-emerald-50 dark:bg-emerald-900/20 border border-emerald-200 dark:border-emerald-800 rounded-lg">
            <div className="flex items-center gap-3">
              <div className="flex-1">
                <span className="text-sm font-medium text-emerald-700 dark:text-emerald-300">
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

      {/* 游戏配置 */}
      <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
            <FolderOpen className="w-5 h-5" />
          </div>
          <h3 className="text-lg font-bold text-main">游戏配置</h3>
        </div>

        <div className="space-y-4">
          {/* 当前赛季 */}
          <div className="flex items-center gap-4">
            <label className="text-sm font-medium text-muted whitespace-nowrap w-20">当前赛季</label>
            <span className="text-sm font-medium text-main">{!seasonLoaded ? '加载中...' : currentSeason ? currentSeason.name : '暂无赛季'}</span>
          </div>

          {/* 游戏目录 */}
          <div className="flex items-center gap-4 pt-4 border-t border-base">
            <label className="text-sm font-medium text-muted whitespace-nowrap w-20">游戏目录</label>
            <div className="flex-1 flex items-center gap-2">
              <p className="text-sm text-main break-all font-mono select-all flex-1 px-3 py-2 bg-base/50 border border-base rounded-lg truncate">
                {gameDirectory || '未设置'}
              </p>
              {pathValid === true && <Check className="w-4 h-4 text-emerald-500 flex-shrink-0" />}
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

          {/* 路径校验提示 */}
          {pathValid === false && (
            <div className="flex items-center gap-2 pl-24">
              <div className="flex items-center gap-1.5 text-xs text-red-500">
                <AlertTriangle className="w-3.5 h-3.5" />
                <span>目录无效，请选择包含 SeasunGame 的安装根目录</span>
              </div>
            </div>
          )}

          {/* 扫描结果 */}
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

          {/* 自动扫描 */}
          <div className="flex items-center justify-between gap-4 pt-4 border-t border-base">
            <div className="flex items-center gap-3">
              <div className="flex items-center gap-4">
                <label className="text-sm font-medium text-muted whitespace-nowrap w-20">自动扫描</label>
                <p className="text-xs text-muted">
                  {appConfig?.autoScanEnabled
                    ? '已开启，游戏运行时自动扫描副本掉落'
                    : '已关闭，仅手动录入副本记录'}
                </p>
              </div>
            </div>
            <button
              onClick={() => handleAutoScanToggle(!appConfig?.autoScanEnabled)}
              className={`relative w-11 h-6 rounded-full transition-colors flex-shrink-0 ${
                appConfig?.autoScanEnabled ? 'bg-emerald-500' : 'bg-slate-300 dark:bg-slate-600'
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
            <div className="p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
              <div className="flex items-start gap-2">
                <Info className="w-4 h-4 text-amber-500 flex-shrink-0 mt-0.5" />
                <p className="text-xs text-amber-700 dark:text-amber-300">
                  使用前请确认茗伊插件已开启：插件集 → 团队 → 团队工具 → 勾选"战斗事件记录"并启用秘境保存，否则无法扫描到副本数据。
                </p>
              </div>
            </div>
          )}

          {/* 启动刷新装分 */}
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
                appConfig?.autoRefreshEquipScore ? 'bg-emerald-500' : 'bg-slate-300 dark:bg-slate-600'
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

      {/* 自定义配置 */}
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

          {/* 调试模式标识（仅 debug 模式开启时可见） */}
          {debugEnabled && (
            <div className="flex items-center gap-4 pt-4 border-t border-base">
              <label className="text-sm font-medium text-amber-600 dark:text-amber-400 whitespace-nowrap w-20">调试模式</label>
              <div className="flex-1 flex items-center justify-between">
                <p className="text-xs text-amber-600 dark:text-amber-400">
                  调试模式已开启（仅本次会话有效，重启后自动关闭）
                </p>
                <button
                  onClick={() => {
                    setDebugEnabled(false);
                    setShowClearJclCacheConfirm(false);
                    toast.info('已关闭调试模式');
                  }}
                  className="btn btn-secondary text-xs whitespace-nowrap"
                  type="button"
                >
                  关闭
                </button>
              </div>
            </div>
          )}

          {/* 清理 JCL 缓存（仅 debug 模式下可见） */}
          {debugEnabled && (
            <div className="flex items-center gap-4 pt-4 border-t border-base">
              <label className="text-sm font-medium text-muted whitespace-nowrap w-20">JCL 缓存</label>
              <div className="flex-1 flex items-center justify-between">
                <p className="text-xs text-muted">清空副本战斗日志解析缓存，下次扫描会重新解析所有 JCL 文件</p>
                <button
                  onClick={() => setShowClearJclCacheConfirm(true)}
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
                onClick={() => setShowResetConfirm(true)}
                className="px-3 py-1.5 text-xs bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors flex items-center gap-1.5 whitespace-nowrap"
              >
                <RotateCcw className="w-3.5 h-3.5" />
                重新初始化
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* 问题反馈 */}
      <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
            <MessageCircle className="w-5 h-5" />
          </div>
          <h3 className="text-lg font-bold text-main">问题反馈</h3>
        </div>
        <div className="flex items-center gap-5 p-4 bg-base/30 rounded-lg">
          <img
            src="/qq-group-qr.jpg"
            alt="QQ群二维码"
            className="w-28 h-28 rounded-lg border border-base object-contain bg-white flex-shrink-0"
          />
          <div className="flex flex-col justify-center min-w-0 flex-1">
            <p className="text-sm text-main mb-1.5">加入 QQ 群反馈问题、提建议或交流使用心得</p>
            <div className="flex items-baseline gap-2">
              <span className="text-xs text-muted">群号</span>
              <span className="text-lg font-bold text-primary select-all">1085903108</span>
            </div>
            <p className="text-xs text-muted mt-1.5">扫描左侧二维码或搜索群号加入</p>
          </div>
        </div>
      </div>

      {/* 重新初始化确认弹窗 */}
      {showResetConfirm && (
        <>
          <div
            className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]"
            onClick={() => !resetting && setShowResetConfirm(false)}
          />
          <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
            <div className="bg-surface rounded-xl border border-base shadow-lg p-6 max-w-sm w-full pointer-events-auto">
              <h3 className="text-base font-bold text-main mb-2">确认重新初始化</h3>
              <p className="text-sm text-muted mb-5">
                此操作将清空游戏目录和账号配置，应用重载后回到引导界面。副本记录和 AI 配置等数据不受影响。
              </p>
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => setShowResetConfirm(false)}
                  disabled={resetting}
                  className="btn btn-secondary text-sm"
                >
                  取消
                </button>
                <button
                  onClick={handleReset}
                  disabled={resetting}
                  className="px-4 py-2 text-sm bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors disabled:opacity-50"
                >
                  {resetting ? '正在重置...' : '确认重置'}
                </button>
              </div>
            </div>
          </div>
        </>
      )}

      {/* 清理 JCL 缓存确认弹窗 */}
      {showClearJclCacheConfirm && (
        <>
          <div
            className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]"
            onClick={() => !clearingJclCache && setShowClearJclCacheConfirm(false)}
          />
          <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
            <div className="bg-surface rounded-xl border border-base shadow-lg p-6 max-w-sm w-full pointer-events-auto">
              <h3 className="text-base font-bold text-main mb-2">确认清理 JCL 缓存</h3>
              <p className="text-sm text-muted mb-5">
                将清空副本战斗日志的解析缓存。下次扫描副本时会重新解析所有 JCL 文件，可能耗时略增。副本记录和已确认数据不受影响。
              </p>
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => setShowClearJclCacheConfirm(false)}
                  disabled={clearingJclCache}
                  className="btn btn-secondary text-sm"
                >
                  取消
                </button>
                <button
                  onClick={handleClearJclCache}
                  disabled={clearingJclCache}
                  className="px-4 py-2 text-sm bg-amber-600 text-white rounded-lg hover:bg-amber-700 transition-colors disabled:opacity-50"
                >
                  {clearingJclCache ? '清理中...' : '确认清理'}
                </button>
              </div>
            </div>
          </div>
        </>
      )}

      {/* 目标目录已有数据库文件冲突确认弹窗 */}
      {pendingDirConflict && (
        <>
          <div
            className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]"
            onClick={() => !switchingDir && setPendingDirConflict(null)}
          />
          <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
            <div className="bg-surface rounded-xl border border-base shadow-lg p-6 max-w-md w-full pointer-events-auto">
              <div className="flex items-start gap-3 mb-3">
                <div className="w-9 h-9 bg-amber-100 text-amber-700 rounded-lg flex items-center justify-center flex-shrink-0">
                  <AlertTriangle className="w-5 h-5" />
                </div>
                <div className="min-w-0 flex-1">
                  <h3 className="text-base font-bold text-main mb-1">目标目录已有数据库文件</h3>
                  <p className="text-sm text-muted">
                    <span className="font-mono text-xs break-all">{pendingDirConflict.path}</span>
                  </p>
                </div>
              </div>
              <div className="text-sm text-muted mb-5 space-y-2">
                <p>
                  该目录中已存在 <span className="font-mono text-main">jx3-raid-manager.db</span> 文件。请选择处理方式：
                </p>
                <div className="bg-base/30 rounded-lg p-3 space-y-1.5">
                  <p className="text-xs text-main">
                    <span className="font-semibold text-amber-700">覆盖目标文件：</span>
                    用当前数据替换目标目录中的旧数据库，目标原有数据将丢失。
                  </p>
                  <p className="text-xs text-main">
                    <span className="font-semibold text-muted">取消：</span>
                    不切换目录，当前数据不受影响。
                  </p>
                </div>
              </div>
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => setPendingDirConflict(null)}
                  disabled={switchingDir}
                  className="btn btn-secondary text-sm"
                >
                  取消
                </button>
                <button
                  onClick={() => applySwitchDir(pendingDirConflict.path, pendingDirConflict.action, true)}
                  disabled={switchingDir}
                  className="px-4 py-2 text-sm bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors disabled:opacity-50"
                >
                  {switchingDir ? '处理中...' : '覆盖目标文件'}
                </button>
              </div>
            </div>
          </div>
        </>
      )}

      {/* 路径无效提示弹窗 */}
      {showPathError && (
        <>
          <div
            className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]"
            onClick={() => setShowPathError(false)}
          />
          <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
            <div className="bg-surface rounded-xl border border-base shadow-lg p-6 max-w-sm w-full pointer-events-auto">
              <h3 className="text-base font-bold text-main mb-2">游戏目录无效</h3>
              <p className="text-sm text-muted mb-5">
                所选目录无法识别为剑网三安装目录。请选择包含 SeasunGame 文件夹的根目录，例如 <span className="font-mono text-main">E:\Game\SeasunGame</span>，系统会自动补全到游戏运行目录。
              </p>
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => setShowPathError(false)}
                  className="btn btn-secondary text-sm"
                >
                  知道了
                </button>
                <button
                  onClick={() => {
                    setShowPathError(false);
                    handleBrowseGameDirectory();
                  }}
                  className="btn btn-primary text-sm"
                >
                  重新选择
                </button>
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  );
};

// 工具函数：格式化发布日期
function formatUpdatePubDate(pubDate: string | undefined): string | null {
  if (!pubDate) return null;
  try {
    const date = new Date(pubDate);
    return date.toLocaleDateString('zh-CN');
  } catch {
    return null;
  }
}
