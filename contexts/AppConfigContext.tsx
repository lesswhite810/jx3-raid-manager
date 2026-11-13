import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';
import type { AppConfig, CloseBehavior, RaidViewMode } from '../types';
import { appConfigService } from '../services/appConfig';
import { toast } from '../utils/toastManager';

interface AppConfigContextType {
  appConfig: AppConfig | null;
  isLoading: boolean;
  /** 设置游戏目录：写入 app_config 表（唯一存储源，不再同步到 config_json） */
  updateGameDirectory: (path: string) => Promise<void>;
  /** 标记引导流程完成 */
  markSetupCompleted: () => Promise<void>;
  /** 重新初始化（清空配置后重载应用） */
  resetAll: () => Promise<void>;
  /** 设置自动扫描开关 */
  setAutoScanEnabled: (enabled: boolean) => Promise<void>;
  /** 设置试炼结算翻牌位截图识别开关 */
  setTrialScreenshotRecognitionEnabled: (enabled: boolean) => Promise<void>;
  /** 设置启动刷新装分开关 */
  setAutoRefreshEquipScoreEnabled: (enabled: boolean) => Promise<void>;
  /** 设置关闭按钮行为（ask 每次询问 / minimize 最小化到托盘 / exit 直接退出） */
  setCloseBehavior: (behavior: CloseBehavior) => Promise<void>;
  /** 设置副本管理显示模式（current 当前模式 CD 概览 / history 历史模式 页签 + 收藏界面） */
  setRaidViewMode: (mode: RaidViewMode) => Promise<void>;
  /** 设置 CD 概览展示周常开关（关闭时仅展示当前赛季副本） */
  setShowWeeklyRaidsEnabled: (enabled: boolean) => Promise<void>;
  /** 设置副本管理隐藏非满级角色开关（开启时界面仅显示等级达到当前版本上限的角色） */
  setHideNonMaxLevelRolesEnabled: (enabled: boolean) => Promise<void>;
}

const AppConfigContext = createContext<AppConfigContextType | undefined>(undefined);

const DEFAULT_APP_CONFIG: AppConfig = {
  gameDirectory: null,
  setupCompleted: false,
  lastScanMingyiAt: null,
  autoScanEnabled: false,
  trialScreenshotRecognitionEnabled: false,
  autoRefreshEquipScore: true,
  closeBehavior: 'ask',
  raidViewMode: 'current',
  showWeeklyRaids: true,
  hideNonMaxLevelRoles: false,
};

export const AppConfigProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const loadConfig = async () => {
      try {
        const config = await appConfigService.get();
        if (!cancelled) {
          // 新增字段在旧的本地配置中可能缺失，保持截图识别默认关闭、关闭行为默认每次询问。
          setAppConfig({
            ...DEFAULT_APP_CONFIG,
            ...config,
            trialScreenshotRecognitionEnabled: config.trialScreenshotRecognitionEnabled ?? false,
            closeBehavior: config.closeBehavior ?? 'ask',
            raidViewMode: config.raidViewMode ?? 'current',
            showWeeklyRaids: config.showWeeklyRaids ?? true,
            hideNonMaxLevelRoles: config.hideNonMaxLevelRoles ?? false,
          });
        }
      } catch (error) {
        console.error('[AppConfig] 加载应用配置失败:', error);
        if (!cancelled) {
          // 加载失败时使用默认配置，避免阻塞应用启动
          setAppConfig(DEFAULT_APP_CONFIG);
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    };
    loadConfig();
    return () => {
      cancelled = true;
    };
  }, []);

  const updateGameDirectory = useCallback(async (path: string) => {
    try {
      await appConfigService.setGameDirectory(path);
      setAppConfig(prev => (prev ? { ...prev, gameDirectory: path } : { ...DEFAULT_APP_CONFIG, gameDirectory: path }));
    } catch (error) {
      console.error('[AppConfig] 设置游戏目录失败:', error);
      toast.error('保存游戏目录失败');
      throw error;
    }
  }, []);

  const markSetupCompleted = useCallback(async () => {
    try {
      await appConfigService.completeSetup();
      setAppConfig(prev => (prev ? { ...prev, setupCompleted: true } : { ...DEFAULT_APP_CONFIG, setupCompleted: true }));
    } catch (error) {
      console.error('[AppConfig] 标记引导完成失败:', error);
      toast.error('完成引导失败');
      throw error;
    }
  }, []);

  const resetAll = useCallback(async () => {
    try {
      await appConfigService.resetSetup();
      setAppConfig(DEFAULT_APP_CONFIG);
      // 重载应用，回到 SetupGuide
      window.location.reload();
    } catch (error) {
      console.error('[AppConfig] 重新初始化失败:', error);
      toast.error('重新初始化失败');
      throw error;
    }
  }, []);

  const setAutoScanEnabled = useCallback(async (enabled: boolean) => {
    try {
      await appConfigService.setAutoScanEnabled(enabled);
      setAppConfig(prev => (prev ? { ...prev, autoScanEnabled: enabled } : { ...DEFAULT_APP_CONFIG, autoScanEnabled: enabled }));
    } catch (error) {
      console.error('[AppConfig] 设置自动扫描开关失败:', error);
      toast.error('设置自动扫描开关失败');
      throw error;
    }
  }, []);

  const setTrialScreenshotRecognitionEnabled = useCallback(async (enabled: boolean) => {
    try {
      await appConfigService.setTrialScreenshotRecognitionEnabled(enabled);
      setAppConfig(prev => (
        prev
          ? { ...prev, trialScreenshotRecognitionEnabled: enabled }
          : { ...DEFAULT_APP_CONFIG, trialScreenshotRecognitionEnabled: enabled }
      ));
    } catch (error) {
      console.error('[AppConfig] 设置试炼截图识别开关失败:', error);
      toast.error('保存试炼截图识别设置失败');
      throw error;
    }
  }, []);

  const setAutoRefreshEquipScoreEnabled = useCallback(async (enabled: boolean) => {
    try {
      await appConfigService.setAutoRefreshEquipScoreEnabled(enabled);
      setAppConfig(prev => (prev ? { ...prev, autoRefreshEquipScore: enabled } : { ...DEFAULT_APP_CONFIG, autoRefreshEquipScore: enabled }));
    } catch (error) {
      console.error('[AppConfig] 设置启动刷新装分开关失败:', error);
      toast.error('设置启动刷新装分开关失败');
      throw error;
    }
  }, []);

  const setCloseBehavior = useCallback(async (behavior: CloseBehavior) => {
    try {
      await appConfigService.setCloseBehavior(behavior);
      setAppConfig(prev => (prev ? { ...prev, closeBehavior: behavior } : { ...DEFAULT_APP_CONFIG, closeBehavior: behavior }));
    } catch (error) {
      console.error('[AppConfig] 设置关闭按钮行为失败:', error);
      toast.error('保存关闭按钮行为失败');
      throw error;
    }
  }, []);

  const setRaidViewMode = useCallback(async (mode: RaidViewMode) => {
    try {
      await appConfigService.setRaidViewMode(mode);
      setAppConfig(prev => (prev ? { ...prev, raidViewMode: mode } : { ...DEFAULT_APP_CONFIG, raidViewMode: mode }));
    } catch (error) {
      console.error('[AppConfig] 设置副本管理显示模式失败:', error);
      toast.error('保存副本管理显示模式失败');
      throw error;
    }
  }, []);

  const setShowWeeklyRaidsEnabled = useCallback(async (enabled: boolean) => {
    try {
      await appConfigService.setShowWeeklyRaidsEnabled(enabled);
      setAppConfig(prev => (prev ? { ...prev, showWeeklyRaids: enabled } : { ...DEFAULT_APP_CONFIG, showWeeklyRaids: enabled }));
    } catch (error) {
      console.error('[AppConfig] 设置展示周常开关失败:', error);
      toast.error('保存展示周常设置失败');
      throw error;
    }
  }, []);

  const setHideNonMaxLevelRolesEnabled = useCallback(async (enabled: boolean) => {
    try {
      await appConfigService.setHideNonMaxLevelRolesEnabled(enabled);
      setAppConfig(prev => (prev ? { ...prev, hideNonMaxLevelRoles: enabled } : { ...DEFAULT_APP_CONFIG, hideNonMaxLevelRoles: enabled }));
    } catch (error) {
      console.error('[AppConfig] 设置隐藏非满级角色开关失败:', error);
      toast.error('保存隐藏非满级角色设置失败');
      throw error;
    }
  }, []);

  return (
    <AppConfigContext.Provider
      value={{
        appConfig,
        isLoading,
        updateGameDirectory,
        markSetupCompleted,
        resetAll,
        setAutoScanEnabled,
        setTrialScreenshotRecognitionEnabled,
        setAutoRefreshEquipScoreEnabled,
        setCloseBehavior,
        setRaidViewMode,
        setShowWeeklyRaidsEnabled,
        setHideNonMaxLevelRolesEnabled,
      }}
    >
      {children}
    </AppConfigContext.Provider>
  );
};

export const useAppConfig = (): AppConfigContextType => {
  const context = useContext(AppConfigContext);
  if (context === undefined) {
    throw new Error('useAppConfig must be used within an AppConfigProvider');
  }
  return context;
};
