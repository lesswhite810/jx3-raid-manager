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
  sect?: string; // Class/Sect e.g., ChunYang, WanHua (由后端根据 martial 自动映射)
  martial?: string; // 心法名称
  isClient?: boolean; // @deprecated 使用 Account.type 判断代清账号
  disabled?: boolean; // 是否禁用该角色
  equipmentScore?: number; // 装备分数（装分）
  visibility?: Record<string, boolean>; // 可见性配置 { raid: true, baizhan: false, trial: true }
}

// 副本类型
export interface InstanceType {
  id: number;
  type: string;         // 'raid' | 'baizhan' | 'trial'
  name: string;         // '团队副本' | '百战' | '试炼之地'
}

// 角色副本可见性
export interface RoleInstanceVisibility {
  id: string;
  roleId: string;
  instanceTypeId: number;
  instanceType: string;
  visible: boolean;
}

// 副本类型常量
export const INSTANCE_TYPES = {
  RAID: 'raid',
  BAIZHAN: 'baizhan',
  TRIAL: 'trial'
} as const;

export type InstanceTypeCode = typeof INSTANCE_TYPES[keyof typeof INSTANCE_TYPES];

export interface Account {
  id: string;
  accountName: string; // 账号名称（目录中的账号名）
  username?: string; // Game Account (optional)
  password?: string; // Game Password (optional)
  type: AccountType;
  sortOrder?: number; // 账号显示排序
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
  date: string | number;
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
  hasSecretBook?: boolean; // 秘籍
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
  date: string | number;
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
}

export interface TrialFlipPositionStats {
  position: number;
  flipCount: number;
  flippedEquipmentCount: number;
  appearanceCount: number;
  flipEquipmentRate: number;
  appearanceRate: number;
}

export interface TrialBossEquipmentPositionStats {
  position: number;
  appearanceCount: number;
  appearanceRate: number;
}

export interface TrialBossEquipmentStats {
  bossKey: string;
  bosses: [string, string, string];
  totalRecords: number;
  equipmentCount: number;
  positions: TrialBossEquipmentPositionStats[];
  bestEquipmentPosition: TrialBossEquipmentPositionStats | null;
}

export interface TrialFlipStatsSummary {
  totalRecords: number;
  positions: TrialFlipPositionStats[];
  bestFlipPosition: TrialFlipPositionStats | null;
  bestAppearancePosition: TrialFlipPositionStats | null;
  bossEquipmentStats: TrialBossEquipmentStats[];
}

export type AnyRecord = RaidRecord | TrialPlaceRecord | BaizhanRecord;

export interface DashboardStats {
  totalGold: number;
  totalRaids: number;
  xuanjingCount: number;
  equipCount: number;
  dropRate: number;
  clientIncome: number;
}

export interface Raid {
  name: string;
  difficulty: '普通' | '英雄' | '挑战';
  playerCount: 10 | 25;
  version?: string;
  notes?: string;
  isActive: boolean;
  static?: boolean;
  bosses?: Boss[]; // BOSS 列表，有值时启用 BOSS CD 追踪
}

// BOSS CD 追踪相关类型
export interface Boss {
  id: string;
  name: string;
  order: number; // BOSS 顺序 (1-6)
}

export interface RaidBossConfig {
  raidName: string;
  difficulty: '普通' | '英雄' | '挑战';
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
  date: string | number;
  roleId: string;
  accountId: string;
  bossIds?: string[]; // 多选BOSS ID列表（25人本）
  bossNames?: string[]; // 多选BOSS名称列表（25人本）
}

export interface BossCooldownInfo {
  bossId: string;
  bossName: string;
  hasRecord: boolean; // 本 CD 周期内是否有记录
  lastRecordDate?: string | number;
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

export interface TrialConfig {
  autoFillEnabled: boolean;
}

export interface Config {
  ai: AIConfig;
  game: GameConfig;
  trial?: TrialConfig;
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

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'installing'
  | 'upToDate'
  | 'error'
  | 'portableManualOnly';

export interface UpdateRuntimeInfo {
  currentVersion: string;
  executablePath: string;
  isPortable: boolean;
  willInstallInPlace: boolean;
  hasUninstallExecutable: boolean;
  updaterConfigured: boolean;
  releaseUrl: string;
}

export interface UpdateCheckResult {
  currentVersion: string;
  available: boolean;
  version?: string;
  body?: string;
  pubDate?: string;
  isPortable: boolean;
  willInstallInPlace: boolean;
  updaterConfigured: boolean;
  releaseUrl: string;
}

export interface UpdateProgressPayload {
  event: 'started' | 'progress' | 'finished';
  contentLength?: number;
  chunkLength?: number;
  downloadedBytes?: number;
  totalBytes?: number;
}

// 百战记录类型
export interface BaizhanRecord {
  id: string;
  accountId: string;
  roleId: string;
  roleName: string;
  server: string;
  date: string | number;
  goldIncome: number;   // 收入（金）
  goldExpense?: number; // 支出（金）
  notes?: string;
  type: 'baizhan';
}

// ========== 赛季系统 ==========

export interface GameVersion {
  id?: number;
  name: string;
  sortOrder: number;
}

export interface Season {
  id?: number;
  name: string;
  versionId: number;
  startDate: number;
  endDate?: number;
  sortOrder: number;
  trialEquipLevelMin?: number;
  trialEquipLevelMax?: number;
}

// ==================== 活跃检测（C 阶段） ====================

/** 活跃等级（与后端 enum 对应，使用 lowercase） */
export type AccountActiveLevel = 'active' | 'recent' | 'idle' | 'offline';

/** 单个角色的活跃状态（来自茗伊目录扫描 + info.jx3dat 解析） */
export interface RoleActiveState {
  /** 茗伊 uid（目录名中的数字 ID） */
  uid: string;
  /** 角色名（从 info.jx3dat 解析） */
  roleName: string;
  /** 服务器（从 info.jx3dat 解析） */
  server: string;
  /** 大区（从 info.jx3dat 解析） */
  region: string;
  activeLevel: AccountActiveLevel;
  isOnline: boolean;
  isRecentlyActive: boolean;
  lastActivityTime: string | null;
  lastActivitySource: string | null;
}

/** JX3 进程信息 */
export interface Jx3ProcessInfo {
  pid: number;
  name: string;
  startTime: string;
  startTimeUnix: number;
  exePath: string;
  workDirectory: string;
}

/** JX3 运行时状态 */
export interface Jx3RuntimeStatus {
  isRunning: boolean;
  matchedProcess: Jx3ProcessInfo | null;
  allProcesses: Jx3ProcessInfo[];
  /** 匹配配置游戏目录的进程数量（多开时每个进程对应一个当前在线角色） */
  matchedProcessCount: number;
  multiInstanceDetected: boolean;
  multiInstanceHint: string | null;
}

/** 批量活跃检测结果 */
export interface BatchActiveResult {
  jx3Running: boolean;
  jx3StartTime: string | null;
  jx3EndTime: string | null;
  jx3ProcessCount: number;
  multiInstanceDetected: boolean;
  multiInstanceHint: string | null;
  roles: RoleActiveState[];
  scanDurationMs: number;
}

/** 应用配置（来自 app_config 表） */
export interface AppConfig {
  gameDirectory: string | null;
  setupCompleted: boolean;
  lastScanMingyiAt: string | null;
  accountIds: string[];
}
