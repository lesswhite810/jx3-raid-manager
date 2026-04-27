import React, { useCallback, useEffect, useState } from 'react';
import { Config, UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus, Season } from '../types';
import { Check, AlertTriangle, FolderOpen, Download, RefreshCw, Database, ExternalLink, Search, Monitor } from 'lucide-react';
import { isValidGamePath } from '../utils/configUtils';
import { formatUpdatePubDate } from '../utils/updaterUtils';
import { db } from '../services/db';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from '../utils/toastManager';
import { scanJx3Clients, Jx3ClientInfo } from '../services/gameDirectoryScanner';

interface ConfigManagerProps {
  config: Config;
  setConfig: React.Dispatch<React.SetStateAction<Config>>;
  updateRuntimeInfo: UpdateRuntimeInfo | null;
  updateStatus: UpdateStatus;
  updateCheckResult: UpdateCheckResult | null;
  onCheckForUpdates: () => Promise<void>;
}

export const ConfigManager: React.FC<ConfigManagerProps> = ({
  config,
  setConfig,
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
    if (!config.game.gameDirectory) {
      setPathValid(null);
      return;
    }
    isValidGamePath(config.game.gameDirectory).then(result => {
      setPathValid(result.isValid);
    });
  }, [config.game.gameDirectory]);

  const handleScanClients = useCallback(async () => {
    setScanningClients(true);
    setShowScanResults(false);
    try {
      const result = await scanJx3Clients();
      if (result.success && result.clients.length > 0) {
        setScanResults(result.clients);
        setShowScanResults(true);
        if (result.clients.length === 1 && !config.game.gameDirectory) {
          const client = result.clients[0];
          handleConfigChange('game', 'gameDirectory', client.workDirectory);
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
  }, [config.game.gameDirectory]);

  const handleSelectClient = useCallback((client: Jx3ClientInfo) => {
    handleConfigChange('game', 'gameDirectory', client.workDirectory);
    setShowScanResults(false);
    toast.success(`已选择 ${client.displayName}`);
  }, []);

  const handleConfigChange = (section: keyof Config, key: string, value: unknown) => {
    const nextConfig = { ...config, [section]: { ...config[section], [key]: value } };
    setConfig(nextConfig);
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

  return (
    <div className="space-y-6">
      <div className="space-y-2">
        <h2 className="text-2xl font-bold text-main">系统配置</h2>
        <p className="text-sm text-muted">配置项修改后会自动保存，无需手动点击保存。</p>
      </div>

      <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
        <div className="flex items-center justify-between gap-4 mb-6">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
              <Download className="w-5 h-5" />
            </div>
            <div>
              <h3 className="text-lg font-bold text-main">版本与更新</h3>
              <p className="text-xs text-muted">安装版支持应用内更新，便携版仅提供下载提示</p>
            </div>
          </div>
          <button
            onClick={onCheckForUpdates}
            disabled={updateStatus === 'checking' || updateStatus === 'downloading' || updateStatus === 'installing'}
            className="btn btn-secondary flex items-center gap-2 text-sm"
          >
            <RefreshCw className={`w-4 h-4 ${updateStatus === 'checking' ? 'animate-spin' : ''}`} />
            {updateStatus === 'checking' ? '检查中...' : '检查更新'}
          </button>
        </div>

        <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
          <InfoCard label="当前版本" value={`v${updateRuntimeInfo?.currentVersion ?? updateCheckResult?.currentVersion ?? '未知'}`} />
          <InfoCard label="运行形态" value={updateRuntimeInfo?.isPortable ? '便携版' : '安装版'} />
          <InfoCard label="更新状态" value={getUpdateStatusText()} />
        </div>

        {updateCheckResult?.available && updateCheckResult.version && (
          <div className="mt-4 p-4 bg-base/50 rounded-lg border border-base">
            <div className="flex items-center gap-3">
              <div className="flex-1">
                <span className="text-sm font-medium text-main">发现新版本 v{updateCheckResult.version}</span>
                {publishedAtText && (
                  <span className="text-xs text-muted ml-2">发布时间：{publishedAtText}</span>
                )}
              </div>
              <span className="text-xs text-muted">
                {updateCheckResult.isPortable
                  ? '便携版检测到新版本后会跳转到 GitHub 下载'
                  : '确认更新后将下载安装包执行升级'}
              </span>
            </div>
          </div>
        )}
      </div>

      <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
            <FolderOpen className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-lg font-bold text-main">游戏配置</h3>
            <p className="text-xs text-muted">设置游戏安装路径以实现自动扫描等功能</p>
          </div>
        </div>

        <div className="space-y-4">
          <div className="flex items-center gap-4">
            <label className="text-sm font-medium text-muted whitespace-nowrap">当前赛季</label>
            <span className="text-sm font-medium text-main">{currentSeason ? currentSeason.name : '加载中...'}</span>
          </div>

          <div className="flex items-center gap-4">
            <label className="text-sm font-medium text-muted whitespace-nowrap">游戏安装目录</label>
            <div className="flex-1 flex items-center gap-2">
              <input
                type="text"
                value={config.game.gameDirectory}
                onChange={(e) => handleConfigChange('game', 'gameDirectory', e.target.value)}
                className="flex-1 px-3 py-2 bg-base/50 border border-base rounded-lg text-main focus:bg-surface focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted/50 text-sm"
                placeholder="输入剑网三安装目录，例如 E:\Game\SeasunGame"
              />
              <button
                onClick={handleScanClients}
                disabled={scanningClients}
                className="btn btn-secondary flex items-center gap-2 text-sm whitespace-nowrap"
                title="从注册表扫描剑网3客户端"
              >
                {scanningClients ? (
                  <RefreshCw className="w-4 h-4 animate-spin" />
                ) : (
                  <Search className="w-4 h-4" />
                )}
                {scanningClients ? '扫描中...' : '扫描'}
              </button>
            </div>
          </div>
          <div className="flex items-center gap-4 pl-[7.5rem]">
            {pathValid === false && (
              <div className="flex items-center gap-1.5 text-xs text-red-500">
                <AlertTriangle className="w-3.5 h-3.5" />
                <span>游戏目录路径无效</span>
              </div>
            )}
            {pathValid === true && (
              <div className="flex items-center gap-1.5 text-xs text-emerald-500">
                <Check className="w-3.5 h-3.5" />
                <span>游戏目录路径有效</span>
              </div>
            )}
            <span className="text-xs text-muted">支持填写安装根目录，运行时会自动补全到 Game\JX3\bin\zhcn_hd</span>
          </div>

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
        </div>
      </div>

      <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
            <Database className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-lg font-bold text-main">数据存储</h3>
            <p className="text-xs text-muted">配置数据库与日志文件的存储位置</p>
          </div>
        </div>

        <div className="space-y-4">
          <div className="flex items-center gap-4">
            <div className="flex-1">
              <div className="flex items-center gap-2 mb-2">
                <span className={`px-2 py-0.5 rounded text-xs font-medium ${
                  dataDirInfo?.location === 'custom'
                    ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300'
                    : dataDirInfo?.location === 'install'
                    ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300'
                    : 'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300'
                }`}>
                  {getDataDirLocationText()}
                </span>
                {dataDirInfo?.isInstallMode && dataDirInfo?.location === 'install' && (
                  <span className="px-2 py-0.5 rounded text-xs font-medium bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-300">
                    安装版
                  </span>
                )}
              </div>
              <p className="text-sm text-main break-all font-mono select-all">
                {dataDirInfo?.currentPath ?? '加载中...'}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={handleSelectCustomDataDir}
                className="btn btn-secondary flex items-center gap-2 text-sm"
              >
                <ExternalLink className="w-4 h-4" />
                切换目录
              </button>
              {dataDirInfo?.customDirConfigured && (
                <button
                  onClick={handleResetCustomDataDir}
                  className="btn btn-secondary text-sm"
                >
                  还原默认
                </button>
              )}
            </div>
          </div>
          <p className="text-xs text-muted">
            修改目录后需要重启应用才能生效，重启时会自动迁移数据库和日志文件
          </p>
        </div>
      </div>
    </div>
  );
};

interface InfoCardProps {
  label: string;
  value: string;
  badge?: string;
  badgeClass?: string;
}

const InfoCard: React.FC<InfoCardProps> = ({ label, value, badge, badgeClass }) => (
  <div className="p-3 bg-base/50 rounded-lg border border-base">
    <div className="text-xs text-muted mb-1">{label}</div>
    <div className="flex items-center gap-2">
      <span className="text-sm font-medium text-main">{value}</span>
      {badge && (
        <span className={`px-1.5 py-0.5 rounded text-xs font-medium ${badgeClass || 'bg-primary/10 text-primary'}`}>
          {badge}
        </span>
      )}
    </div>
  </div>
);
