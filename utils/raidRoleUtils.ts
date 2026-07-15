import { AccountType } from '../types';

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
