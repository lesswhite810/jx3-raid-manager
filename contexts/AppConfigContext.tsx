import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';
import type { AppConfig } from '../types';
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
  /** 设置启动刷新装分开关 */
  setAutoRefreshEquipScoreEnabled: (enabled: boolean) => Promise<void>;
}

const AppConfigContext = createContext<AppConfigContextType | undefined>(undefined);

const DEFAULT_APP_CONFIG: AppConfig = {
  gameDirectory: null,
  setupCompleted: false,
  lastScanMingyiAt: null,
  autoScanEnabled: false,
  autoRefreshEquipScore: true,
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
          setAppConfig(config);
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

  return (
    <AppConfigContext.Provider
      value={{
        appConfig,
        isLoading,
        updateGameDirectory,
        markSetupCompleted,
        resetAll,
        setAutoScanEnabled,
        setAutoRefreshEquipScoreEnabled,
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
