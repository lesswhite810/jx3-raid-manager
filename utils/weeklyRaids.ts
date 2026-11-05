import { Raid } from '../types';

export interface WeeklyRaidGroup {
  name: string;
  version: string;
  raids: Raid[];
  missingTenPlayerConfig: boolean;
}

export function buildWeeklyRaidGroups(
  raids: Raid[],
  weeklyNames: string[],
  favoriteNames: ReadonlySet<string>,
): WeeklyRaidGroup[] {
  const seen = new Set<string>();
  const groups: WeeklyRaidGroup[] = [];
  for (const rawName of weeklyNames) {
    const name = rawName.trim();
    if (!name || seen.has(name) || favoriteNames.has(name)) continue;
    seen.add(name);
    const matches = raids.filter(raid => raid.name === name && raid.playerCount === 10);
    groups.push({
      name,
      version: matches[0]?.version || '其他',
      raids: [...matches].sort((a, b) => Number(b.isActive) - Number(a.isActive)),
      missingTenPlayerConfig: matches.length === 0,
    });
  }
  return groups;
}
