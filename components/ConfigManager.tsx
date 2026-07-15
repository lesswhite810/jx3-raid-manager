import React, { useCallback, useEffect, useState } from 'react';
import { UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus, Season } from '../types';
import { Check, AlertTriangle, FolderOpen, Download, RefreshCw, ExternalLink, Search, Monitor, RotateCcw, MessageCircle, Info } from 'lucide-react';
import { isValidGamePath } from '../utils/configUtils';
import { db } from '../services/db';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from '../utils/toastManager';
import { scanJx3Clients, Jx3ClientInfo } from '../services/gameDirectoryScanner';
import { useAppConfig } from '../contexts/AppConfigContext';

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
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [showPathError, setShowPathError] = useState(false);

  const { appConfig, updateGameDirectory, resetAll, setAutoScanEnabled } = useAppConfig();

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
        await db.setCustomDataDir(customPath);
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
      const targetPath = await db.resetCustomDataDir();
      toast.success(`已恢复默认数据目录: ${targetPath}`);
      toast.info('修改将在重启应用后生效，重启时会自动迁移数据库和日志文件');
      await loadDataDirInfo();
    } catch (error) {
      console.error('Failed to reset custom data dir:', error);
      toast.error('恢复默认目录失败: ' + (error instanceof Error ? error.message : String(error)));
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

  const handleAutoScanToggle = async (enabled: boolean) => {
    if (enabled) {
      toast.info('请确保已在茗伊插件中开启：插件集 → 团队 → 团队工具 → 勾选"战斗事件记录"并启用秘境保存');
    }
    await setAutoScanEnabled(enabled);
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
          <div className="p-3 bg-base/50 rounded-lg border border-base">
            <div className="text-xs text-muted mb-1">当前版本</div>
            <div className="text-sm font-medium text-main">v{currentVersion}</div>
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
            <span className="text-sm font-medium text-main">{currentSeason ? currentSeason.name : '加载中...'}</span>
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
