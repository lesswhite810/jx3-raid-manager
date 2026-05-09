import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-shell';
import { UpdateCheckResult, UpdateProgressPayload, UpdateRuntimeInfo } from '../types';
import { normalizeReleaseNotes } from '../utils/updaterUtils';

export const UPDATER_PROGRESS_EVENT = 'updater://progress';

const normalizeCheckResult = (result: UpdateCheckResult): UpdateCheckResult => ({
  ...result,
  body: normalizeReleaseNotes(result.body)
});

const isTauriEnv = (): boolean => typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;

const browserRuntimeInfo = (): UpdateRuntimeInfo => ({
  currentVersion: 'dev',
  executablePath: '',
  isPortable: true,
  willInstallInPlace: false,
  hasUninstallExecutable: false,
  updaterConfigured: false,
  releaseUrl: ''
});

class UpdaterService {
  async getRuntimeInfo(): Promise<UpdateRuntimeInfo> {
    if (!isTauriEnv()) {
      return browserRuntimeInfo();
    }

    return invoke<UpdateRuntimeInfo>('updater_get_runtime_info');
  }

  async check(): Promise<UpdateCheckResult> {
    if (!isTauriEnv()) {
      return {
        currentVersion: 'dev',
        available: false,
        isPortable: true,
        willInstallInPlace: false,
        updaterConfigured: false,
        releaseUrl: ''
      };
    }

    const result = await invoke<UpdateCheckResult>('updater_check');
    return normalizeCheckResult(result);
  }

  async downloadAndInstall(
    onProgress?: (payload: UpdateProgressPayload) => void
  ): Promise<void> {
    if (!isTauriEnv()) {
      throw new Error('当前构建未启用自动更新');
    }

    const unlisten = await listen<UpdateProgressPayload>(UPDATER_PROGRESS_EVENT, event => {
      onProgress?.(event.payload);
    });

    try {
      await invoke('updater_download_and_install');
    } finally {
      await unlisten();
    }
  }

  async openReleasePage(url: string): Promise<void> {
    if (!isTauriEnv()) {
      window.open(url, '_blank', 'noopener,noreferrer');
      return;
    }

    await open(url);
  }
}

export const updaterService = new UpdaterService();
