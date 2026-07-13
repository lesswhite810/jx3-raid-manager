import React, { useCallback, useEffect, useState } from 'react';
import { UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus, Season } from '../types';
import { Check, AlertTriangle, RefreshCw, Search, Monitor, X } from 'lucide-react';
import { isValidGamePath } from '../utils/configUtils';
import { db } from '../services/db';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from '../utils/toastManager';
import { scanJx3Clients, Jx3ClientInfo } from '../services/gameDirectoryScanner';
import { useAppConfig } from '../contexts/AppConfigContext';

interface ConfigManagerProps {
  isOpen: boolean;
  onClose: () => void;
  updateRuntimeInfo: UpdateRuntimeInfo | null;
  updateStatus: UpdateStatus;
  updateCheckResult: UpdateCheckResult | null;
  onCheckForUpdates: () => Promise<void>;
}

export const ConfigManager: React.FC<ConfigManagerProps> = ({
  isOpen,
  onClose,
  updateRuntimeInfo,
  updateStatus,
  updateCheckResult,
  onCheckForUpdates
}) => {
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

  const { appConfig, updateGameDirectory, resetAll } = useAppConfig();

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
    if (isOpen) {
      loadDataDirInfo();
      loadCurrentSeason();
    }
  }, [isOpen, loadDataDirInfo, loadCurrentSeason]);

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

  const handleGameDirectoryChange = (value: string) => {
    setGameDirectory(value);
    updateGameDirectory(value).catch(error => {
      console.error('[ConfigManager] 同步游戏目录到 app_config 失败:', error);
    });
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

  const getDataDirLocationText = () => {
    if (!dataDirInfo) return '加载中...';
    switch (dataDirInfo.location) {
      case 'custom': return '自定义目录';
      case 'install': return '安装目录';
      case 'user_home': return '用户目录';
      default: return '未知';
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

  const isUpdateBusy = updateStatus === 'checking' || updateStatus === 'downloading' || updateStatus === 'installing';
  const currentVersion = updateRuntimeInfo?.currentVersion ?? updateCheckResult?.currentVersion ?? '未知';
  const isPortable = updateRuntimeInfo?.isPortable;
  const hasNewVersion = updateCheckResult?.available && updateCheckResult.version;

  return (
    <>
      {/* 遮罩 */}
      <div
        className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[120]"
        onClick={onClose}
      />

      {/* 弹窗主体 */}
      <div className="fixed inset-0 z-[121] flex items-start justify-center p-4 pt-[10vh] pointer-events-none">
        <div className="bg-surface rounded-xl border border-base shadow-lg w-full max-w-lg pointer-events-auto">
          {/* 弹窗头 */}
          <div className="flex items-center justify-between px-5 py-4 border-b border-base">
            <h2 className="text-lg font-bold text-main">配置</h2>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg text-muted hover:text-main hover:bg-base transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* 弹窗内容 */}
          <div className="px-5 py-4 space-y-3">
            {/* 游戏目录 */}
            <div className="flex items-center gap-3">
              <label className="text-sm text-muted whitespace-nowrap w-20">游戏目录</label>
              <div className="flex-1 flex items-center gap-2">
                <input
                  type="text"
                  value={gameDirectory}
                  onChange={(e) => handleGameDirectoryChange(e.target.value)}
                  className="flex-1 px-3 py-1.5 bg-base/50 border border-base rounded-lg text-main focus:bg-surface focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted/50 text-sm"
                  placeholder="如 E:\Game\SeasunGame"
                />
                {pathValid === true && <Check className="w-4 h-4 text-emerald-500 flex-shrink-0" />}
                {pathValid === false && <AlertTriangle className="w-4 h-4 text-red-500 flex-shrink-0" />}
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

            {/* 赛季 */}
            <div className="flex items-center gap-3">
              <label className="text-sm text-muted whitespace-nowrap w-20">当前赛季</label>
              <span className="text-sm text-main">{currentSeason ? currentSeason.name : '加载中...'}</span>
            </div>

            {/* 数据目录 */}
            <div className="flex items-center gap-3">
              <label className="text-sm text-muted whitespace-nowrap w-20">数据目录</label>
              <div className="flex-1 flex items-center gap-2">
                <span className={`px-1.5 py-0.5 rounded text-xs font-medium whitespace-nowrap ${
                  dataDirInfo?.location === 'custom'
                    ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300'
                    : dataDirInfo?.location === 'install'
                    ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300'
                    : 'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300'
                }`}>
                  {getDataDirLocationText()}
                </span>
                <span className="text-sm text-muted font-mono truncate flex-1" title={dataDirInfo?.currentPath}>
                  {dataDirInfo?.currentPath ?? '加载中...'}
                </span>
                <button
                  onClick={handleSelectCustomDataDir}
                  className="btn btn-secondary text-xs px-2 py-1 whitespace-nowrap"
                >
                  切换
                </button>
                {dataDirInfo?.customDirConfigured && (
                  <button
                    onClick={handleResetCustomDataDir}
                    className="btn btn-secondary text-xs px-2 py-1 whitespace-nowrap"
                  >
                    还原
                  </button>
                )}
              </div>
            </div>

            {/* 扫描结果 */}
            {showScanResults && scanResults.length > 0 && (
              <div className="p-3 bg-base/30 rounded-lg border border-base">
                <div className="flex items-center gap-2 mb-2">
                  <Monitor className="w-4 h-4 text-primary" />
                  <span className="text-sm font-medium text-main">已检测到的客户端</span>
                  <button
                    onClick={() => setShowScanResults(false)}
                    className="ml-auto text-xs text-muted hover:text-main transition-colors"
                  >
                    收起
                  </button>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                  {scanResults.map((client, index) => (
                    <div
                      key={index}
                      className="p-2.5 rounded-lg border border-base hover:border-primary/50 hover:bg-surface/50 transition-all cursor-pointer group"
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

            {/* 底部操作行 */}
            <div className="flex items-center justify-between pt-2 border-t border-base">
              <button
                onClick={() => setShowResetConfirm(true)}
                className="text-xs text-muted hover:text-red-500 transition-colors"
              >
                重置配置
              </button>
              <div className="flex items-center gap-1.5 text-xs text-muted">
                <span>v{currentVersion}</span>
                <span>·</span>
                <span>{isPortable ? '便携版' : '安装版'}</span>
                <span>·</span>
                {hasNewVersion ? (
                  <span className="text-emerald-500 font-medium">
                    发现新版本 v{updateCheckResult!.version}
                  </span>
                ) : (
                  <span>{getUpdateStatusText()}</span>
                )}
                <button
                  onClick={onCheckForUpdates}
                  disabled={isUpdateBusy}
                  className="text-primary hover:underline transition-colors disabled:opacity-50 flex items-center gap-1 ml-1"
                >
                  {updateStatus === 'checking' && <RefreshCw className="w-3 h-3 animate-spin" />}
                  {isUpdateBusy ? '检查中...' : '检查更新'}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* 重置确认弹窗（嵌套） */}
      {showResetConfirm && (
        <>
          <div
            className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[130]"
            onClick={() => !resetting && setShowResetConfirm(false)}
          />
          <div className="fixed inset-0 z-[131] flex items-center justify-center p-4 pointer-events-none">
            <div className="bg-surface rounded-xl border border-base shadow-lg p-6 max-w-sm w-full pointer-events-auto">
              <h3 className="text-base font-bold text-main mb-2">确认重新初始化</h3>
              <p className="text-sm text-muted mb-5">
                应用将立即重载，未保存的数据可能丢失。副本记录、AI 配置等数据不会删除。
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
    </>
  );
};
