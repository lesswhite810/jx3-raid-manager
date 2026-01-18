import { StaticRaid } from '../data/staticRaids';

export const GAME_VERSIONS = [
  '丝路风雨',   // 最新版本 130级
  '横刀断浪',   // 120级
  '奉天证道',   // 110级
  '世外蓬莱',   // 100级
  '重制版',     // 95级
  '风骨霸刀',   // 95级
  '剑胆琴心',   // 95级
  '安史之乱',   // 90级
  '巴蜀风云',   // 80级
  '风起稻香',   // 70级
] as const;

export type GameVersion = typeof GAME_VERSIONS[number];

export const LATEST_VERSION: GameVersion = '丝路风雨';

export function isLatestVersion(version: string): boolean {
  return version === LATEST_VERSION;
}

export function getVersionIndex(version: string): number {
  return GAME_VERSIONS.indexOf(version as GameVersion);
}

export function isNewerVersion(version1: string, version2: string): boolean {
  return getVersionIndex(version1) < getVersionIndex(version2);
}

export function isOlderVersion(version1: string, version2: string): boolean {
  return getVersionIndex(version1) > getVersionIndex(version2);
}

export interface RaidCheckResult {
  isLatestVersion25Raid: boolean;
  shouldShowClientRoles: boolean;
}

export function checkRaidForClientRoles(
  _raidName: string, 
  playerCount: number, 
  version: string, 
  _staticRaids: StaticRaid[]
): RaidCheckResult {
  const isLatest = isLatestVersion(version);
  const is25Raid = playerCount === 25;
  
  return {
    isLatestVersion25Raid: isLatest && is25Raid,
    shouldShowClientRoles: isLatest && is25Raid
  };
}

export function isClientRoleVisible(
  accountType: 'OWN' | 'CLIENT' | undefined,
  playerCount: number,
  version: string,
  _staticRaids: StaticRaid[]
): boolean {
  if (accountType !== 'CLIENT') {
    return true;
  }
  
  return checkRaidForClientRoles('', playerCount, version, []).shouldShowClientRoles;
}

export function shouldShowClientRoleInRaid(
  playerCount: number,
  version: string,
  _staticRaids: StaticRaid[]
): boolean {
  const isLatest = isLatestVersion(version);
  const is25Raid = playerCount === 25;
  
  return isLatest && is25Raid;
}

export function filterRolesForRaid<T extends { id: string; name: string }>(
  roles: T[],
  playerCount: number,
  version: string,
  staticRaids: StaticRaid[],
  getAccountType: (role: T) => 'OWN' | 'CLIENT' | undefined
): T[] {
  const shouldShowClient = shouldShowClientRoleInRaid(playerCount, version, staticRaids);
  
  if (shouldShowClient) {
    return roles;
  }
  
  return roles.filter(role => getAccountType(role) !== 'CLIENT');
}

export function getVersionDisplayName(version: string): string {
  const index = getVersionIndex(version);
  if (index === -1) return version;
  return `${version} (第${GAME_VERSIONS.length - index}版)`;
}
