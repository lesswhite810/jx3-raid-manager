import { invoke } from '@tauri-apps/api/tauri';

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

  // Version management
  async getSchemaVersion(): Promise<number> {
    await this.init();
    try {
      return await invoke<number>('db_get_schema_version');
    } catch (error) {
      console.error('Failed to get schema version:', error);
      return 0;
    }
  }

  async checkMigrationNeeded(): Promise<boolean> {
    await this.init();
    try {
      return await invoke<boolean>('db_check_migration_needed');
    } catch (error) {
      console.error('Failed to check migration needed:', error);
      return false;
    }
  }

  async getAccounts(): Promise<any[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_accounts_with_roles');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get accounts:', error);
      return [];
    }
  }

  async saveAccounts(accounts: any[]): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_accounts', { accounts: JSON.stringify(accounts) });
    } catch (error) {
      console.error('Failed to save accounts:', error);
      throw error;
    }
  }

  // Structured accounts API (V1+)
  async getAccountsStructured(): Promise<any[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_accounts_structured');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get structured accounts:', error);
      return [];
    }
  }

  async getRolesByAccount(accountId: string): Promise<any[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_roles_by_account', { accountId });
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get roles by account:', error);
      return [];
    }
  }

  async getAllRoles(): Promise<any[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_all_roles');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get all roles:', error);
      return [];
    }
  }

  async saveAccountStructured(account: any): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_account_structured', { accountJson: JSON.stringify(account) });
    } catch (error) {
      console.error('Failed to save structured account:', error);
      throw error;
    }
  }

  async saveRoleStructured(role: any): Promise<void> {
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

  async getRecords(): Promise<any[]> {
    await this.init();
    try {
      const data = await invoke<string[]>('db_get_records');
      return data.map(item => JSON.parse(item));
    } catch (error) {
      console.error('Failed to get records:', error);
      return [];
    }
  }

  async saveRecords(records: any[]): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_records', { records: JSON.stringify(records) });
    } catch (error) {
      console.error('Failed to save records:', error);
      throw error;
    }
  }

  async getRaids(): Promise<any[]> {
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

  async saveRaids(raids: any[]): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_raids', { raids: JSON.stringify(raids) });
    } catch (error) {
      console.error('Failed to save raids:', error);
      throw error;
    }
  }

  async getMigrationStatus(): Promise<string | null> {
    await this.init();
    try {
      return await invoke<string | null>('db_get_migration_status');
    } catch (error) {
      console.error('Failed to get migration status:', error);
      return null;
    }
  }

  async checkMigrationCompleted(): Promise<boolean> {
    await this.init();
    try {
      return await invoke<boolean>('db_check_migration_completed');
    } catch (error) {
      console.error('Failed to check migration status:', error);
      return false;
    }
  }

  async setMigrationStatus(status: string, errorMessage?: string): Promise<void> {
    await this.init();
    try {
      await invoke('db_set_migration_status', { status, errorMessage: errorMessage || null });
    } catch (error) {
      console.error('Failed to set migration status:', error);
      throw error;
    }
  }

  async getConfig(): Promise<any | null> {
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

  async saveConfig(config: any): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_config', { config: JSON.stringify(config) });
    } catch (error) {
      console.error('Failed to save config:', error);
      throw error;
    }
  }

  async addRecord(record: any): Promise<void> {
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

  async getRecordsByRaid(raidId: string): Promise<any[]> {
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

  async analyzeDuplicates(): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_analyze_duplicates');
    } catch (error) {
      console.error('Failed to analyze duplicates:', error);
      return '分析失败: ' + String(error);
    }
  }

  async deduplicateAccounts(): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_deduplicate_accounts');
    } catch (error) {
      console.error('Failed to deduplicate accounts:', error);
      return '去重失败: ' + String(error);
    }
  }

  async deduplicateRaids(): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_deduplicate_raids');
    } catch (error) {
      console.error('Failed to deduplicate raids:', error);
      return '去重失败: ' + String(error);
    }
  }

  async addUniqueConstraintRaids(): Promise<string> {
    await this.init();
    try {
      return await invoke<string>('db_add_unique_constraint_raids');
    } catch (error) {
      console.error('Failed to add unique constraint:', error);
      return '添加约束失败: ' + String(error);
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
  async getCache(key: string): Promise<{ value: any, updatedAt: string } | null> {
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

  async saveCache(key: string, value: any): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_cache', { key, value: JSON.stringify(value) });
    } catch (error) {
      console.error(`Failed to save cache for ${key}:`, error);
      throw error;
    }
  }

  async saveEquipments(equipments: any[]): Promise<void> {
    await this.init();
    try {
      await invoke('db_save_equipments', { equipments: JSON.stringify(equipments) });
    } catch (error) {
      console.error('Failed to save equipments:', error);
      throw error;
    }
  }

  async getEquipments(): Promise<any[]> {
    await this.init();
    try {
      const data = await invoke<string>('db_get_equipments');
      return JSON.parse(data);
    } catch (error) {
      console.error('Failed to get equipments:', error);
      return [];
    }
  }

  async addTrialRecord(record: any): Promise<void> {
    await this.init();
    try {
      await invoke('db_add_trial_record', { record: JSON.stringify(record) });
    } catch (error) {
      console.error('Failed to add trial record:', error);
      throw error;
    }
  }

  async getTrialRecords(): Promise<any[]> {
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

  async addBaizhanRecord(record: any): Promise<void> {
    await this.init();
    try {
      await invoke('db_add_baizhan_record', { record: JSON.stringify(record) });
    } catch (error) {
      console.error('Failed to add baizhan record:', error);
      throw error;
    }
  }

  async getBaizhanRecords(): Promise<any[]> {
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

  async updateBaizhanRecord(record: any): Promise<void> {
    await this.init();
    try {
      // 先删除旧记录，再添加更新后的记录（复用现有 Tauri 命令）
      await invoke('db_delete_baizhan_record', { id: record.id });
      await invoke('db_add_baizhan_record', { record: JSON.stringify(record) });
    } catch (error) {
      console.error('Failed to update baizhan record:', error);
      throw error;
    }
  }
}

export const db = new DatabaseService();

