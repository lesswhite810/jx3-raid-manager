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

class UpdaterService {
  async getRuntimeInfo(): Promise<UpdateRuntimeInfo> {
    return invoke<UpdateRuntimeInfo>('updater_get_runtime_info');
  }

  async check(): Promise<UpdateCheckResult> {
    const result = await invoke<UpdateCheckResult>('updater_check');
    return normalizeCheckResult(result);
  }

  async downloadAndInstall(
    onProgress?: (payload: UpdateProgressPayload) => void
  ): Promise<void> {
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
    await open(url);
  }
}

export const updaterService = new UpdaterService();
