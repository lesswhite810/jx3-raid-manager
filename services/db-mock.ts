/**
 * Mock Database Service for Web Development
 * 提供模拟数据，支持在Web模式下测试UI
 */

interface MockRecord {
  id: string;
  raidId: string;
  date: string;
  items: any[];
}

interface MockAccount {
  id: string;
  name: string;
  server: string;
  roles: MockRole[];
}

interface MockRole {
  id: string;
  name: string;
  class: string;
}

class MockDatabaseService {
  private initialized = false;
  private accounts: MockAccount[] = [];
  private records: MockRecord[] = [];
  private raids: any[] = [];
  private config: any = null;

  async init(): Promise<void> {
    if (this.initialized) return;
    this.loadMockData();
    this.initialized = true;
    console.log('🎭 Mock Database initialized');
  }

  private loadMockData(): void {
    // 模拟账号数据
    this.accounts = [
      {
        id: 'acc1',
        name: '测试账号',
        server: '唯我独尊',
        roles: [
          { id: 'role1', name: '测试花间', class: '花间游' },
          { id: 'role2', name: '测试万花', class: '离经易脉' }
        ]
      }
    ];

    // 模拟副本数据
    this.raids = [
      {
        id: 'raid1',
        name: '25人普通·绝境战场',
        difficulty: '25人普通'
      },
      {
        id: 'raid2',
        name: '10人英雄·千雷惊狱',
        difficulty: '10人英雄'
      }
    ];

    // 模拟记录数据
    this.records = [
      {
        id: 'rec1',
        raidId: 'raid1',
        date: new Date().toISOString(),
        items: [
          { name: '玄晶', type: 'material' },
          { name: '宠物·雪团子', type: 'pet' }
        ]
      }
    ];

    // 模拟配置
    this.config = {
      theme: 'light',
      language: 'zh-CN',
      fontSize: 14
    };
  }

  // 模拟账号接口
  async getAccounts(): Promise<any[]> {
    await this.init();
    return this.accounts;
  }

  async saveAccounts(accounts: any[]): Promise<void> {
    await this.init();
    this.accounts = accounts;
    console.log('🎭 Saved accounts (mock):', accounts);
  }

  async getAccountsStructured(): Promise<any[]> {
    await this.init();
    return this.accounts;
  }

  async getRolesByAccount(accountId: string): Promise<any[]> {
    await this.init();
    const account = this.accounts.find(a => a.id === accountId);
    return account?.roles || [];
  }

  async getAllRoles(): Promise<any[]> {
    await this.init();
    return this.accounts.flatMap(acc => acc.roles);
  }

  async saveAccountStructured(account: any): Promise<void> {
    await this.init();
    const index = this.accounts.findIndex(a => a.id === account.id);
    if (index >= 0) {
      this.accounts[index] = account;
    } else {
      this.accounts.push(account);
    }
  }

  async saveRoleStructured(role: any): Promise<void> {
    await this.init();
    // 简化处理，实际应用中需要更复杂的逻辑
    console.log('🎭 Saved role (mock):', role);
  }

  async deleteAccountStructured(accountId: string): Promise<void> {
    await this.init();
    this.accounts = this.accounts.filter(a => a.id !== accountId);
  }

  async deleteRoleStructured(roleId: string): Promise<void> {
    await this.init();
    // 简化处理
    console.log('🎭 Deleted role (mock):', roleId);
  }

  // 模拟记录接口
  async getRecords(): Promise<any[]> {
    await this.init();
    return this.records;
  }

  async saveRecords(records: any[]): Promise<void> {
    await this.init();
    this.records = records;
  }

  async addRecord(record: any): Promise<void> {
    await this.init();
    this.records.push(record);
  }

  async deleteRecord(recordId: string): Promise<void> {
    await this.init();
    this.records = this.records.filter(r => r.id !== recordId);
  }

  async getRecordsByRaid(raidId: string): Promise<any[]> {
    await this.init();
    return this.records.filter(r => r.raidId === raidId);
  }

  // 模拟副本接口
  async getRaids(): Promise<any[]> {
    await this.init();
    return this.raids;
  }

  async saveRaids(raids: any[]): Promise<void> {
    await this.init();
    this.raids = raids;
  }

  // 模拟配置接口
  async getConfig(): Promise<any | null> {
    await this.init();
    return this.config;
  }

  async saveConfig(config: any): Promise<void> {
    await this.init();
    this.config = config;
    console.log('🎭 Saved config (mock):', config);
  }

  // 模拟其他接口
  async getSchemaVersion(): Promise<number> {
    return 1;
  }

  async checkMigrationNeeded(): Promise<boolean> {
    return false;
  }

  async getMigrationStatus(): Promise<string | null> {
    return null;
  }

  async checkMigrationCompleted(): Promise<boolean> {
    return true;
  }

  async setMigrationStatus(status: string, errorMessage?: string): Promise<void> {
    console.log('🎭 Migration status (mock):', status);
  }

  async backup(backupPath: string): Promise<void> {
    console.log('🎭 Backup (mock):', backupPath);
  }

  async restore(restorePath: string): Promise<void> {
    console.log('🎭 Restore (mock):', restorePath);
  }

  async analyzeDuplicates(): Promise<string> {
    return '模拟：未发现重复数据';
  }

  async deduplicateAccounts(): Promise<string> {
    return '模拟：账号去重完成';
  }

  async deduplicateRaids(): Promise<string> {
    return '模拟：副本去重完成';
  }

  async addUniqueConstraintRaids(): Promise<string> {
    return '模拟：添加约束完成';
  }

  async getConfigDebug(): Promise<string> {
    return JSON.stringify(this.config, null, 2);
  }

  async resetConfig(defaultConfig: string): Promise<string> {
    this.config = JSON.parse(defaultConfig);
    return '模拟：配置已重置';
  }

  async getCache(key: string): Promise<{ value: any, updatedAt: string } | null> {
    return null;
  }

  async saveCache(key: string, value: any): Promise<void> {
    console.log('🎭 Cache saved (mock):', key);
  }

  async saveEquipments(equipments: any[]): Promise<void> {
    console.log('🎭 Saved equipments (mock):', equipments.length);
  }

  async getEquipments(): Promise<any[]> {
    return [];
  }

  async addTrialRecord(record: any): Promise<void> {
    console.log('🎭 Added trial record (mock):', record);
  }

  async getTrialRecords(): Promise<any[]> {
    return [];
  }

  async deleteTrialRecord(id: string): Promise<void> {
    console.log('🎭 Deleted trial record (mock):', id);
  }
}

export const dbMock = new MockDatabaseService();
