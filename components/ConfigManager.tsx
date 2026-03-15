import React, { useEffect, useState } from 'react';
import { Config, UpdateCheckResult, UpdateRuntimeInfo, UpdateStatus } from '../types';
import { Check, AlertTriangle, FolderOpen, Download, RefreshCw } from 'lucide-react';
import { saveConfigToStorage, isValidGamePath } from '../utils/configUtils';

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
  const [pathValid, setPathValid] = useState<boolean | null>(null);

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
                {updateCheckResult.pubDate && (
                  <p className="text-xs text-muted mt-1">
                    发布时间：{new Date(updateCheckResult.pubDate).toLocaleString('zh-CN')}
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
    </div>
  );
};
