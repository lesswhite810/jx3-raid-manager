import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { mockInvoke } from './mockInvoke';
import type { AppConfig, CloseBehavior, RaidViewMode } from '../types';

// 环境检测：纯浏览器环境使用 mock（与 db.ts 保持一致）
const isBrowserEnv = !window.__TAURI_INTERNALS__ && typeof window !== 'undefined';

const invoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  if (isBrowserEnv) {
    return mockInvoke<T>(cmd, args);
  }
  return tauriInvoke<T>(cmd, args);
};

export const appConfigService = {
  async get(): Promise<AppConfig> {
    return invoke<AppConfig>('get_app_config');
  },

  async setGameDirectory(path: string): Promise<void> {
    await invoke('set_game_directory', { path });
  },

  async completeSetup(): Promise<void> {
    await invoke('complete_setup');
  },

  async resetSetup(): Promise<void> {
    await invoke('reset_setup');
  },

  async setAutoScanEnabled(enabled: boolean): Promise<void> {
    await invoke('set_auto_scan_enabled', { enabled });
  },

  async setTrialScreenshotRecognitionEnabled(enabled: boolean): Promise<void> {
    await invoke('set_trial_screenshot_recognition_enabled', { enabled });
  },

  async setAutoRefreshEquipScoreEnabled(enabled: boolean): Promise<void> {
    await invoke('set_auto_refresh_equip_score_enabled', { enabled });
  },

  /** 保存关闭按钮行为，下次点击关闭时不再重复询问（设为 ask 则恢复每次询问） */
  async setCloseBehavior(behavior: CloseBehavior): Promise<void> {
    await invoke('set_close_behavior', { behavior });
  },

  /** 保存副本管理显示模式（current 当前模式 / history 历史模式） */
  async setRaidViewMode(mode: RaidViewMode): Promise<void> {
    await invoke('set_raid_view_mode', { mode });
  },

  /** 保存 CD 概览展示周常开关（关闭时仅展示当前赛季副本） */
  async setShowWeeklyRaidsEnabled(enabled: boolean): Promise<void> {
    await invoke('set_show_weekly_raids_enabled', { enabled });
  },

  /** 保存副本管理隐藏非满级角色开关（开启时界面仅显示等级达到当前版本上限的角色） */
  async setHideNonMaxLevelRolesEnabled(enabled: boolean): Promise<void> {
    await invoke('set_hide_non_max_level_roles_enabled', { enabled });
  },
};
