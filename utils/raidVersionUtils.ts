export interface RaidCheckResult {
  isLatestVersion25Raid: boolean;
  shouldShowClientRoles: boolean;
}

export function checkRaidForClientRoles(
  _raidName: string,
  playerCount: number,
  _version: string
): RaidCheckResult {
  // 原本判断是否是最新版本，既然已经没法确切维护最新版本列表
  // 我们默认 25 人本都显示老板号，或者根据实际业务保留此接口
  const is25Raid = playerCount === 25;

  return {
    isLatestVersion25Raid: is25Raid,
    shouldShowClientRoles: is25Raid
  };
}

export function isClientRoleVisible(
  accountType: 'OWN' | 'CLIENT' | undefined,
  playerCount: number,
  version: string
): boolean {
  if (accountType !== 'CLIENT') {
    return true;
  }

  return checkRaidForClientRoles('', playerCount, version).shouldShowClientRoles;
}

export function shouldShowClientRoleInRaid(
  playerCount: number,
  _version: string
): boolean {
  return playerCount === 25;
}

export function filterRolesForRaid<T extends { id: string; name: string }>(
  roles: T[],
  playerCount: number,
  version: string,
  getAccountType: (role: T) => 'OWN' | 'CLIENT' | undefined
): T[] {
  const shouldShowClient = shouldShowClientRoleInRaid(playerCount, version);

  if (shouldShowClient) {
    return roles;
  }

  return roles.filter(role => getAccountType(role) !== 'CLIENT');
}
