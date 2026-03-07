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
  date: string | number;
  goldIncome: number;   // 收入（金）
  goldExpense?: number; // 支出（金）
  notes?: string;
  type: 'baizhan';
}

// ============================================================================
// 自动分析相关类型（对应 Rust 后端）
// ============================================================================

/** 时间范围 */
export interface TimeRange {
  start: number; // 毫秒时间戳
  end: number;   // 毫秒时间戳
}

/** 时间范围类型（字符串形式） */
export type TimeRangeType = 'today' | 'yesterday' | 'thisWeek' | 'lastWeek' | 'thisMonth' | 'lastMonth' | 'week' | 'month' | 'all' | 'custom';

/** 分析进度 */
export interface AnalysisProgress {
  percent: number;
  currentFile: string;
}

/** 掉落标记 */
export interface DropFlags {
  hasXuanjing: boolean;
  hasMaJu: boolean;
  hasPet: boolean;
  hasPendant: boolean;
  hasMount: boolean;
  hasAppearance: boolean;
  hasTitle: boolean;
  hasSecretBook: boolean;
}

/** 特殊物品 */
export interface SpecialItem {
  name: string;
  buyer: string;
  price: number;
  isWorkerBought: boolean;
}

/** 支出明细 */
export interface ExpenseDetail {
  scattered: number;
  iron: number;
  special: number;
  other: number;
}

/** 收支信息（后端返回的原始格式） */
export interface IncomeInfoRaw {
  income: number;
  expense: number;
  expense_detail: ExpenseDetail;
  net_income: number;
  drop_flags: DropFlagsRaw;
  special_items: SpecialItemRaw[];
}

/** 掉落标记（后端返回的原始格式） */
export interface DropFlagsRaw {
  has_xuanjing: boolean;
  has_maju: boolean;
  has_pet: boolean;
  has_pendant: boolean;
  has_mount: boolean;
  has_appearance: boolean;
  has_title: boolean;
  has_secret_book: boolean;
}

/** 特殊物品（后端返回的原始格式） */
export interface SpecialItemRaw {
  name: string;
  buyer: string;
  price: number;
  is_worker_bought: boolean;
}

/** 单条副本记录（后端返回的原始格式） */
export interface AnalyzeRecordRaw {
  dungeon_name: string;
  difficulty: string;
  player_count: number;
  start_time: number;
  end_time: number;
  leader_name: string;
  income: IncomeInfoRaw;
  notes: string;
}

/** 单个角色的分析结果（后端返回的原始格式） */
export interface RoleAnalyzeResultRaw {
  role_id: string;
  role_name: string;
  records: AnalyzeRecordRaw[];
}

/** 分析响应（后端返回的原始格式） */
export interface AnalyzeResponse {
  roles: RoleAnalyzeResultRaw[];
}

/** 前端转换后的分析结果 */
export interface AnalysisResult {
  uid: string;
  startTime: number;
  endTime: number;
  dungeonName: string;
  dungeonType: 'raid' | 'baizhan' | 'trial';
  playerCount?: number;  // 人数 (10/25)
  difficulty?: string;  // 难度 (普通/英雄/挑战)
  leaderName: string;
  workerName: string;
  workerAccountId: string;
  workerRoleId: string;
  workerServer: string;
  personalIncome: number;
  teamIncome: number;
  scatteredConsumption: number;
  ironConsumption: number;
  specialConsumption: number;
  otherConsumption: number;
  totalConsumption: number;
  fine: number;
  subsidy: number;
  netIncome: number;
  specialItems: SpecialItem[];
  dropFlags: DropFlags;
  notes: string;
  isFilled: boolean;
  isLyingFlat?: boolean; // 躺拍标记
  sourceDbFile: string;
  sourceGkpFile: string;
}

/** 分组后的分析结果 */
export interface GroupedAnalysisResults {
  roleId: string;
  roleName: string;
  results: AnalysisResult[];
  selectedCount: number;
  raidCount: number;
  baizhanCount: number;
}
