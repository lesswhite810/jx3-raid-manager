import { invoke } from '@tauri-apps/api/core';
import { ParsedAccount, ParsedRole } from './directoryParser';

// 重新导出 ParsedRole
export type { ParsedRole };

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

// 自动解析结果
export interface AutoParseResult {
  success: boolean;
  newAccounts: number;
  updatedAccounts: number;
  newRoles: number;
  updatedRoles: number;
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

/**
 * 自动解析游戏目录并保存到数据库
 * 后端完成解析、合并、入库，前端仅触发和查询
 */
export async function autoParseGameDirectory(gameDirectory: string): Promise<AutoParseResult> {
  try {
    const result = await invoke<AutoParseResult>('auto_parse_game_directory', {
      gameDirectory
    });
    return result;
  } catch (error) {
    return {
      success: false,
      newAccounts: 0,
      updatedAccounts: 0,
      newRoles: 0,
      updatedRoles: 0,
      error: `自动解析失败: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}

// 客户端类型
export type ClientType =
  | 'zhcn_hd'
  | 'zhcn_exp'
  | 'zhcn_tw'
  | 'classic_yq'
  | 'classic_exp';

// 扫描到的客户端信息
export interface Jx3ClientInfo {
  clientType: ClientType;
  displayName: string;
  installPath: string;
  workDirectory: string;
  version?: string;
}

// 扫描结果
export interface ScanClientsResult {
  success: boolean;
  clients: Jx3ClientInfo[];
  error?: string;
}

/**
 * 扫描已安装的剑网3客户端
 * 从 Windows 注册表读取客户端安装信息
 */
export async function scanJx3Clients(): Promise<ScanClientsResult> {
  try {
    const result = await invoke<ScanClientsResult>('scan_jx3_clients');
    return result;
  } catch (error) {
    return {
      success: false,
      clients: [],
      error: `扫描失败: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}
