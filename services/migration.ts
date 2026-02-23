import { db } from './db';

const STORAGE_KEYS = {
  ACCOUNTS: 'jx3_accounts',
  RECORDS: 'jx3_records',
  RAIDS: 'jx3_raids',
  CONFIG: 'jx3_config',
} as const;

function log(level: 'INFO' | 'WARN' | 'ERROR', message: string, data?: any) {
  const timestamp = new Date().toISOString().split('T')[1].split('.')[0];
  const prefix = `[${timestamp}] [${level}]`;
  if (data !== undefined) {
    console.log(`${prefix} ${message}`, data);
  } else {
    console.log(`${prefix} ${message}`);
  }
}

/// 检查 localStorage 是否有旧数据
export async function checkLocalStorageData(): Promise<{
  hasAccounts: boolean;
  hasRecords: boolean;
  hasRaids: boolean;
  hasConfig: boolean;
  totalItems: number;
  accountsCount: number;
  recordsCount: number;
  raidsCount: number;
}> {
  const accountsRaw = localStorage.getItem(STORAGE_KEYS.ACCOUNTS);
  const recordsRaw = localStorage.getItem(STORAGE_KEYS.RECORDS);
  const raidsRaw = localStorage.getItem(STORAGE_KEYS.RAIDS);
  const configRaw = localStorage.getItem(STORAGE_KEYS.CONFIG);

  let accountsCount = 0, recordsCount = 0, raidsCount = 0;

  if (accountsRaw) {
    try {
      const accounts = JSON.parse(accountsRaw);
      accountsCount = Array.isArray(accounts) ? accounts.length : 0;
    } catch {
      log('ERROR', '解析账号数据失败');
    }
  }

  if (recordsRaw) {
    try {
      const records = JSON.parse(recordsRaw);
      recordsCount = Array.isArray(records) ? records.length : 0;
    } catch {
      log('ERROR', '解析记录数据失败');
    }
  }

  if (raidsRaw) {
    try {
      const raids = JSON.parse(raidsRaw);
      raidsCount = Array.isArray(raids) ? raids.length : 0;
    } catch {
      log('ERROR', '解析副本数据失败');
    }
  }

  return {
    hasAccounts: accountsCount > 0,
    hasRecords: recordsCount > 0,
    hasRaids: raidsCount > 0,
    hasConfig: !!configRaw,
    totalItems: accountsCount + recordsCount + raidsCount + (configRaw ? 1 : 0),
    accountsCount,
    recordsCount,
    raidsCount
  };
}

/// 清除 localStorage 中的旧数据
function clearLocalStorageData(): void {
  log('INFO', '清除 localStorage 中的旧数据...');
  try {
    localStorage.removeItem(STORAGE_KEYS.ACCOUNTS);
    localStorage.removeItem(STORAGE_KEYS.RECORDS);
    localStorage.removeItem(STORAGE_KEYS.RAIDS);
    localStorage.removeItem(STORAGE_KEYS.CONFIG);
    log('INFO', '  ✓ localStorage 数据已清除');
  } catch (error: any) {
    log('ERROR', `清除 localStorage 失败: ${error.message}`);
  }
}

/// 执行 localStorage 数据迁移
/// 返回迁移结果
export async function migrateLocalStorageData(): Promise<{
  success: boolean;
  message: string;
  migrated: {
    accounts: number;
    records: number;
    raids: number;
    config: boolean;
  };
  details: string[];
}> {
  log('INFO', '=== 开始 localStorage 数据迁移 ===');

  const result = { accounts: 0, records: 0, raids: 0, config: false };
  const details: string[] = [];

  try {
    // 检查是否已完成迁移
    const alreadyMigrated = await db.isLocalStorageMigrated();
    if (alreadyMigrated) {
      log('INFO', 'localStorage 迁移已完成，跳过');
      return { success: true, message: '迁移已完成', migrated: result, details };
    }

    // 迁移账号数据
    const accountsRaw = localStorage.getItem(STORAGE_KEYS.ACCOUNTS);
    if (accountsRaw) {
      try {
        const accounts = JSON.parse(accountsRaw);
        if (Array.isArray(accounts) && accounts.length > 0) {
          await db.saveAccounts(accounts);
          result.accounts = accounts.length;
          details.push(`账号: 迁移 ${accounts.length} 个账号`);
          log('INFO', `  ✓ 迁移 ${accounts.length} 个账号`);
        } else {
          details.push('账号: 无数据');
        }
      } catch (e: any) {
        details.push(`账号: 迁移失败 - ${e.message}`);
        log('ERROR', `  账号迁移失败: ${e.message}`);
      }
    } else {
      details.push('账号: localStorage 中无数据');
    }

    // 迁移记录数据
    const recordsRaw = localStorage.getItem(STORAGE_KEYS.RECORDS);
    if (recordsRaw) {
      try {
        const records = JSON.parse(recordsRaw);
        if (Array.isArray(records) && records.length > 0) {
          await db.saveRecords(records);
          result.records = records.length;
          details.push(`记录: 迁移 ${records.length} 条记录`);
          log('INFO', `  ✓ 迁移 ${records.length} 条记录`);
        } else {
          details.push('记录: 无数据');
        }
      } catch (e: any) {
        details.push(`记录: 迁移失败 - ${e.message}`);
        log('ERROR', `  记录迁移失败: ${e.message}`);
      }
    } else {
      details.push('记录: localStorage 中无数据');
    }

    // 迁移副本数据（注意：V2+ 版本 raids 使用静态数据，不迁移）
    const raidsRaw = localStorage.getItem(STORAGE_KEYS.RAIDS);
    if (raidsRaw) {
      try {
        const raids = JSON.parse(raidsRaw);
        if (Array.isArray(raids) && raids.length > 0) {
          // V2+ 版本使用静态副本数据，不需要迁移用户的自定义副本
          // 但保留数据用于参考
          details.push(`副本: V2+ 使用静态数据，忽略 ${raids.length} 个用户自定义副本`);
          log('INFO', `  ⊘ 忽略 ${raids.length} 个自定义副本（使用静态数据）`);
          result.raids = 0; // 不实际迁移
        } else {
          details.push('副本: 无数据');
        }
      } catch (e: any) {
        details.push(`副本: 解析失败 - ${e.message}`);
        log('ERROR', `  副本解析失败: ${e.message}`);
      }
    } else {
      details.push('副本: localStorage 中无数据');
    }

    // 迁移配置数据
    const configRaw = localStorage.getItem(STORAGE_KEYS.CONFIG);
    if (configRaw) {
      try {
        const config = JSON.parse(configRaw);
        await db.saveConfig(config);
        result.config = true;
        details.push('配置: 迁移配置数据');
        log('INFO', `  ✓ 迁移配置`);
      } catch (e: any) {
        details.push(`配置: 迁移失败 - ${e.message}`);
        log('ERROR', `  配置迁移失败: ${e.message}`);
      }
    } else {
      details.push('配置: localStorage 中无数据');
    }

    // 标记迁移完成
    await db.setLocalStorageMigrated();
    details.push('迁移状态: 已标记完成');

    // 清除 localStorage 数据
    clearLocalStorageData();
    details.push('清理: 已清除 localStorage 数据');

    const hasMigrated = result.accounts > 0 || result.records > 0 || result.config;
    const summary = hasMigrated
      ? `迁移完成: ${result.accounts} 账号, ${result.records} 记录, ${result.config ? '1 配置' : '0 配置'}`
      : '无需迁移';

    log('INFO', `=== localStorage 迁移完成 ===`);

    return {
      success: true,
      message: summary,
      migrated: result,
      details
    };
  } catch (error: any) {
    const errorMsg = `迁移过程出错: ${error.message}`;
    details.push(`错误: ${error.message}`);
    log('ERROR', errorMsg);
    return {
      success: false,
      message: errorMsg,
      migrated: result,
      details
    };
  }
}

/// 获取数据库版本信息（用于调试）
export async function getVersionInfo(): Promise<any> {
  try {
    return await db.getVersionInfo();
  } catch (error: any) {
    log('ERROR', `获取版本信息失败: ${error.message}`);
    return null;
  }
}

// ========== 以下是旧的诊断函数，保留用于调试 ==========

export async function diagnoseMigration(): Promise<{
  localStorage: {
    hasData: boolean;
    accountsCount: number;
    recordsCount: number;
    raidsCount: number;
    hasConfig: boolean;
    dataSize: {
      accounts: number;
      records: number;
      raids: number;
      config: number;
    };
  };
  database: {
    connectionOk: boolean;
    accountsCount: number;
    recordsCount: number;
    raidsCount: number;
    hasConfig: boolean;
  };
  errors: string[];
}> {
  const result = {
    localStorage: {
      hasData: false,
      accountsCount: 0,
      recordsCount: 0,
      raidsCount: 0,
      hasConfig: false,
      dataSize: { accounts: 0, records: 0, raids: 0, config: 0 }
    },
    database: {
      connectionOk: false,
      accountsCount: 0,
      recordsCount: 0,
      raidsCount: 0,
      hasConfig: false
    },
    errors: [] as string[]
  };

  log('INFO', '=== 开始迁移诊断 ===');

  try {
    log('INFO', '[诊断 1/2] 检查 localStorage 数据...');

    const accountsRaw = localStorage.getItem(STORAGE_KEYS.ACCOUNTS);
    const recordsRaw = localStorage.getItem(STORAGE_KEYS.RECORDS);
    const raidsRaw = localStorage.getItem(STORAGE_KEYS.RAIDS);
    const configRaw = localStorage.getItem(STORAGE_KEYS.CONFIG);

    result.localStorage.dataSize.accounts = accountsRaw?.length || 0;
    result.localStorage.dataSize.records = recordsRaw?.length || 0;
    result.localStorage.dataSize.raids = raidsRaw?.length || 0;
    result.localStorage.dataSize.config = configRaw?.length || 0;

    if (accountsRaw) {
      try {
        const accounts = JSON.parse(accountsRaw);
        result.localStorage.accountsCount = Array.isArray(accounts) ? accounts.length : 0;
        log('INFO', `  账号: ${result.localStorage.accountsCount} 条`);
      } catch (e: any) {
        result.errors.push('解析账号数据失败: ' + e.message);
      }
    }

    if (recordsRaw) {
      try {
        const records = JSON.parse(recordsRaw);
        result.localStorage.recordsCount = Array.isArray(records) ? records.length : 0;
        log('INFO', `  记录: ${result.localStorage.recordsCount} 条`);
      } catch (e: any) {
        result.errors.push('解析记录数据失败: ' + e.message);
      }
    }

    if (raidsRaw) {
      try {
        const raids = JSON.parse(raidsRaw);
        result.localStorage.raidsCount = Array.isArray(raids) ? raids.length : 0;
        log('INFO', `  副本: ${result.localStorage.raidsCount} 条`);
      } catch (e: any) {
        result.errors.push('解析副本数据失败: ' + e.message);
      }
    }

    if (configRaw) {
      result.localStorage.hasConfig = true;
      log('INFO', `  配置: 有`);
    }

    result.localStorage.hasData =
      result.localStorage.accountsCount > 0 ||
      result.localStorage.recordsCount > 0 ||
      result.localStorage.raidsCount > 0 ||
      result.localStorage.hasConfig;

  } catch (error: any) {
    result.errors.push('检查 localStorage 失败: ' + error.message);
  }

  try {
    log('INFO', '[诊断 2/2] 检查数据库...');

    await db.init();
    result.database.connectionOk = true;

    const accounts = await db.getAccounts();
    result.database.accountsCount = accounts.length;

    const records = await db.getRecords();
    result.database.recordsCount = records.length;

    const raids = await db.getRaids();
    result.database.raidsCount = raids.length;

    const config = await db.getConfig();
    result.database.hasConfig = !!config;

    log('INFO', `  数据库: 账号 ${accounts.length}, 记录 ${records.length}, 副本 ${raids.length}`);

  } catch (error: any) {
    result.errors.push('检查数据库失败: ' + error.message);
  }

  log('INFO', '=== 诊断完成 ===');
  return result;
}

export async function debugConfig(): Promise<string> {
  log('INFO', '=== 调试配置数据 ===');
  try {
    return await db.getConfigDebug();
  } catch (error: any) {
    log('ERROR', `调试失败: ${error.message}`);
    return '调试失败: ' + error.message;
  }
}

export async function resetConfig(defaultConfig: string): Promise<string> {
  log('INFO', '=== 重置配置数据 ===');
  try {
    return await db.resetConfig(defaultConfig);
  } catch (error: any) {
    log('ERROR', `重置失败: ${error.message}`);
    return '重置失败: ' + error.message;
  }
}

// ========== 以下函数已废弃，保留用于兼容 ==========

export async function forceMigrate(): Promise<{
  success: boolean;
  message: string;
  migrated: {
    accounts: number;
    records: number;
    raids: number;
    config: boolean;
  };
  details: any;
}> {
  const result = await migrateLocalStorageData();
  return {
    ...result,
    details: {
      dbBefore: { accounts: 0, records: 0, raids: 0 },
      dbAfter: { accounts: result.migrated.accounts, records: result.migrated.records, raids: result.migrated.raids },
      skipped: { accounts: 0, records: 0, raids: 0 }
    }
  };
}

export async function manualMigrate(): Promise<{
  success: boolean;
  message: string;
}> {
  const result = await migrateLocalStorageData();
  return { success: result.success, message: result.message };
}

// ========== 以下函数已废弃，仅用于兼容 MigrationStatus 组件 ==========

export async function deduplicateAccounts(): Promise<{
  success: boolean;
  message: string;
  removed: number;
  remaining: number;
}> {
  log('WARN', 'deduplicateAccounts 已废弃');
  return { success: false, message: '此功能已废弃', removed: 0, remaining: 0 };
}

export async function analyzeDuplicates(): Promise<string> {
  log('WARN', 'analyzeDuplicates 已废弃');
  return '此功能已废弃';
}

export async function deduplicateRaids(): Promise<string> {
  log('WARN', 'deduplicateRaids 已废弃');
  return '此功能已废弃';
}

export async function addUniqueConstraint(): Promise<string> {
  log('WARN', 'addUniqueConstraint 已废弃');
  return '此功能已废弃';
}
