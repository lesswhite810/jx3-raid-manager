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
  transactionId?: string;
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
  hasMaJu?: boolean;
  hasPet?: boolean;
  hasPendant?: boolean;
  hasMount?: boolean;
  hasAppearance?: boolean;
  hasTitle?: boolean;
  type?: 'raid' | 'trial';
  bossId?: string;
  bossName?: string;
  bossIds?: string[]; // 多选BOSS ID列表（25人本）
  bossNames?: string[]; // 多选BOSS名称列表（25人本）
}

export interface TrialPlaceRecord {
  id: string;
  accountId: string;
  roleId: string;
  roleName: string;
  server: string;
  date: string;
  layer: number; // 1-100
  bosses: [string, string, string]; // 3 bosses
  // Flat structure for cards (Equip IDs)
  card1: string;
  card2: string;
  card3: string;
  card4: string;
  card5: string;
  flippedIndex: number; // 1-5
  type: 'trial';
  notes?: string;
  transactionType?: 'income' | 'expense' | 'combined'; // meaningful for trial? prob not but for compatibility
}

export type AnyRecord = RaidRecord | TrialPlaceRecord;

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

// BOSS CD 追踪相关类型
export interface Boss {
  id: string;
  name: string;
  order: number; // BOSS 顺序 (1-6)
}

export interface RaidBossConfig {
  raidName: string;
  difficulty: 'NORMAL' | 'HEROIC' | 'CHALLENGE';
  playerCount: 10 | 25;
  version: string;
  bosses: Boss[];
  hasBossTracking: boolean; // 是否启用 BOSS CD 追踪
}

export interface BossRecord {
  id: string;
  raidRecordId: string; // 关联的副本记录 ID
  bossId: string;
  bossName: string;
  date: string;
  roleId: string;
  accountId: string;
  bossIds?: string[]; // 多选BOSS ID列表（25人本）
  bossNames?: string[]; // 多选BOSS名称列表（25人本）
}

export interface BossCooldownInfo {
  bossId: string;
  bossName: string;
  hasRecord: boolean; // 本 CD 周期内是否有记录
  lastRecordDate?: string;
  canAddRecord: boolean; // 是否可以添加记录
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

// 百战记录类型
export interface BaizhanRecord {
  id: string;
  accountId: string;
  roleId: string;
  roleName: string;
  server: string;
  date: string;
  difficulty: 3 | 6 | 10; // 3人/6人/10人
  bossId: string;
  bossName: string;
  skillColor: string; // 百战技能颜色
  skillLevel: number; // 精耐等级
  rewards?: string[]; // 奖励列表
  notes?: string;
  type: 'baizhan';
}

// 百战BOSS信息
export interface BaizhanBoss {
  id: string;
  name: string;
  tier: number; // 阶数 (1-10)
  recommendedSkillLevel: number; // 推荐精耐等级
  difficulty: 3 | 6 | 10; // 可挑战难度
}