/**
 * Database Service Wrapper
 * 根据运行环境自动选择真实数据库或模拟数据库
 */

import { db as dbReal } from './db';
import { db as dbMock } from './db-mock';

// 检测是否在Tauri环境中运行
const isTauriEnvironment = (): boolean => {
  return typeof window !== 'undefined' &&
         !!(window as any).__TAURI__;
};

// 导出统一的数据库服务实例
export const db = isTauriEnvironment() ? dbReal : dbMock;

// 打印当前使用的数据库服务
console.log(`📦 Database Service: ${isTauriEnvironment() ? 'Real (Tauri)' : 'Mock (Web)'}`);

// 重新导出类型，方便IDE提示
export type DatabaseService = typeof dbReal;
