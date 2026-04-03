import React, { useCallback, useEffect, useState } from 'react';
import { Config, UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus } from '../types';
import { Check, AlertTriangle, FolderOpen, Download, RefreshCw, Database, ExternalLink } from 'lucide-react';
import { saveConfigToStorage, isValidGamePath } from '../utils/configUtils';
import { formatUpdatePubDate } from '../utils/updaterUtils';
import { db } from '../services/db';
import { open } from '@tauri-apps/plugin-dialog';
import { toast } from '../utils/toastManager';

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

  const loadDataDirInfo = useCallback(async () => {
    try {
      const info = await db.getDataDirInfo();
      setDataDirInfo(info);
    } catch (error) {
      console.error('Failed to load data dir info:', error);
    }
  }, []);

  // 加载数据目录信息
  useEffect(() => {
    loadDataDirInfo();
  }, [loadDataDirInfo]);

  useEffect(() => {
    if (!config.game.gameDirectory) {
      setPathValid(null);
      return;
    }

    isValidGamePath(config.game.gameDirectory).then(result => {
      setPathValid(result.isValid);
    });
  }, [config.game.gameDirectory]);

  const handleConfigChange = (section: keyof Config, key: string, value: unknown) => {
    const nextConfig = { ...config, [section]: { ...config[section], [key]: value } };
    setConfig(nextConfig);

    if (section === 'game' && key === 'gameDirectory') {
      saveConfigToStorage(nextConfig);
    }
  };

  const getUpdateStatusText = () => {
    switch (updateStatus) {
      case 'checking':
        return '正在检查更新';
      case 'available':
        return '发现新版本';
      case 'downloading':
        return '正在下载更新';
      case 'installing':
        return '正在安装更新';
      case 'upToDate':
        return '当前已是最新版本';
      case 'portableManualOnly':
        return '便携版需手动下载更新';
      case 'error':
        return '检查更新失败';
      default:
        return updateRuntimeInfo?.updaterConfigured ? '尚未检查更新' : '当前构建未启用自动更新';
    }
  };

  // 选择自定义数据目录
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
      case 'custom':
        return '自定义目录';
      case 'install':
        return '安装目录';
      case 'user_home':
        return '用户目录';
      default:
        return '未知';
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

        <div className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
            <label className="text-sm font-medium text-muted">当前版本</label>
            <div className="col-span-2 text-sm text-main font-medium">
              v{updateRuntimeInfo?.currentVersion ?? updateCheckResult?.currentVersion ?? '未知'}
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
            <label className="text-sm font-medium text-muted">运行形态</label>
            <div className="col-span-2 text-sm text-main">
              {updateRuntimeInfo?.isPortable ? '便携版' : '安装版'}
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
            <label className="text-sm font-medium text-muted">更新状态</label>
            <div className="col-span-2 text-sm text-main">{getUpdateStatusText()}</div>
          </div>

          {updateCheckResult?.available && updateCheckResult.version && (
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-start">
              <label className="text-sm font-medium text-muted pt-0.5">最新版本</label>
              <div className="col-span-2">
                <div className="text-sm font-medium text-main">v{updateCheckResult.version}</div>
                {publishedAtText && (
                  <p className="text-xs text-muted mt-1">
                    发布时间：{publishedAtText}
                  </p>
                )}
                <p className="text-xs text-muted mt-1">
                  {updateCheckResult.isPortable
                    ? '当前为便携版，检测到新版本后会跳转到 GitHub Release 下载页面。'
                    : '确认更新后将下载安装包，并按当前安装路径执行升级。'}
                </p>
              </div>
            </div>
          )}
        </div>
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
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
            <label className="text-sm font-medium text-muted">游戏安装目录</label>
            <div className="col-span-2">
              <input
                type="text"
                value={config.game.gameDirectory}
                onChange={(e) => handleConfigChange('game', 'gameDirectory', e.target.value)}
                className="w-full px-3 py-2 bg-base/50 border border-base rounded-lg text-main focus:bg-surface focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted/50 text-sm"
                placeholder="输入剑网三安装目录，例如 E:\\Game\\SeasunGame"
              />
              <p className="mt-2 text-xs text-muted">
                支持填写安装根目录，运行时会自动补全到 Game\JX3\bin\zhcn_hd。
              </p>

              {pathValid === false && (
                <div className="flex items-center gap-1.5 text-xs text-red-500 mt-2">
                  <AlertTriangle className="w-3.5 h-3.5" />
                  <span>游戏目录路径无效</span>
                </div>
              )}

              {pathValid === true && (
                <div className="flex items-center gap-1.5 text-xs text-emerald-500 mt-2">
                  <Check className="w-3.5 h-3.5" />
                  <span>游戏目录路径有效</span>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* 数据目录配置 */}
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
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-start">
            <label className="text-sm font-medium text-muted pt-0.5">当前数据目录</label>
            <div className="col-span-2">
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
                    安装版默认
                  </span>
                )}
              </div>
              <div className="py-1">
                <p className="text-sm text-main break-all font-mono select-all">
                  {dataDirInfo?.currentPath ?? '加载中...'}
                </p>
              </div>
              <p className="mt-2 text-xs text-muted">
                {dataDirInfo?.location === 'install'
                  ? '当前使用安装目录存储数据，适合安装版用户'
                  : dataDirInfo?.location === 'custom'
                  ? '当前使用自定义目录存储数据'
                  : '当前使用用户目录存储数据'}
              </p>
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
            <label className="text-sm font-medium text-muted">自定义目录</label>
            <div className="col-span-2">
              <div className="flex items-center gap-2">
                <button
                  onClick={handleSelectCustomDataDir}
                  className="btn btn-secondary flex items-center gap-2 text-sm"
                >
                  <ExternalLink className="w-4 h-4" />
                  选择其他目录
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
              <p className="mt-2 text-xs text-muted">
                修改后需要重启应用才能生效。下次启动时会自动将数据库和日志迁移到目标目录。
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
