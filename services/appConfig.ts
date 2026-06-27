import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../types';

export const appConfigService = {
  async get(): Promise<AppConfig> {
    return invoke<AppConfig>('get_app_config');
  },

  async setGameDirectory(path: string): Promise<void> {
    await invoke('set_game_directory', { path });
  },

  async setAccountIds(accountIds: string[]): Promise<void> {
    await invoke('set_account_ids', { accountIds });
  },

  async completeSetup(): Promise<void> {
    await invoke('complete_setup');
  },

  async resetSetup(): Promise<void> {
    await invoke('reset_setup');
  },
};
