import { AccountType } from '../types';

/**
 * 单副本级别的角色禁用配置：raidKey（形如 `25人普通洛阳之战`）→ 该副本里被禁用的角色 ID。
 *
 * 只保存被禁用（visible === false）的条目：没有条目即表示默认可见，
 * 与详情页 `roleVisibilityMap[roleId] !== false` 的判定口径一致。
 */
export type RaidRoleVisibilityMap = ReadonlyMap<string, ReadonlySet<string>>;

/** 把 `raid_role_visibility` 的查询结果转成按副本分组的禁用角色集合 */
export const buildRaidRoleVisibilityMap = (
  entries: ReadonlyArray<{ roleId: string; raidKey: string; visible: boolean }>,
): Map<string, Set<string>> => {
  const map = new Map<string, Set<string>>();
  entries.forEach(entry => {
    if (entry.visible) return;
    const roleIds = map.get(entry.raidKey) ?? new Set<string>();
    roleIds.add(entry.roleId);
    map.set(entry.raidKey, roleIds);
  });
  return map;
};

/** 判断某角色是否在指定副本里被禁用（未配置时视为可见） */
export const isRaidRoleDisabled = (
  map: RaidRoleVisibilityMap | undefined,
  raidKey: string,
  roleId: string,
): boolean => map?.get(raidKey)?.has(roleId) ?? false;

export interface SearchableRaidRole {
  id: string;
  name: string;
  server: string;
  region: string;
  sect?: string;
  martial?: string;
  equipmentScore?: number;
  accountName: string;
  accountNote?: string;
}

export interface RaidClearStatsRole {
  id: string;
  canRun: boolean;
  hasPendingRecord?: boolean;
  bossCooldowns?: Array<{
    hasRecord: boolean;
  }>;
}

export interface RaidClearStats {
  noneClearedCount: number;
  partialClearedCount: number;
  pendingClearedCount: number;
  completeClearedCount: number;
}

export const getClientAccountNote = (
  accountType: AccountType | string | undefined,
  note?: string
): string | undefined => {
  if (accountType !== AccountType.CLIENT) {
    return undefined;
  }

  const trimmedNote = note?.trim();
  return trimmedNote ? trimmedNote : undefined;
};

export const filterRaidRoles = <T extends SearchableRaidRole>(roles: T[], searchTerm: string): T[] => {
  const normalizedSearch = searchTerm.trim().toLowerCase();
  if (!normalizedSearch) {
    return roles;
  }

  return roles.filter(role => {
    return [
      role.name,
      role.server,
      role.region,
      role.sect,
      role.accountName,
      role.accountNote
    ].some(field => field?.toLowerCase().includes(normalizedSearch));
  });
};

export const getRaidClearStats = <T extends RaidClearStatsRole>(
  roles: T[],
  roleVisibilityMap: Record<string, boolean>
): RaidClearStats => {
  const enabledRoles = roles.filter(role => roleVisibilityMap[role.id] !== false);

  // 有待确认记录的角色优先归为"待确认"
  const pendingRoles = enabledRoles.filter(role => role.hasPendingRecord);
  const nonPendingRoles = enabledRoles.filter(role => !role.hasPendingRecord);

  const noneClearedCount = nonPendingRoles.filter(role => {
    if (!role.bossCooldowns || role.bossCooldowns.length === 0) {
      return role.canRun;
    }

    return role.bossCooldowns.filter(boss => boss.hasRecord).length === 0;
  }).length;

  const partialClearedCount = nonPendingRoles.filter(role => {
    if (!role.bossCooldowns || role.bossCooldowns.length === 0) {
      return false;
    }

    const completedCount = role.bossCooldowns.filter(boss => boss.hasRecord).length;
    return completedCount > 0 && completedCount < role.bossCooldowns.length;
  }).length;

  const fullyClearedCount = nonPendingRoles.filter(role => {
    if (!role.bossCooldowns || role.bossCooldowns.length === 0) {
      return !role.canRun;
    }

    return role.bossCooldowns.filter(boss => boss.hasRecord).length === role.bossCooldowns.length;
  }).length;

  return {
    noneClearedCount,
    partialClearedCount,
    pendingClearedCount: pendingRoles.length,
    completeClearedCount: fullyClearedCount
  };
};
