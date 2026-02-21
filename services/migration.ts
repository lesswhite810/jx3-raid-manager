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
        log('INFO', `  账号: ${result.localStorage.accountsCount} 条 (${result.localStorage.dataSize.accounts} bytes)`);
      } catch (e: any) {
        result.errors.push('解析账号数据失败: ' + e.message);
        log('ERROR', `  账号数据解析失败: ${e.message}`);
      }
    } else {
      log('INFO', `  账号: 无数据`);
    }

    if (recordsRaw) {
      try {
        const records = JSON.parse(recordsRaw);
        result.localStorage.recordsCount = Array.isArray(records) ? records.length : 0;
        log('INFO', `  记录: ${result.localStorage.recordsCount} 条 (${result.localStorage.dataSize.records} bytes)`);
      } catch (e: any) {
        result.errors.push('解析记录数据失败: ' + e.message);
        log('ERROR', `  记录数据解析失败: ${e.message}`);
      }
    } else {
      log('INFO', `  记录: 无数据`);
    }

    if (raidsRaw) {
      try {
        const raids = JSON.parse(raidsRaw);
        result.localStorage.raidsCount = Array.isArray(raids) ? raids.length : 0;
        log('INFO', `  副本: ${result.localStorage.raidsCount} 条 (${result.localStorage.dataSize.raids} bytes)`);
      } catch (e: any) {
        result.errors.push('解析副本数据失败: ' + e.message);
        log('ERROR', `  副本数据解析失败: ${e.message}`);
      }
    } else {
      log('INFO', `  副本: 无数据`);
    }

    if (configRaw) {
      result.localStorage.hasConfig = true;
      log('INFO', `  配置: 有 (${result.localStorage.dataSize.config} bytes)`);
    } else {
      log('INFO', `  配置: 无`);
    }

    result.localStorage.hasData =
      result.localStorage.accountsCount > 0 ||
      result.localStorage.recordsCount > 0 ||
      result.localStorage.raidsCount > 0 ||
      result.localStorage.hasConfig;

  } catch (error: any) {
    result.errors.push('检查 localStorage 失败: ' + error.message);
    log('ERROR', `  检查 localStorage 失败: ${error.message}`);
  }

  try {
    log('INFO', '[诊断 2/2] 检查 SQLite 数据库...');

    await db.init();
    result.database.connectionOk = true;
    log('INFO', '  数据库连接成功');

    const accounts = await db.getAccounts();
    result.database.accountsCount = accounts.length;
    log('INFO', `  数据库账号: ${accounts.length} 条`);
    if (accounts.length > 0 && typeof accounts[0] === 'string') {
      log('INFO', `    账号数据预览: ${accounts[0].substring(0, 100)}...`);
    }

    const records = await db.getRecords();
    result.database.recordsCount = records.length;
    log('INFO', `  数据库记录: ${records.length} 条`);
    if (records.length > 0) {
      const recordStr = typeof records[0] === 'string' ? records[0] : JSON.stringify(records[0]);
      log('INFO', `    记录数据预览: ${recordStr.substring(0, 100)}...`);
    }

    const raids = await db.getRaids();
    result.database.raidsCount = raids.length;
    log('INFO', `  数据库副本: ${raids.length} 条`);
    if (raids.length > 0) {
      const raidStr = typeof raids[0] === 'string' ? raids[0] : JSON.stringify(raids[0]);
      log('INFO', `    副本数据预览: ${raidStr.substring(0, 100)}...`);
    }

    const config = await db.getConfig();
    result.database.hasConfig = !!config;
    log('INFO', `  数据库配置: ${config ? '有' : '无'}`);
    if (config) {
      const configStr = typeof config === 'string' ? config : JSON.stringify(config);
      log('INFO', `    配置数据: ${configStr.substring(0, 100)}...`);
    }

  } catch (error: any) {
    result.errors.push('检查数据库失败: ' + error.message);
    log('ERROR', `  数据库检查失败: ${error.message}`);
    log('ERROR', `  错误堆栈: ${error.stack || '无'}`);
  }

  log('INFO', '=== 诊断完成 ===');
  return result;
}

export async function deduplicateAccounts(): Promise<{
  success: boolean;
  message: string;
  removed: number;
  remaining: number;
}> {
  log('INFO', '=== 开始账号去重 (Rust) ===');

  try {
    const result = await db.deduplicateAccounts();
    log('INFO', `  ${result}`);

    // 解析结果
    const match = result.match(/处理前: (\d+) 条.*删除: (\d+) 条.*保留: (\d+) 条/);
    if (match) {
      return {
        success: true,
        message: result,
        removed: parseInt(match[2]),
        remaining: parseInt(match[3])
      };
    }

    return { success: true, message: result, removed: 0, remaining: 0 };
  } catch (error: any) {
    log('ERROR', `去重失败: ${error.message}`);
    return { success: false, message: error.message, removed: 0, remaining: 0 };
  }
}

export async function analyzeDuplicates(): Promise<string> {
  log('INFO', '=== 分析重复数据 ===');
  try {
    return await db.analyzeDuplicates();
  } catch (error: any) {
    log('ERROR', `分析失败: ${error.message}`);
    return '分析失败: ' + error.message;
  }
}

export async function deduplicateRaids(): Promise<string> {
  log('INFO', '=== 开始副本去重 (Rust) ===');
  try {
    return await db.deduplicateRaids();
  } catch (error: any) {
    log('ERROR', `去重失败: ${error.message}`);
    return '去重失败: ' + error.message;
  }
}

export async function addUniqueConstraint(): Promise<string> {
  log('INFO', '=== 添加唯一性约束 ===');
  try {
    return await db.addUniqueConstraintRaids();
  } catch (error: any) {
    log('ERROR', `添加约束失败: ${error.message}`);
    return '添加约束失败: ' + error.message;
  }
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

export async function forceMigrate(): Promise<{
  success: boolean;
  message: string;
  migrated: {
    accounts: number;
    records: number;
    raids: number;
    config: boolean;
  };
  details: {
    dbBefore: { accounts: number; records: number; raids: number };
    dbAfter: { accounts: number; records: number; raids: number };
    skipped: { accounts: number; records: number; raids: number };
  };
}> {
  log('INFO', '=== 开始智能迁移 ===');

  try {
    log('INFO', '[1/6] 初始化数据库...');
    await db.init();
    log('INFO', '  数据库初始化成功');

    log('INFO', '[2/6] 检查迁移状态...');
    const migrationCompleted = await db.checkMigrationCompleted();
    if (migrationCompleted) {
      log('INFO', '  迁移已完成，跳过迁移步骤');
      return {
        success: true,
        message: '迁移已完成',
        migrated: { accounts: 0, records: 0, raids: 0, config: false },
        details: {
          dbBefore: { accounts: 0, records: 0, raids: 0 },
          dbAfter: { accounts: 0, records: 0, raids: 0 },
          skipped: { accounts: 0, records: 0, raids: 0 }
        }
      };
    }
    log('INFO', '  迁移未完成，继续执行迁移');

    log('INFO', '[3/6] 获取数据库当前状态...');
    const dbAccountsBefore = await db.getAccounts();
    const dbRecordsBefore = await db.getRecords();
    const dbRaidsBefore = await db.getRaids();
    log('INFO', `  当前数据库: 账号 ${dbAccountsBefore.length}, 记录 ${dbRecordsBefore.length}, 副本 ${dbRaidsBefore.length}`);

    const dbAccountIds = new Set(dbAccountsBefore.map((a: any) => (typeof a === 'string' ? JSON.parse(a).id : a.id)));
    const dbRecordIds = new Set(dbRecordsBefore.map((r: any) => (typeof r === 'string' ? JSON.parse(r).id : r.id)));
    const dbRaidIds = new Set(dbRaidsBefore.map((r: any) => (typeof r === 'string' ? JSON.parse(r).id : r.id)));

    log('INFO', '[4/6] 迁移数据（去重）...');
    const result = { accounts: 0, records: 0, raids: 0, config: false };
    const skipped = { accounts: 0, records: 0, raids: 0 };

    try {
      const accountsRaw = localStorage.getItem(STORAGE_KEYS.ACCOUNTS);
      if (accountsRaw) {
        const accounts = JSON.parse(accountsRaw);
        if (Array.isArray(accounts) && accounts.length > 0) {
          const newAccounts = accounts.filter((a: any) => !dbAccountIds.has(a.id));
          if (newAccounts.length > 0) {
            log('INFO', `  发现 ${newAccounts.length} 个新账号（跳过 ${accounts.length - newAccounts.length} 个重复）`);
            await db.saveAccounts(newAccounts);
            result.accounts = newAccounts.length;
            log('INFO', `  ✓ 迁移 ${newAccounts.length} 个新账号`);
          } else {
            skipped.accounts = accounts.length;
            log('INFO', `  全部 ${accounts.length} 个账号已存在，跳过`);
          }
        }
      }
    } catch (error: any) {
      log('ERROR', `  账号迁移失败: ${error.message}`);
    }

    try {
      const recordsRaw = localStorage.getItem(STORAGE_KEYS.RECORDS);
      if (recordsRaw) {
        const records = JSON.parse(recordsRaw);
        if (Array.isArray(records) && records.length > 0) {
          const newRecords = records.filter((r: any) => !dbRecordIds.has(r.id));
          if (newRecords.length > 0) {
            log('INFO', `  发现 ${newRecords.length} 条新记录（跳过 ${records.length - newRecords.length} 条重复）`);
            await db.saveRecords(newRecords);
            result.records = newRecords.length;
            log('INFO', `  ✓ 迁移 ${newRecords.length} 条新记录`);
          } else {
            skipped.records = records.length;
            log('INFO', `  全部 ${records.length} 条记录已存在，跳过`);
          }
        }
      }
    } catch (error: any) {
      log('ERROR', `  记录迁移失败: ${error.message}`);
    }

    try {
      const raidsRaw = localStorage.getItem(STORAGE_KEYS.RAIDS);
      if (raidsRaw) {
        const raids = JSON.parse(raidsRaw);
        if (Array.isArray(raids) && raids.length > 0) {
          const newRaids = raids.filter((r: any) => !dbRaidIds.has(r.id));
          if (newRaids.length > 0) {
            log('INFO', `  发现 ${newRaids.length} 个新副本（跳过 ${raids.length - newRaids.length} 个重复）`);
            await db.saveRaids(newRaids);
            result.raids = newRaids.length;
            log('INFO', `  ✓ 迁移 ${newRaids.length} 个新副本`);
          } else {
            skipped.raids = raids.length;
            log('INFO', `  全部 ${raids.length} 个副本已存在，跳过`);
          }
        }
      }
    } catch (error: any) {
      log('ERROR', `  副本迁移失败: ${error.message}`);
    }

    try {
      const configRaw = localStorage.getItem(STORAGE_KEYS.CONFIG);
      if (configRaw) {
        await db.saveConfig(JSON.parse(configRaw));
        result.config = true;
        log('INFO', `  ✓ 配置已更新`);
      }
    } catch (error: any) {
      log('ERROR', `  配置迁移失败: ${error.message}`);
    }

    log('INFO', '[5/6] 验证迁移结果...');
    const dbAccountsAfter = await db.getAccounts();
    const dbRecordsAfter = await db.getRecords();
    const dbRaidsAfter = await db.getRaids();
    const dbConfig = await db.getConfig();

    const accountsAdded = dbAccountsAfter.length - dbAccountsBefore.length;
    const recordsAdded = dbRecordsAfter.length - dbRecordsBefore.length;
    const raidsAdded = dbRaidsAfter.length - dbRaidsBefore.length;

    const accountsMatch = accountsAdded === result.accounts;
    const recordsMatch = recordsAdded === result.records;
    const raidsMatch = raidsAdded === result.raids;
    const configMatch = result.config ? !!dbConfig : true;

    log('INFO', `  账号: ${accountsMatch ? '✓' : '✗'} (新增:${dbAccountsAfter.length - dbAccountsBefore.length}, 期望:${result.accounts})`);
    log('INFO', `  记录: ${recordsMatch ? '✓' : '✗'} (新增:${dbRecordsAfter.length - dbRecordsBefore.length}, 期望:${result.records})`);
    log('INFO', `  副本: ${raidsMatch ? '✓' : '✗'} (新增:${dbRaidsAfter.length - dbRaidsBefore.length}, 期望:${result.raids})`);
    log('INFO', `  配置: ${configMatch ? '✓' : '✗'}`);

    const allMatch = accountsMatch && recordsMatch && raidsMatch && configMatch;
    const hasNewData = result.accounts > 0 || result.records > 0 || result.raids > 0 || result.config;

    log('INFO', '[6/6] 完成迁移状态记录...');

    if (allMatch && hasNewData) {
      log('INFO', '  ✓ 迁移成功！记录迁移状态');
      await db.setMigrationStatus('completed');
    } else if (!hasNewData) {
      log('INFO', '  ✓ 数据已是最新的，记录迁移状态');
      await db.setMigrationStatus('completed');
    } else {
      log('WARN', '  ⚠ 部分数据可能未正确迁移');
      await db.setMigrationStatus('partial', '部分数据验证不匹配');
    }

    return {
      success: allMatch || !hasNewData,
      message: hasNewData
        ? (allMatch ? '迁移成功' : '部分数据未完全匹配')
        : '数据已是最新的',
      migrated: result,
      details: {
        dbBefore: {
          accounts: dbAccountsBefore.length,
          records: dbRecordsBefore.length,
          raids: dbRaidsBefore.length
        },
        dbAfter: {
          accounts: dbAccountsAfter.length,
          records: dbRecordsAfter.length,
          raids: dbRaidsAfter.length
        },
        skipped
      }
    };
  } catch (error: any) {
    log('ERROR', `迁移过程出错: ${error.message}`);
    await db.setMigrationStatus('failed', error.message);
    return {
      success: false,
      message: '迁移失败: ' + error.message,
      migrated: { accounts: 0, records: 0, raids: 0, config: false },
      details: {
        dbBefore: { accounts: 0, records: 0, raids: 0 },
        dbAfter: { accounts: 0, records: 0, raids: 0 },
        skipped: { accounts: 0, records: 0, raids: 0 }
      }
    };
  }
}

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

export async function manualMigrate(): Promise<{
  success: boolean;
  message: string;
}> {
  log('INFO', '=== 手动触发迁移 ===');

  const result = await forceMigrate();

  if (result.success) {
    log('INFO', '迁移成功！请重启应用。');
    return { success: true, message: '迁移成功！请重启应用。' };
  } else {
    log('ERROR', `迁移失败: ${result.message}`);
    return { success: false, message: `迁移失败: ${result.message}` };
  }
}
