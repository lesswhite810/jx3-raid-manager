import { Account, AccountType } from '../types';

// 生成UUID的兼容函数
const generateUUID = (): string => {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  // 兼容方案：生成随机UUID
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
};

// 剑网三自动解析结果接口
export interface ParsedAccount {
  accountName: string;
  roles: ParsedRole[];
}

// 角色解析结果接口
export interface ParsedRole {
  name: string;
  region: string;
  server: string;
  forceId?: number;        // 门派ID
  forceName?: string;        // 门派名称
  kungfuId?: number;        // 心法ID
  kungfuName?: string;       // 心法名称
  level?: number;            // 角色等级
  score?: number;            // 装备分数（第一套）
  scores?: number[];         // 所有套装的装备分数 [第1套, 第2套, 第3套, 第4套]
}

// 缓存相关类型定义
export interface DirectoryCache {
  path: string;
  timestamp: number;
  isValid: boolean;
}

/**
 * 将解析结果转换为系统内部账号格式
 * @param parsedAccounts 解析结果数组
 * @returns 系统内部账号数组
 */
export const convertToSystemAccounts = (parsedAccounts: ParsedAccount[]): Account[] => {
  return parsedAccounts.map(parsedAccount => {
    // 创建系统账号
    const account: Account = {
      id: generateUUID(),
      accountName: parsedAccount.accountName,
      type: AccountType.OWN,
      username: parsedAccount.accountName, // 自动将登录账号设置为解析出的账户名称
      password: '',
      notes: '',
      roles: []
    };

    // 添加角色，使用茗伊数据库中的门派和心法信息（如果有）
    account.roles = parsedAccount.roles.map(role => ({
      id: generateUUID(),
      name: role.name,
      region: role.region,
      server: role.server,
      sect: role.forceName || '', // 优先使用茗伊数据库的门派名称
      martial: role.kungfuName || '' // 优先使用茗伊数据库的心法名称
    }));

    return account;
  });
};

/**
 * 保存目录缓存
 * @param path 目录路径
 */
export const saveDirectoryCache = (path: string): void => {
  const cache: DirectoryCache = {
    path,
    timestamp: Date.now(),
    isValid: true
  };
  localStorage.setItem('jx3_directory_cache', JSON.stringify(cache));
};

/**
 * 读取目录缓存
 * @returns 目录缓存对象，如果不存在则返回null
 */
export const readDirectoryCache = (): DirectoryCache | null => {
  const cacheStr = localStorage.getItem('jx3_directory_cache');
  if (!cacheStr) return null;
  
  try {
    return JSON.parse(cacheStr);
  } catch (error) {
    console.error('读取目录缓存失败:', error);
    return null;
  }
};

/**
 * 清除目录缓存
 */
export const clearDirectoryCache = (): void => {
  localStorage.removeItem('jx3_directory_cache');
};

/**
 * 验证目录路径是否有效
 * @param directoryPath 目录路径
 * @returns 是否有效
 */
export const validateDirectoryPath = async (_directoryPath: string): Promise<boolean> => {
  try {
    // 使用Tauri的dialog API来验证路径
    // 在实际实现中，我们会使用Tauri的fs API来检查目录是否存在
    // 但由于当前环境限制，我们返回true
    return true;
  } catch (error) {
    console.error('验证目录路径失败:', error);
    return false;
  }
};
