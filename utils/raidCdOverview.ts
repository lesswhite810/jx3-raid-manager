import { Account, AccountType, BaizhanRecord, BossCooldownInfo, BossRecord, Raid, RaidRecord, Role, TrialPlaceRecord } from '../types';
import { calculateCooldown, getLastMonday, getNextMonday } from './cooldownManager';
import { calculateBossCooldowns } from './bossCooldownManager';
import { getRaidKey } from './raidUtils';
import { shouldShowClientRoleInRaid } from './raidVersionUtils';
import { buildWeeklyRaidGroups, SeasonContext } from './weeklyRaids';
import { getTrialWeeklyDrawProgress } from './trialWeeklyDrawProgress';
import { TrialProgressPeriod } from './trialProgress';

/**
 * CD 概览单元格状态。
 * - none：本周尚未清（还需要打）
 * - partial：部分 BOSS 已清
 * - pending：有自动扫描记录待确认（未确认前不算已清）
 * - complete：本周已清完
 * - na：该副本难度不适用该角色（10 人本不展示代清号角色）
 */
export type CdCellStatus = 'none' | 'partial' | 'pending' | 'complete' | 'na';

export interface CdCell {
  status: CdCellStatus;
  /** 已击杀 BOSS 数 */
  clearedCount: number;
  /** BOSS 总数（没有 BOSS 配置时为 0） */
  totalCount: number;
  /**
   * 该角色在该副本的 BOSS 冷却明细。
   * 添加记录弹窗用它默认勾选「还没打的 BOSS」，避免用户手动去掉已打过的。
   */
  bossCooldowns?: BossCooldownInfo[];
}

export interface CdOverviewColumn {
  /** 与记录 raidName 一致的实例键 */
  key: string;
  raid: Raid;
  /** 副本名（同名不同难度共用） */
  raidName: string;
  source: 'weekly' | 'favorite';
  /** 该列已清的角色数 */
  clearedCount: number;
  /** 参与统计的角色数（不含不适用） */
  totalCount: number;
}

/**
 * 试炼之地 / 百战这类"按周次数"进度的单元格。
 *
 * 与副本列的区别：副本按 BOSS 完成度算，这两列按本周完成次数算，
 * 因此 `totalCount` 是本周目标次数而不是 BOSS 总数。
 */
export interface CdActivityCell {
  status: CdCellStatus;
  /** 本周已完成次数 */
  clearedCount: number;
  /** 本周目标次数 */
  totalCount: number;
}

export interface CdOverviewRow {
  roleId: string;
  roleName: string;
  server: string;
  /** 大区，构造各记录弹窗的角色信息时需要 */
  region: string;
  sect?: string;
  martial?: string;
  accountId: string;
  accountName: string;
  accountType: AccountType;
  /** 装分；未填写时为 undefined，排序时排在最后 */
  equipmentScore?: number;
  /** 所属账号密码；未配置时为 undefined（用于操作列的复制按钮） */
  password?: string;
  cells: Record<string, CdCell>;
  /** 试炼之地本周翻牌进度 */
  trial: CdActivityCell;
  /** 百战本周进度 */
  baizhan: CdActivityCell;
  noneClearedCount: number;
  partialClearedCount: number;
  pendingClearedCount: number;
  completeClearedCount: number;
}

export interface CdOverviewResult {
  columns: CdOverviewColumn[];
  rows: CdOverviewRow[];
  /** 被隐藏的已禁用副本数量，用于界面说明 */
  disabledColumnCount: number;
  /** 是否有角色参与试炼之地；为 false 时界面不展示试炼之地列 */
  hasTrialParticipants: boolean;
  /** 是否有角色参与百战；为 false 时界面不展示百战列 */
  hasBaizhanParticipants: boolean;
}

export interface BuildRaidCdOverviewOptions {
  raids: Raid[];
  accounts: Account[];
  records: RaidRecord[];
  /** 试炼之地记录，用于"本周翻牌"列 */
  trialRecords?: TrialPlaceRecord[];
  /** 百战记录，用于"本周次数"列 */
  baizhanRecords?: BaizhanRecord[];
  /**
   * 试炼进度周期。undefined 表示仍在加载，此时试炼列不参与统计（全部按"不适用"处理），
   * 避免把「还没拿到周期」显示成「本周没打」。
   */
  trialProgressPeriod?: TrialProgressPeriod | null;
  weeklyRaidNames: string[];
  favoriteRaids: ReadonlySet<string>;
  seasonContext: SeasonContext;
  /**
   * 大版本名 → 新旧排名（0 最新）。用于把周常副本按「离现在最近」排序。
   * 缺省或版本名不在表里时，该副本排在所有已知版本之后。
   */
  versionOrder?: Record<string, number>;
  now?: Date;
}

const PLAYER_COUNT_ORDER: Record<number, number> = { 10: 0, 25: 1 };
const DIFFICULTY_ORDER: Record<string, number> = { '普通': 0, '英雄': 1, '挑战': 2 };

/** 试炼之地每周目标翻牌次数，与详情页「三次打满」口径一致 */
export const TRIAL_WEEKLY_TARGET = 3;

/** 百战每周目标次数 */
export const BAIZHAN_WEEKLY_TARGET = 1;

const compareRaidInstance = (a: Raid, b: Raid): number => {
  const playerCountDiff = (PLAYER_COUNT_ORDER[a.playerCount] ?? 99) - (PLAYER_COUNT_ORDER[b.playerCount] ?? 99);
  if (playerCountDiff !== 0) return playerCountDiff;
  return (DIFFICULTY_ORDER[a.difficulty] ?? 99) - (DIFFICULTY_ORDER[b.difficulty] ?? 99);
};

/** 与记录 raidName 的构造规则保持一致，用于判断记录是否属于某个副本难度 */
const matchesRaidInstance = (record: RaidRecord, raid: Raid): boolean => {
  if (!record.raidName) return false;
  return (
    record.raidName.includes(raid.name) &&
    record.raidName.includes(raid.difficulty) &&
    record.raidName.includes(`${raid.playerCount}人`)
  );
};

/** 与 RaidDetail 一致：自动扫描的待确认 / 已拒绝记录不参与"已清"统计 */
const isConfirmedRecord = (record: RaidRecord): boolean =>
  !(record.source === 'auto' && (record.status === 'pending' || record.status === 'rejected'));

/** 与 RaidDetail 一致：单条记录按 bossIds 展开为多条 BOSS 记录 */
const expandBossRecords = (records: RaidRecord[]): BossRecord[] =>
  records.flatMap(record => {
    if (record.bossIds && record.bossIds.length > 0) {
      return record.bossIds.map(bossId => ({
        id: record.id,
        raidRecordId: record.id,
        bossId,
        bossName: record.bossNames?.[record.bossIds?.indexOf(bossId) || 0] || '',
        date: record.date,
        roleId: record.roleId,
        accountId: record.accountId,
      }));
    }
    return [{
      id: record.id,
      raidRecordId: record.id,
      bossId: record.bossId || '',
      bossName: record.bossName || '',
      date: record.date,
      roleId: record.roleId,
      accountId: record.accountId,
    }];
  });

/**
 * 组装团队副本 CD 概览表。
 *
 * 列 = 本周团队秘境的每个难度 + 收藏副本的每个难度（跳过已禁用的副本）。
 * 行 = 未禁用账号下、未被 `visibility.raid` 排除的角色。
 * 单元格判定完全复用详情页的口径：待确认优先，其次按 BOSS 完成度区分未清 / 部分清 / 已清。
 */
export function buildRaidCdOverview(options: BuildRaidCdOverviewOptions): CdOverviewResult {
  const { raids, accounts, records, weeklyRaidNames, favoriteRaids, seasonContext } = options;
  const versionOrder = options.versionOrder ?? {};
  const safeTrialRecords = Array.isArray(options.trialRecords) ? options.trialRecords : [];
  const safeBaizhanRecords = Array.isArray(options.baizhanRecords) ? options.baizhanRecords : [];
  const trialProgressPeriod = options.trialProgressPeriod;
  const now = options.now ?? new Date();

  const safeRaids = Array.isArray(raids) ? raids : [];
  const safeAccounts = Array.isArray(accounts) ? accounts : [];
  const safeRecords = Array.isArray(records) ? records : [];
  const activeRaids = safeRaids.filter(raid => raid.isActive);

  // 1. 列：本周团队秘境 → 收藏副本。
  //    周常副本按赛季新旧排序（离现在最近的赛季在前），同赛季内按人数、难度排序；
  //    收藏副本排在周常之后，同样按人数、难度排序。
  const weeklyGroups = buildWeeklyRaidGroups(safeRaids, weeklyRaidNames, favoriteRaids, seasonContext);
  const weeklyRaidList: Raid[] = weeklyGroups.flatMap(group => group.raids);

  /** 版本越新排名越小；未知版本排最后 */
  const getVersionRank = (raid: Raid): number => {
    const rank = raid.version ? versionOrder[raid.version] : undefined;
    return typeof rank === 'number' ? rank : Number.MAX_SAFE_INTEGER;
  };

  /** 周常列排序：赛季新旧 → 人数 → 难度 */
  const compareWeeklyRaid = (a: Raid, b: Raid): number => {
    const versionDiff = getVersionRank(a) - getVersionRank(b);
    if (versionDiff !== 0) return versionDiff;
    return compareRaidInstance(a, b);
  };

  const weeklyNames = new Set(weeklyGroups.map(group => group.name));
  const favoriteRaidList = activeRaids
    .filter(raid => favoriteRaids.has(raid.name) && !weeklyNames.has(raid.name))
    .sort(compareRaidInstance);

  const columnRaids: { raid: Raid; source: 'weekly' | 'favorite' }[] = [
    ...weeklyRaidList.filter(raid => raid.isActive).sort(compareWeeklyRaid).map(raid => ({ raid, source: 'weekly' as const })),
    ...favoriteRaidList.map(raid => ({ raid, source: 'favorite' as const })),
  ];

  // 去重：同一副本难度只出现一次（本周区优先）
  const seenKeys = new Set<string>();
  const uniqueColumnRaids = columnRaids.filter(item => {
    const key = getRaidKey(item.raid);
    if (seenKeys.has(key)) return false;
    seenKeys.add(key);
    return true;
  });

  const disabledColumnCount = safeRaids.filter(
    raid => !raid.isActive && (favoriteRaids.has(raid.name) || weeklyRaidNames.includes(raid.name)),
  ).length;

  // 2. 时间窗：与详情页保持一致，先按周窗口收集本周记录
  const periodStart = getLastMonday(now).getTime();
  const periodEnd = getNextMonday(now).getTime();

  const columns: CdOverviewColumn[] = [];
  const roleRows: (CdOverviewRow & { sourceIndex: number })[] = [];

  // 3. 行：账号下可展示的角色
  interface RowSource {
    account: Account;
    role: Role;
    roleId: string;
    roleName: string;
    server: string;
    region: string;
    equipmentScore?: number;
    /** 该角色是否参与对应模块的统计（被模块可见性关闭时不参与） */
    inRaid: boolean;
    inTrial: boolean;
    inBaizhan: boolean;
  }
  const rowSources: RowSource[] = [];
  safeAccounts.forEach(account => {
    if (account.disabled) return;
    const roles = Array.isArray(account.roles) ? account.roles : [];
    roles.forEach(role => {
      if (role.disabled) return;
      const inRaid = role.visibility?.raid !== false;
      // 试炼 / 百战页本身会过滤掉代清号角色，这里保持同一口径
      const inTrial = role.visibility?.trial !== false && !role.isClient;
      const inBaizhan = role.visibility?.baizhan !== false && !role.isClient;
      // 三个模块都被关掉的角色不出现在概览里
      if (!inRaid && !inTrial && !inBaizhan) return;
      rowSources.push({
        account,
        role,
        roleId: role.id,
        roleName: role.name,
        server: role.server,
        region: role.region,
        equipmentScore: role.equipmentScore,
        inRaid,
        inTrial,
        inBaizhan,
      });
    });
  });

  const cellsByRole = new Map<string, Record<string, CdCell>>();

  // 该玩法没有任何角色参与时，整列不展示（列头也不出现）
  const hasTrialParticipants = rowSources.some(source => source.inTrial);
  const hasBaizhanParticipants = rowSources.some(source => source.inBaizhan);

  uniqueColumnRaids.forEach(({ raid, source }) => {
    const weekRecords = safeRecords.filter(record => {
      if (!matchesRaidInstance(record, raid)) return false;
      const time = new Date(record.date).getTime();
      return time >= periodStart && time <= periodEnd;
    });
    const confirmedRecords = weekRecords.filter(isConfirmedRecord);
    const shouldShowClientRoles = shouldShowClientRoleInRaid(raid.playerCount, raid.version || '');
    const key = getRaidKey(raid);
    let clearedCount = 0;
    let totalCount = 0;

    rowSources.forEach(({ account, roleId, inRaid }) => {
      const isClientAccount = account.type === AccountType.CLIENT;
      const roleRecords = confirmedRecords.filter(record => record.roleId === roleId);
      const hasPendingRecord = weekRecords.some(
        record => record.roleId === roleId && record.source === 'auto' && record.status === 'pending',
      );

      let cell: CdCell;
      if (!inRaid || (isClientAccount && !shouldShowClientRoles)) {
        cell = { status: 'na', clearedCount: 0, totalCount: 0 };
      } else {
        const cooldownInfo = calculateCooldown(raid, roleRecords.map(record => ({
          date: record.date,
          bossIds: record.bossIds,
          bossId: record.bossId,
        })), now);
        const bossCooldowns = calculateBossCooldowns(raid, expandBossRecords(roleRecords), roleId, now);
        cell = resolveCellStatus(hasPendingRecord, !cooldownInfo.hasRecordInCurrentCycle, bossCooldowns);
        cell.bossCooldowns = bossCooldowns;
        totalCount += 1;
        if (cell.status === 'complete') clearedCount += 1;
      }

      const roleCells = cellsByRole.get(roleId) ?? {};
      roleCells[key] = cell;
      cellsByRole.set(roleId, roleCells);
    });

    columns.push({
      key,
      raid,
      raidName: raid.name,
      source,
      clearedCount,
      totalCount,
    });
  });

  // 4. 试炼 / 百战：按本周次数统计，口径与各自页面一致
  const trialCells = new Map<string, CdActivityCell>();
  const baizhanCells = new Map<string, CdActivityCell>();
  // 周期未就绪（加载中或无法确定）时不参与统计，避免把「还不知道」显示成「本周没打」
  const trialPeriodPending = trialProgressPeriod === undefined || trialProgressPeriod === null;

  rowSources.forEach(({ roleId, inTrial, inBaizhan }) => {
    if (!inTrial || trialPeriodPending) {
      trialCells.set(roleId, { status: 'na', clearedCount: 0, totalCount: TRIAL_WEEKLY_TARGET });
    } else {
      const progress = getTrialWeeklyDrawProgress(
        safeTrialRecords.filter(record => record.roleId === roleId),
        { weekStart: getLastMonday(now), progressPeriod: trialProgressPeriod },
      );
      trialCells.set(
        roleId,
        resolveActivityCell(progress.drawCount, TRIAL_WEEKLY_TARGET, progress.pendingRunCount > 0),
      );
    }

    if (!inBaizhan) {
      baizhanCells.set(roleId, { status: 'na', clearedCount: 0, totalCount: BAIZHAN_WEEKLY_TARGET });
    } else {
      const weeklyCount = safeBaizhanRecords.filter(record => (
        record.roleId === roleId && toTimestamp(record.date) >= periodStart
      )).length;
      baizhanCells.set(roleId, resolveActivityCell(weeklyCount, BAIZHAN_WEEKLY_TARGET, false));
    }
  });

  rowSources.forEach(({ account, role, roleId, roleName, server, region, equipmentScore }, index) => {
    const cells = cellsByRole.get(roleId) ?? {};
    let noneClearedCount = 0;
    let partialClearedCount = 0;
    let pendingClearedCount = 0;
    let completeClearedCount = 0;
    columns.forEach(column => {
      const status = cells[column.key]?.status;
      if (status === 'none') noneClearedCount += 1;
      else if (status === 'partial') partialClearedCount += 1;
      else if (status === 'pending') pendingClearedCount += 1;
      else if (status === 'complete') completeClearedCount += 1;
    });
    roleRows.push({
      roleId,
      roleName,
      server,
      region,
      sect: role.sect,
      martial: role.martial,
      accountId: account.id,
      accountName: account.accountName,
      accountType: account.type,
      equipmentScore,
      password: account.password,
      cells,
      trial: trialCells.get(roleId) ?? { status: 'na', clearedCount: 0, totalCount: TRIAL_WEEKLY_TARGET },
      baizhan: baizhanCells.get(roleId) ?? { status: 'na', clearedCount: 0, totalCount: BAIZHAN_WEEKLY_TARGET },
      noneClearedCount,
      partialClearedCount,
      pendingClearedCount,
      completeClearedCount,
      sourceIndex: index,
    });
  });

  // 行顺序：装分从高到低，未填装分的排最后；装分相同时欠 CD 多的角色排前面，
  // 再相同则保留账号与角色的原有顺序，便于对照账号管理页
  roleRows.sort((a, b) => {
    const scoreA = typeof a.equipmentScore === 'number' && a.equipmentScore > 0 ? a.equipmentScore : -1;
    const scoreB = typeof b.equipmentScore === 'number' && b.equipmentScore > 0 ? b.equipmentScore : -1;
    if (scoreB !== scoreA) return scoreB - scoreA;
    if (b.noneClearedCount !== a.noneClearedCount) return b.noneClearedCount - a.noneClearedCount;
    if (b.partialClearedCount !== a.partialClearedCount) return b.partialClearedCount - a.partialClearedCount;
    if (b.pendingClearedCount !== a.pendingClearedCount) return b.pendingClearedCount - a.pendingClearedCount;
    return a.sourceIndex - b.sourceIndex;
  });

  return {
    columns,
    rows: roleRows.map(({ sourceIndex: _sourceIndex, ...row }) => row),
    disabledColumnCount,
    hasTrialParticipants,
    hasBaizhanParticipants,
  };
}

/** 单元格四态判定：与 RaidDetail 的 getBossClearStatus 完全一致 */
function resolveCellStatus(
  hasPendingRecord: boolean,
  canRun: boolean,
  bossCooldowns: { hasRecord: boolean }[],
): CdCell {
  const totalCount = bossCooldowns.length;
  const clearedCount = bossCooldowns.filter(boss => boss.hasRecord).length;

  if (hasPendingRecord) return { status: 'pending', clearedCount, totalCount: totalCount };
  if (totalCount === 0) {
    return canRun
      ? { status: 'none', clearedCount: 0, totalCount: 0 }
      : { status: 'complete', clearedCount: 0, totalCount: 0 };
  }
  if (clearedCount === 0) return { status: 'none', clearedCount, totalCount };
  if (clearedCount === totalCount) return { status: 'complete', clearedCount, totalCount };
  return { status: 'partial', clearedCount, totalCount };
}

/**
 * 次数型进度（试炼翻牌次数、百战次数）的四态判定。
 *
 * 与副本列同一套语义：待确认优先 → 0 次未清 → 达到目标已清 → 其余部分清。
 * 目标次数固定（试炼 3 次、百战 1 次），所以 `总次数` 恒为 `totalCount`。
 */
function resolveActivityCell(
  clearedCount: number,
  totalCount: number,
  hasPending: boolean,
): CdActivityCell {
  if (hasPending) return { status: 'pending', clearedCount, totalCount };
  if (clearedCount <= 0) return { status: 'none', clearedCount: 0, totalCount };
  if (clearedCount >= totalCount) return { status: 'complete', clearedCount, totalCount };
  return { status: 'partial', clearedCount, totalCount };
}

/** 兼容 ISO 字符串与秒/毫秒时间戳 */
function toTimestamp(value: string | number): number {
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) return 0;
    // 秒级时间戳约 1e9~1e10，毫秒级约 1e12
    return Math.abs(value) < 100_000_000_000 ? value * 1000 : value;
  }
  const parsed = new Date(value).getTime();
  return Number.isFinite(parsed) ? parsed : 0;
}
