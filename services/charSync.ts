import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { mockInvoke } from './mockInvoke';

// 环境检测：纯浏览器环境使用 mock（与 db.ts / appConfig.ts 保持一致）
const isBrowserEnv = !window.__TAURI_INTERNALS__ && typeof window !== 'undefined';

const invoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  if (isBrowserEnv) {
    return mockInvoke<T>(cmd, args);
  }
  return tauriInvoke<T>(cmd, args);
};

/** 角色目录定位信息 */
export interface RoleDirInfo {
  accountName: string;
  region: string;
  server: string;
  roleName: string;
  /** userdata 下的完整角色目录绝对路径 */
  roleDir: string;
  /** 从 info.jx3dat 解析的 uid（预留字段，MVP 未使用） */
  uid?: string | null;
}

/** 同步选项（与 Rust 端 SyncOptions 对齐，camelCase） */
export interface SyncOptions {
  /** 同步 addon.jx3dat（插件启用状态） */
  syncAddonJx3dat: boolean;
  /** 同步 custom.dat + custom.dat.addon（聊天自定义） */
  syncCustom: boolean;
  /** 同步 hotkey.data + hotkey_*.txt（热键索引+定义） */
  syncHotkey: boolean;
  /** 同步 userpreferences.jx3dat（核心：UI设置/技能栏/气场可见性等） */
  syncUserprefs: boolean;
  /** 覆盖 vs 跳过已存在 */
  overwrite: boolean;
}

/** 单次同步结果 */
export interface SyncResult {
  success: boolean;
  copiedFiles: string[];
  skippedFiles: string[];
  backupDir?: string;
  error?: string;
}

/** 备份条目（同步历史） */
export interface BackupEntry {
  backupDir: string;
  timestamp: string;
  targetRole: string;
  /** 精确目标角色目录（来自备份元数据；旧备份可能为空，需按角色名降级匹配） */
  targetRoleDir?: string | null;
  fileCount: number;
}

export const charSyncService = {
  /** 列出当前游戏目录下所有可同步角色 */
  async listSyncableRoles(gameDirectory: string): Promise<RoleDirInfo[]> {
    return invoke<RoleDirInfo[]>('list_syncable_roles', { gameDirectory });
  },

  /** 执行跨角色设置同步 */
  async syncCharacterSettings(
    gameDirectory: string,
    source: RoleDirInfo,
    target: RoleDirInfo,
    options: SyncOptions
  ): Promise<SyncResult> {
    return invoke<SyncResult>('sync_character_settings', {
      gameDirectory,
      source,
      target,
      options,
    });
  },

  /** 列出本地备份目录（即同步历史，无需 DB） */
  async listBackups(): Promise<BackupEntry[]> {
    return invoke<BackupEntry[]>('list_char_sync_backups');
  },

  /** 从备份回滚目标角色设置（目标目录必须在当前游戏目录下） */
  async rollback(gameDirectory: string, backupDir: string, targetRoleDir: string): Promise<void> {
    await invoke('rollback_char_sync', { gameDirectory, backupDir, targetRoleDir });
  },

  /** 备份回滚标记 CRUD（V18 起存储在 SQLite，之前存于 webview localStorage） */
  rollbackMarks: {
    /** 列出所有已回滚的备份目录 → 回滚时间（ISO） */
    async list(): Promise<Record<string, string>> {
      return invoke<Record<string, string>>('list_char_sync_rollback_marks');
    },
    /** 写入或覆盖单个标记（同 backup_dir 重复写入取最新） */
    async set(backupDir: string, timestamp: string): Promise<void> {
      await invoke('set_char_sync_rollback_mark', { args: { backupDir, timestamp } });
    },
    /** 清空所有标记（通常不需要，数据库重建后表自然为空） */
    async clear(): Promise<void> {
      await invoke('clear_char_sync_rollback_marks');
    },
  },
};
