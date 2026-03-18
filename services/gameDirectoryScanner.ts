import { invoke } from '@tauri-apps/api/core';
import { ParsedAccount } from './directoryParser';

export interface GkpFileInfo {
  filePath: string;
  fileName: string;
  timestamp: number;
  playerCount: number;
  mapName: string;
  roleName?: string;
}

export interface GameDirectoryScanResult {
  success: boolean;
  accounts: ParsedAccount[];
  gkpFiles: GkpFileInfo[];
  error?: string;
}

export interface ScanProgress {
  current: number;
  total: number;
  message: string;
}

export interface ScanOptions {
  gameDirectory: string;
  onProgress?: (progress: ScanProgress) => void;
  activeRoles?: Array<{ name: string; server: string; region: string }>;
}

export async function scanGameDirectory(options: ScanOptions): Promise<GameDirectoryScanResult> {
  try {
    const { gameDirectory, onProgress, activeRoles } = options;

    onProgress?.({ current: 0, total: 2, message: '正在扫描账号信息...' });

    const result = await invoke<GameDirectoryScanResult>('scan_game_directory', {
      gameDirectory,
      activeRoles: activeRoles ?? []
    });

    onProgress?.({ current: 1, total: 2, message: '正在扫描GKP文件...' });
    onProgress?.({ current: 2, total: 2, message: '扫描完成' });

    return result;
  } catch (error) {
    return {
      success: false,
      accounts: [],
      gkpFiles: [],
      error: `扫描失败: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}

export async function validateGameDirectory(gameDirectory: string): Promise<boolean> {
  try {
    const result = await invoke<{ isValid: boolean }>('validate_game_directory', {
      gameDirectory
    });
    return result.isValid;
  } catch (error) {
    console.warn('验证游戏目录失败:', error);
    return false;
  }
}
