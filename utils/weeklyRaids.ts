import { Raid } from '../types';

/** 本周卡片展示的难度范围：本赛季副本展示全部难度，历史赛季副本只展示 10 人难度 */
export type WeeklyRaidScope = 'current' | 'history';

export interface WeeklyRaidGroup {
  name: string;
  version: string;
  /** 实际需要展示的难度列表 */
  raids: Raid[];
  /** 是否同时被收藏：收藏的周常副本优先展示在本周团队秘境区域 */
  isFavorite: boolean;
  /** 难度范围，用于决定卡片副标题与展示口径 */
  scope: WeeklyRaidScope;
  /** 历史赛季副本本地缺少 10 人配置，已回退为展示现有难度 */
  missingTenPlayerConfig: boolean;
  /** 本地完全没有该副本的任何难度配置 */
  missingConfig: boolean;
}

/** 判断副本赛季归属所需的上下文 */
export interface SeasonContext {
  /** 当前赛季 id（seasons.id） */
  currentSeasonId: number | null;
  /** 当前赛季所属大版本名（game_versions.name） */
  currentVersionName: string | null;
}

const PLAYER_COUNT_ORDER: Record<number, number> = { 10: 0, 25: 1 };
const DIFFICULTY_ORDER: Record<string, number> = { '普通': 0, '英雄': 1, '挑战': 2 };

/** 历史赛季副本只展示 10 人难度 */
export const HISTORY_SEASON_PLAYER_COUNT = 10;

const compareRaids = (a: Raid, b: Raid): number => {
  const playerCountDiff = (PLAYER_COUNT_ORDER[a.playerCount] ?? 99) - (PLAYER_COUNT_ORDER[b.playerCount] ?? 99);
  if (playerCountDiff !== 0) return playerCountDiff;
  return (DIFFICULTY_ORDER[a.difficulty] ?? 99) - (DIFFICULTY_ORDER[b.difficulty] ?? 99);
};

/** 清洗本周名单：去空白、去空名、同名去重，保留接口返回顺序。 */
export function normalizeWeeklyRaidNames(weeklyNames: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const rawName of weeklyNames) {
    const name = rawName.trim();
    if (!name || seen.has(name)) continue;
    seen.add(name);
    result.push(name);
  }
  return result;
}

/**
 * 判断副本是否属于本赛季。
 *
 * 分两级回退，缺一不可：
 * 1. **优先按赛季 id**：一个大版本下有多个赛季（如「丝路风雨」含丝路风语、太极秘录、
 *    山海源流、暗影千机），用版本名比较会把同版本内的旧赛季副本误判为本赛季。
 * 2. **再按大版本名兜底**：早期预置副本只写了 `version`、库里 `season_id` 为 NULL
 *    （例如敖龙岛的 version 是「世外蓬莱」，而 static_seasons.json 没有该版本的赛季记录）。
 *    若只认赛季 id，这类副本会因「赛季未知」被误判成本赛季并展开全部难度。
 *
 * 任一级可判定即返回结果；两级都无法判定时返回 `null`，由调用方决定降级策略。
 */
export function isCurrentSeasonRaid(raid: Raid, context: SeasonContext): boolean | null {
  if (Number.isInteger(raid.seasonId) && Number.isInteger(context.currentSeasonId)) {
    return raid.seasonId === context.currentSeasonId;
  }
  if (raid.version && context.currentVersionName) {
    return raid.version === context.currentVersionName;
  }
  return null;
}

/**
 * 组装本周团队秘境卡片。
 * - 本赛季副本：展示本地配置里的全部难度（10 人 / 25 人 × 普通 / 英雄 / 挑战）。
 * - 确认属于历史赛季的副本：只展示 10 人难度；本地没有 10 人配置时回退展示现有难度并标记。
 * - 完全无法判定赛季归属时按全部难度展示，避免误伤本赛季副本。
 */
export function buildWeeklyRaidGroups(
  raids: Raid[],
  weeklyNames: string[],
  favoriteNames: ReadonlySet<string>,
  seasonContext: SeasonContext,
): WeeklyRaidGroup[] {
  const groups: WeeklyRaidGroup[] = [];
  for (const name of normalizeWeeklyRaidNames(weeklyNames)) {
    const allMatches = raids.filter(raid => raid.name === name);

    // 同名副本的各难度共享赛季归属；任一难度判定为历史即按历史处理。
    const isHistory = allMatches.some(raid => isCurrentSeasonRaid(raid, seasonContext) === false);

    let visibleRaids = allMatches;
    let missingTenPlayerConfig = false;
    if (isHistory) {
      const tenPlayerRaids = allMatches.filter(raid => raid.playerCount === HISTORY_SEASON_PLAYER_COUNT);
      if (tenPlayerRaids.length > 0) {
        visibleRaids = tenPlayerRaids;
      } else {
        missingTenPlayerConfig = true;
      }
    }

    groups.push({
      name,
      version: allMatches[0]?.version || '其他',
      raids: [...visibleRaids].sort(compareRaids),
      isFavorite: favoriteNames.has(name),
      scope: isHistory ? 'history' : 'current',
      missingTenPlayerConfig,
      missingConfig: allMatches.length === 0,
    });
  }
  // 已收藏的周常副本优先展示，其余保持接口返回顺序
  return groups.sort((a, b) => Number(b.isFavorite) - Number(a.isFavorite));
}
