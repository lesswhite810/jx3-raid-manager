import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { mockInvoke } from './mockInvoke';
import type { GameVersion, Season, RaidRecord, Account, Role, Raid, Config, TrialPlaceRecord, BaizhanRecord, InstanceType, RoleInstanceVisibility } from '../types';

// 环境检测：如果没有注入 __TAURI_INTERNALS__ ，说明是在纯浏览器环境运行
// __TAURI_INTERNALS__ 类型声明位于 src/tauri.d.ts
const isBrowserEnv = !window.__TAURI_INTERNALS__ && typeof window !== 'undefined';

// 包装一个通用的 invoke 函数
const invoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  if (isBrowserEnv) {
    return mockInvoke<T>(cmd, args);
  }
  return tauriInvoke<T>(cmd, args);
};
class DatabaseService {
  private initialized = false;

  async init(): Promise<void> {
    if (this.initialized) return;
    try {
      await invoke('db_init');
      this.initialized = true;
    } catch (error) {
      console.error('Failed to initialize database:', error);
      throw error;
    }
  }

  // ========== 版本管理 ==========

  /// 获取数据库版本信息
  async getVersionInfo(): Promise<{
    schemaVersion: number;
    currentVersion: number;
    isLatest: boolean;
    localStorageMigrated: boolean;
  } | null> {
    await this.init();
    try {
      return await invoke('db_get_version_info');
    } catch (error) {
      console.error('Failed to get version info:', error);
      return null;
    }
  }

  /// 检查 localStorage 迁移是否已完成
  async isLocalStorageMigrated(): Promise<boolean> {
    await this.init();
    try {
      return await invoke<boolean>('db_is_local_storage_migrated');
    } catch (error) {
      console.error('Failed to check localStorage migrated:', error);
      return false;
    }
  }

  /// 标记 localStorage 迁移已完成
  async setLocalStorageMigrated(): Promise<void> {
    await this.init();
    try {
      await invoke('db_set_local_storage_migrated');
    } catch (error) {
      console.error('Failed to set localStorage migrated:', error);
      throw error;
    }
  }

  // ========== 账号管理 ==========

  async getAccounts(): Promise<Account[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_accounts_with_roles');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get accounts:', error);
      return [];
    }
  }

  async saveAccounts(accounts: Account[]): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_accounts', { accounts: JSON.stringify(accounts) });
    } catch (error) {
      console.error('Failed to save accounts:', error);
      throw error;
    }
  }

  // Structured accounts API (V1+)
  async getAccountsStructured(): Promise<Account[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_accounts_structured');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get structured accounts:', error);
      return [];
    }
  }

  async getRolesByAccount(accountId: string): Promise<Role[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_roles_by_account', { accountId });
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get roles by account:', error);
      return [];
    }
  }

  async getAllRoles(): Promise<Role[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_all_roles');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get all roles:', error);
      return [];
    }
  }

  async saveAccountStructured(account: Account): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_account_structured', { accountJson: JSON.stringify(account) });
    } catch (error) {
      console.error('Failed to save structured account:', error);
      throw error;
    }
  }

  async saveRoleStructured(role: Role): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_role_structured', { roleJson: JSON.stringify(role) });
    } catch (error) {
      console.error('Failed to save structured role:', error);
      throw error;
    }
  }

  async deleteAccountStructured(accountId: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_delete_account_structured', { accountId });
    } catch (error) {
      console.error('Failed to delete structured account:', error);
      throw error;
    }
  }

  async deleteRoleStructured(roleId: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_delete_role_structured', { roleId });
    } catch (error) {
      console.error('Failed to delete structured role:', error);
      throw error;
    }
  }

  async deleteDirectory(path: string, targetType: string): Promise<{ deleted: boolean; path: string }> {
    try {
      return await invoke<{ deleted: boolean; path: string }>('db_delete_directory', { path, targetType });
    } catch (error) {
      console.error('Failed to delete directory:', error);
      throw error;
    }
  }

  /// 根据复合键生成确定性 UUID，与后端 generate_uuid_from_key 一致
  /// 前端手动添加账号/角色时使用，确保删除重建后 ID 不变
  async generateDeterministicUUID(key: string): Promise<string> {
    try {
      return await invoke<string>('generate_deterministic_uuid', { key });
    } catch (error) {
      console.error('Failed to generate deterministic UUID, falling back to random:', error);
      // 降级：返回随机 UUID（不影响功能，但不具备确定性）
      if (typeof crypto !== 'undefined' && crypto.randomUUID) {
        return crypto.randomUUID();
      }
      return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
        const r = Math.random() * 16 | 0;
        const v = c === 'x' ? r : (r & 0x3 | 0x8);
        return v.toString(16);
      });
    }
  }

  async getRecords(): Promise<RaidRecord[]> {
    await this.init();
    try {
      const data = await invoke<string[]>('db_get_records');
      return data.map(item => JSON.parse(item));
    } catch (error) {
      console.error('Failed to get records:', error);
      return [];
    }
  }

  async getPendingRecords(): Promise<RaidRecord[]> {
    await this.init();
    try {
      const data = await invoke<string[]>('db_get_pending_records');
      return data.map(item => JSON.parse(item) as RaidRecord);
    } catch (error) {
      console.error('Failed to get pending records:', error);
      return [];
    }
  }

  async saveRecords(records: RaidRecord[]): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_records', { records: JSON.stringify(records) });
    } catch (error) {
      console.error('Failed to save records:', error);
      throw error;
    }
  }

  async getRaids(): Promise<Raid[]> {
    await this.init();
    try {
      const data = await invoke<string[]>('db_get_raids');
      return data.map(item => JSON.parse(item));
    } catch (error) {
      console.error('Failed to get raids:', error);
      return [];
    }
  }

  async getRaidVersions(): Promise<string[]> {
    await this.init();
    try {
      return await invoke<string[]>('db_get_raid_versions');
    } catch (error) {
      console.error('Failed to get raid versions:', error);
      return [];
    }
  }

  // ========== 赛季管理 ==========

  async getGameVersions(): Promise<GameVersion[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_game_versions');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get game versions:', error);
      return [];
    }
  }

  async saveGameVersion(version: GameVersion): Promise<number> {
    await this.init();
    try {
      return await invoke<number>('db_save_game_version', { version: JSON.stringify(version) });
    } catch (error) {
      console.error('Failed to save game version:', error);
      throw error;
    }
  }

  async getSeasons(): Promise<Season[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_seasons');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get seasons:', error);
      return [];
    }
  }

  async getSeasonsByVersion(versionId: number): Promise<Season[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_seasons_by_version', { versionId });
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get seasons by version:', error);
      return [];
    }
  }

  async saveSeason(season: Season): Promise<number> {
    await this.init();
    try {
      return await invoke<number>('db_save_season', { season: JSON.stringify(season) });
    } catch (error) {
      console.error('Failed to save season:', error);
      throw error;
    }
  }

  async getSeasonForDate(timestamp: number): Promise<Season | null> {
    await this.init();
    try {
      return await invoke<Season | null>('db_get_season_for_date', { timestamp });
    } catch (error) {
      console.error('Failed to get season for date:', error);
      return null;
    }
  }

  async getCurrentSeason(): Promise<Season | null> {
    await this.init();
    try {
      return await invoke<Season | null>('db_get_current_season');
    } catch (error) {
      console.error('Failed to get current season:', error);
      return null;
    }
  }

  async saveRaids(raids: Raid[]): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_raids', { raids: JSON.stringify(raids) });
    } catch (error) {
      console.error('Failed to save raids:', error);
      throw error;
    }
  }

  // ========== 配置管理 ==========

  async getConfig(): Promise<Config | null> {
    await this.init();
    try {
      const data = await invoke<string | null>('db_get_config');
      if (!data) return null;
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get config:', error);
      return null;
    }
  }

  async saveConfig(config: Config): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_config', { config: JSON.stringify(config) });
    } catch (error) {
      console.error('Failed to save config:', error);
      throw error;
    }
  }

  async addRecord(record: RaidRecord): Promise<void> {
    await this.init();
    try {
      await invoke('db_add_record', { record: JSON.stringify(record) });
    } catch (error) {
      console.error('Failed to add record:', error);
      throw error;
    }
  }

  async deleteRecord(recordId: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_delete_record', { recordId });
    } catch (error) {
      console.error('Failed to delete record:', error);
      throw error;
    }
  }

  async getRecordsByRaid(raidId: string): Promise<RaidRecord[]> {
    await this.init();
    try {
      const data = await invoke<string[]>('db_get_records_by_raid', { raidId });
      return data.map(item => JSON.parse(item));
    } catch (error) {
      console.error('Failed to get records by raid:', error);
      return [];
    }
  }

  async backup(backupPath: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_backup', { backupPath });
    } catch (error) {
      console.error('Failed to backup database:', error);
      throw error;
    }
  }

  async restore(restorePath: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_restore', { restorePath });
    } catch (error) {
      console.error('Failed to restore database:', error);
      throw error;
    }
  }

  async getConfigDebug(): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_get_config_debug');
    } catch (error) {
      console.error('Failed to get config debug:', error);
      return '获取调试信息失败: ' + String(error);
    }
  }

  async resetConfig(defaultConfig: string): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_reset_config', { defaultConfig });
    } catch (error) {
      console.error('Failed to reset config:', error);
      return '重置配置失败: ' + String(error);
    }
  }
  async getCache(key: string): Promise<{ value: unknown, updatedAt: string } | null> {
    await this.init();
    try {
      const result = await invoke<[string, string] | null>('db_get_cache', { key });
      if (!result) return null;
      return {
        value: JSON.parse(result[0]),
        updatedAt: result[1]
      };
    } catch (error) {
      console.error(`Failed to get cache for ${key}:`, error);
      return null;
    }
  }

  async saveCache(key: string, value: unknown): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_cache', { key, value: JSON.stringify(value) });
    } catch (error) {
      console.error(`Failed to save cache for ${key}:`, error);
      throw error;
    }
  }

  async saveEquipments(equipments: Record<string, unknown>[]): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_equipments', { equipments: JSON.stringify(equipments) });
    } catch (error) {
      console.error('Failed to save equipments:', error);
      throw error;
    }
  }

  async getEquipments(): Promise<Record<string, unknown>[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_equipments');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get equipments:', error);
      return [];
    }
  }

  async clearEquipments(): Promise<void> {
    await this.init();
    try {
      await invoke('db_clear_equipments');
    } catch (error) {
      console.error('Failed to clear equipments:', error);
    }
  }

  async addTrialRecord(record: TrialPlaceRecord): Promise<void> {
    await this.init();
    try {
      await invoke('db_add_trial_record', { record: JSON.stringify(record) });
    } catch (error) {
      console.error('Failed to add trial record:', error);
      throw error;
    }
  }

  async getTrialRecords(): Promise<TrialPlaceRecord[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_trial_records');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get trial records:', error);
      return [];
    }
  }

  async deleteTrialRecord(id: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_delete_trial_record', { id });
    } catch (error) {
      console.error('Failed to delete trial record:', error);
      throw error;
    }
  }

  async addBaizhanRecord(record: BaizhanRecord): Promise<void> {
    await this.init();
    try {
      await invoke('db_add_baizhan_record', { record: JSON.stringify(record) });
    } catch (error) {
      console.error('Failed to add baizhan record:', error);
      throw error;
    }
  }

  async getBaizhanRecords(): Promise<BaizhanRecord[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_baizhan_records');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get baizhan records:', error);
      return [];
    }
  }

  async deleteBaizhanRecord(id: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_delete_baizhan_record', { id });
    } catch (error) {
      console.error('Failed to delete baizhan record:', error);
      throw error;
    }
  }

  async updateBaizhanRecord(record: BaizhanRecord): Promise<void> {
    await this.init();
    try {
      await invoke('db_update_baizhan_record', { record: JSON.stringify(record) });
    } catch (error) {
      console.error('Failed to update baizhan record:', error);
      throw error;
    }
  }

  // ========== 副本收藏 ==========

  async getFavoriteRaids(): Promise<string[]> {
    await this.init();
    try {
      return await invoke<string[]>('db_get_favorite_raids');
    } catch (error) {
      console.error('Failed to get favorite raids:', error);
      return [];
    }
  }

  async addFavoriteRaid(raidName: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_add_favorite_raid', { raidName });
    } catch (error) {
      console.error('Failed to add favorite raid:', error);
      throw error;
    }
  }

  async removeFavoriteRaid(raidName: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_remove_favorite_raid', { raidName });
    } catch (error) {
      console.error('Failed to remove favorite raid:', error);
      throw error;
    }
  }

  async isFavoriteRaid(raidName: string): Promise<boolean> {
    await this.init();
    try {
      return await invoke<boolean>('db_is_favorite_raid', { raidName });
    } catch (error) {
      console.error('Failed to check favorite raid:', error);
      return false;
    }
  }

  // ========== 角色可见性配置 (V5+) ==========

  /// 获取所有副本类型
  async getInstanceTypes(): Promise<InstanceType[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_instance_types');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get instance types:', error);
      return [];
    }
  }

  /// 获取所有角色的可见性配置
  async getAllRoleVisibility(): Promise<RoleInstanceVisibility[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_all_role_visibility');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get all role visibility:', error);
      return [];
    }
  }

  /// 保存单个角色的可见性配置
  async saveRoleVisibility(roleId: string, instanceType: string, visible: boolean): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_role_visibility', { roleId, instanceType, visible });
    } catch (error) {
      console.error('Failed to save role visibility:', error);
      throw error;
    }
  }

  // ========== 团队副本角色可见性配置 (V6+) ==========

  /// 获取指定副本的所有角色可见性配置
  async getRaidRoleVisibility(raidKey: string): Promise<{ roleId: string; visible: boolean }[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_raid_role_visibility', { raidKey });
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get raid role visibility:', error);
      return [];
    }
  }

  /// 保存团队副本中单个角色的可见性配置
  async saveRaidRoleVisibility(roleId: string, raidKey: string, visible: boolean): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_raid_role_visibility', { roleId, raidKey, visible });
    } catch (error) {
      console.error('Failed to save raid role visibility:', error);
      throw error;
    }
  }

  // ========== 数据目录管理 ==========

  /// 获取数据目录信息
  async getDataDirInfo(): Promise<{
    currentPath: string;
    location: 'custom' | 'install' | 'user_home';
    isInstallMode: boolean;
    customDirConfigured: boolean;
  }> {
    await this.init();
    try {
      return await invoke('db_get_data_dir_info');
    } catch (error) {
      console.error('Failed to get data dir info:', error);
      throw error;
    }
  }

  /// 设置自定义数据目录
  /// forceOverwrite=true 时，重启迁移会覆盖目标目录已存在的数据库文件
  async setCustomDataDir(path: string, forceOverwrite: boolean = false): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_set_custom_data_dir', { path, forceOverwrite });
    } catch (error) {
      console.error('Failed to set custom data dir:', error);
      throw error;
    }
  }

  /// 恢复默认数据目录
  /// forceOverwrite=true 时，重启迁移会覆盖目标目录已存在的数据库文件
  async resetCustomDataDir(forceOverwrite: boolean = false): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_reset_custom_data_dir', { forceOverwrite });
    } catch (error) {
      console.error('Failed to reset custom data dir:', error);
      throw error;
    }
  }

  /// 检查目标目录是否已存在数据库文件（用于切换目录前的冲突提示）
  async checkTargetDirHasDb(path: string): Promise<boolean> {
    await this.init();
    try {
      return await invoke<boolean>('db_check_target_dir_has_db', { path });
    } catch (error) {
      console.error('Failed to check target dir has db:', error);
      throw error;
    }
  }

  /// 获取默认数据目录路径（不写入配置，用于切换目录前的冲突检查）
  async getDefaultDataDir(): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_get_default_data_dir');
    } catch (error) {
      console.error('Failed to get default data dir:', error);
      throw error;
    }
  }
}

export const db = new DatabaseService();

