export enum AccountType {
  OWN = 'OWN',
  CLIENT = 'CLIENT'
}

export enum TransactionType {
  INCOME = 'income',
  EXPENSE = 'expense'
}

export interface Role {
  id: string;
  name: string; // Character Name
  server: string; // 区服名称
  region: string; // 大区名称
  sect: string; // Class/Sect e.g., ChunYang, WanHua
  isClient?: boolean; // @deprecated 使用 Account.type 判断代清账号
  disabled?: boolean; // 是否禁用该角色
  equipmentScore?: number; // 装备分数（装分）
}

export interface Account {
  id: string;
  accountName: string; // 账号名称（目录中的账号名）
  username?: string; // Game Account (optional)
  password?: string; // Game Password (optional)
  type: AccountType;
  notes?: string;
  roles: Role[]; // 角色列表
  hidden?: boolean; // 是否隐藏该账户，防止重复解析
  disabled?: boolean; // 是否禁用该账户
}

export interface RaidRecord {
  id: string;
  transactionId?: string; // 交易ID，用于关联同一副本的收入和支出记录
  accountId: string;
  roleId: string;
  raidName: string;
  date: string;
  goldIncome: number;
  goldExpense?: number;
  isCleared?: boolean;
  hasXuanjing: boolean;
  notes?: string;
  roleName?: string;
  server?: string;
  transactionType?: 'income' | 'expense' | 'combined';
  // Special Drops
  hasMaJu?: boolean;      // 马具
  hasPet?: boolean;       // 宠物
  hasPendant?: boolean;   // 挂件
  hasMount?: boolean;     // 坐骑
  hasAppearance?: boolean; // 外观
  hasTitle?: boolean;     // 称号
}

export interface DashboardStats {
  totalGold: number;
  totalRaids: number;
  xuanjingCount: number;
  dropRate: number;
  clientIncome: number;
}

export interface Raid {
  name: string;
  difficulty: 'NORMAL' | 'HEROIC' | 'CHALLENGE';
  playerCount: 10 | 25;
  version?: string;
  notes?: string;
  isActive: boolean;
  static?: boolean;
}

// 配置接口
export interface AIConfig {
  enabled: boolean;
  apiKey: string;
  model: string;
  temperature: number;
  proxyUrl?: string;
  proxyEnabled?: boolean;
}

export interface GameConfig {
  gameDirectory: string;
  autoDetectEnabled: boolean;
}

export interface Config {
  ai: AIConfig;
  game: GameConfig;
}

// Toast 通知类型定义
export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface ToastMessage {
  id: string;
  type: ToastType;
  message: string;
  duration?: number;
  dismissible?: boolean;
}

export interface ToastOptions {
  type?: ToastType;
  duration?: number;
  dismissible?: boolean;
}