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
  // 以下字段来自账号级配置（UI 展示复制账号密码时使用，后端通常不存储于角色表）
  password?: string;
  accountName?: string;
  accountId?: string;
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

/** 散件清单项（散件老板功能，自动扫描计算 + 用户手填单价） */
export interface ScrapsItem {
  name: string;
  count: number;
  /** 单价（金）。null 表示需用户在前端手填 */
  unitPrice: number | null;
  /** 该物品的累计实际购买价（金）。旧版记录可能缺失（undefined） */
  totalPrice?: number;
  category: 'material' | 'equipment';
  priceSource: 'jx3box' | 'npc' | 'manual';
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
  source?: 'auto' | 'manual'; // 记录来源，默认 'manual'
  status?: 'pending' | 'confirmed' | 'rejected' | 'scanning'; // 确认状态，默认 'confirmed'；'scanning' 表示副本进行中，UI 锁定不可确认
  drops?: string[]; // 掉落物原始物品名列表（自动扫描）
  jclFiles?: string[]; // 关联 JCL 文件名列表（自动扫描溯源）
  /** 散件清单（散件老板功能，自动扫描计算） */
  scrapsItems?: ScrapsItem[];
  /** 散件估价总和（扫描时快照；unitPrice 未填齐时为部分和） */
  scrapsValue?: number;
  /** 散件支出（白名单材料的实际购买花费合计；其他装备/小铁等购买计入 goldExpense 不计入此字段） */
  scrapsExpense?: number;
  /** 是否为散件老板（确认时有散件清单默认选中，用户可取消） */
  isScrapsBoss?: boolean;
  /** 自动扫描发现的非首领奖励收入候选，待确认记录可供用户选择。 */
  incomeCandidates?: IncomeCandidate[];
  /** 用户确认时选中的收入候选 ID；旧记录缺失时保持 undefined。 */
  selectedIncomeCandidateIds?: string[];
}

export interface IncomeCandidate {
  id: string;
  /** 消息时间（Unix 秒或毫秒）。 */
  time: number;
  /** 以金为单位的整数金额，兼容现有收入字段。 */
  amount: number;
  /** 原始铜数，存在时用于精确展示。 */
  amountCopper?: number;
  source?: string;
}

/** 单次翻牌的收据、牌位与画面识别结果。 */
export interface TrialDrawReward {
  name: string;
  quantity: number;
}

export interface TrialDraw {
  id: string;
  sourceKey: string;
  /** 同一轮内按收据时间排序、从 1 开始的序号。 */
  sequence: number;
  /** 同秒收据只能稳定显示排序，不能证明真实翻牌先后。 */
  sequenceAmbiguous?: boolean;
  /** 手工翻牌没有日志收据时间；自动翻牌保留日志来源。 */
  source?: 'manual' | 'auto';
  receiptTime: string | number;
  /** 聊天记录哈希可能超过 JavaScript 安全整数范围，须保持字符串。 */
  receiptHash?: string;
  /** 1-5 为已知翻牌位；0 表示未知。 */
  flippedIndex: number;
  rewardItems: TrialDrawReward[];
  /** 本次翻牌确认的精力消耗；未观察到时省略。 */
  staminaCost?: number;
  /** 同一收据窗口内拆分记录的精力消耗，用于保留原始明细。 */
  staminaCosts?: number[];
  // Flat structure for cards (Equip IDs), scoped to this draw.
  card1: string;
  card2: string;
  card3: string;
  card4: string;
  card5: string;
  /** 此次牌面尚未完整核对；不可据此将未观察牌视为无装备。 */
  equipmentUnreviewed?: boolean;
  /** 人工修改过的牌位，包含明确清空的牌。 */
  manualEquipmentCardIndexes?: number[];
  /** 本次表单明确修改的牌位，仅用于保存意图，不作为持久数据。 */
  editedCardIndices?: number[];
}

export interface TrialPlaceRecord {
  id: string;
  accountId: string;
  roleId: string;
  roleName: string;
  server: string;
  date: string | number;
  /** 0 表示自动扫描尚未取得层数，待用户补录确认。 */
  layer: number; // 0-120
  /** 三关 Boss。空字符串表示自动扫描尚未取得对应关卡。 */
  bosses: [string, string, string];
  // Flat structure for cards (Equip IDs)
  card1: string;
  card2: string;
  card3: string;
  card4: string;
  card5: string;
  /** 1-5 为已知翻牌位，0 表示扫描未能识别翻牌位。 */
  flippedIndex: number;
  type: 'trial';
  notes?: string;
  /** 旧版手工记录缺失时按 manual 兼容。 */
  source?: 'auto' | 'manual';
  /** 旧版手工记录缺失时按 confirmed 兼容。 */
  status?: 'pending' | 'confirmed';
  /** 自动扫描候选的稳定来源键，用于幂等写入。 */
  sourceKey?: string;
  /** 五张牌尚未完整核对时为 true。已识别的选中牌可独立保留，旧记录按 false 兼容。 */
  equipmentUnreviewed?: boolean;
  /** 编辑草稿中用户实际修改过的牌位。后端会保留其余空位晚到的识别结果，不持久化。 */
  editedCardIndices?: number[];
  /** 一轮通关后的翻牌明细。旧记录固定按空数组兼容，绝不把父记录旧牌面虚构为首笔。 */
  draws?: TrialDraw[];
}

export interface TrialFlipPositionStats {
  position: number;
  flipCount: number;
  /** 该位置已确认装备结果的翻牌样本数。 */
  equipmentReviewCount: number;
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
  /** 用于翻牌统计的明细数：新记录取子笔，旧记录兼容为一笔。 */
  drawCount: number;
  /** 已确认翻牌位的明细数。 */
  knownPositionDrawCount: number;
  /** 已完整核对五张牌的记录数，用作装备出现率和 Boss 统计的分母。 */
  equipmentReviewedRecords: number;
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
  /** 散件估价合计（仅统计 isScrapsBoss=true 的记录，不并入 totalGold） */
  totalScrapsValue?: number;
  /** 散件支出合计（白名单材料的实际购买花费；装备/小铁等其他购买不计入） */
  totalScrapsExpense?: number;
}

export interface Raid {
  name: string;
  difficulty: '普通' | '英雄' | '挑战';
  playerCount: 10 | 25;
  version?: string;
  /** 所属赛季 id；历史数据可能为空，前端按「未知赛季」处理，不能当作本赛季 */
  seasonId?: number;
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
  status?: 'pending' | 'confirmed' | 'rejected' | 'scanning'; // 确认状态，rejected 不参与 CD 计算；'scanning' 表示副本进行中
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
  maxTokens?: number;
  timeout?: number;
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
  // Rust Option<i64> 在 IPC 中会序列化为 null；null 与 0 都表示该端未配置。
  trialEquipLevelMin?: number | null;
  trialEquipLevelMax?: number | null;
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

/** JX3 进程会话历史（一次运行到退出的时间范围） */
export interface ProcessSession {
  startTimeUnix: number;
  startTime: string;
  endTimeUnix: number | null;
  endTime: string | null;
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
  /** JX3 进程会话历史（本次应用运行期间） */
  processSessions: ProcessSession[];
}

/** 关闭主窗口时的行为：每次询问 / 最小化到托盘 / 直接退出 */
export type CloseBehavior = 'ask' | 'minimize' | 'exit';

/** 应用配置（来自 app_config 表） */
export interface AppConfig {
  gameDirectory: string | null;
  setupCompleted: boolean;
  lastScanMingyiAt: string | null;
  autoScanEnabled: boolean;
  /** 是否在自动扫描的试炼结算流程中识别已选翻牌位，默认关闭。 */
  trialScreenshotRecognitionEnabled: boolean;
  autoRefreshEquipScore: boolean;
  /** 点击关闭按钮时的行为，默认每次询问。 */
  closeBehavior: CloseBehavior;
}
