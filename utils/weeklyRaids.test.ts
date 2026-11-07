import { describe, expect, it } from 'vitest';
import { Raid } from '../types';
import { buildWeeklyRaidGroups, isCurrentSeasonRaid, normalizeWeeklyRaidNames, SeasonContext } from './weeklyRaids';

/** 当前赛季：苍生铸世（id=5，所属大版本「苍生铸世」） */
const CURRENT_SEASON: SeasonContext = { currentSeasonId: 5, currentVersionName: '苍生铸世' };

const raid = (
  name: string,
  playerCount: 10 | 25,
  difficulty: '普通' | '英雄' | '挑战' = '普通',
  extra: Partial<Raid> = {},
): Raid => ({ name, playerCount, difficulty, isActive: true, ...extra });

describe('isCurrentSeasonRaid - 第一级：按赛季 id 判定', () => {
  it('赛季 id 相同即本赛季', () => {
    expect(isCurrentSeasonRaid(raid('洛阳之战', 10, '普通', { seasonId: 5, version: '苍生铸世' }), CURRENT_SEASON)).toBe(true);
  });

  it('同一大版本内的旧赛季按赛季 id 判为历史', () => {
    // 山海源流(id=3)、暗影千机(id=4) 与当前赛季同属「丝路风雨」/「苍生铸世」记录，id 不同即历史
    expect(isCurrentSeasonRaid(raid('会战弓月城', 10, '普通', { seasonId: 3, version: '丝路风雨' }), CURRENT_SEASON)).toBe(false);
    expect(isCurrentSeasonRaid(raid('阆风悬城', 10, '普通', { seasonId: 4, version: '丝路风雨' }), CURRENT_SEASON)).toBe(false);
  });
});

describe('isCurrentSeasonRaid - 第二级：season_id 缺失时按大版本兜底', () => {
  it('敖龙岛：预置数据无 season_id，version 世外蓬莱 ≠ 当前版本，判为历史', () => {
    // 回归用例：static_seasons.json 没有「世外蓬莱」的赛季记录，
    // 该副本入库时 season_id 为 NULL，必须靠 version 兜底才能识别为历史赛季。
    const aolongdao = raid('敖龙岛', 10, '普通', { version: '世外蓬莱' });
    expect(aolongdao.seasonId).toBeUndefined();
    expect(isCurrentSeasonRaid(aolongdao, CURRENT_SEASON)).toBe(false);
  });

  it('season_id 缺失但 version 与当前大版本一致，判为本赛季', () => {
    expect(isCurrentSeasonRaid(raid('洛阳之战', 10, '普通', { version: '苍生铸世' }), CURRENT_SEASON)).toBe(true);
  });

  it('同版本内的赛季即使没有 season_id 也会被判为本赛季（已知局限，需靠 id 兜底）', () => {
    // 这正是第一级存在的理由：有 season_id 时优先按 id，不会被 version 放宽。
    const withId = raid('阆风悬城', 10, '普通', { seasonId: 4, version: '丝路风雨' });
    expect(isCurrentSeasonRaid(withId, CURRENT_SEASON)).toBe(false);
  });
});

describe('isCurrentSeasonRaid - 无法判定', () => {
  it('两侧信息都不足时返回 null', () => {
    const emptyContext: SeasonContext = { currentSeasonId: null, currentVersionName: null };
    expect(isCurrentSeasonRaid(raid('敖龙岛', 10, '普通', { version: '世外蓬莱' }), emptyContext)).toBeNull();
    expect(isCurrentSeasonRaid(raid('敖龙岛', 10, '普通', {}), CURRENT_SEASON)).toBeNull();
  });
});

describe('buildWeeklyRaidGroups - 本赛季副本展示全部难度', () => {
  it('本赛季副本保留全部难度并按人数、难度排序', () => {
    const result = buildWeeklyRaidGroups(
      [
        raid('洛阳之战', 25, '英雄', { seasonId: 5 }),
        raid('洛阳之战', 10, '普通', { seasonId: 5 }),
        raid('洛阳之战', 25, '普通', { seasonId: 5 }),
      ],
      ['洛阳之战'],
      new Set(),
      CURRENT_SEASON,
    );
    expect(result[0].scope).toBe('current');
    expect(result[0].raids.map(r => `${r.playerCount}人${r.difficulty}`)).toEqual(['10人普通', '25人普通', '25人英雄']);
  });
});

describe('buildWeeklyRaidGroups - 历史赛季副本只展示 10 人难度', () => {
  it('敖龙岛（无 season_id、version 为世外蓬莱）只展示 10 人普通', () => {
    // 用户实测回归：当前赛季苍生铸世时，敖龙岛曾被错误展开为三个难度。
    const result = buildWeeklyRaidGroups(
      [
        raid('敖龙岛', 10, '普通', { version: '世外蓬莱' }),
        raid('敖龙岛', 25, '普通', { version: '世外蓬莱' }),
        raid('敖龙岛', 25, '英雄', { version: '世外蓬莱' }),
      ],
      ['敖龙岛'],
      new Set(),
      CURRENT_SEASON,
    );
    expect(result[0].scope).toBe('history');
    expect(result[0].raids.map(r => `${r.playerCount}人${r.difficulty}`)).toEqual(['10人普通']);
  });

  it('有赛季 id 的历史赛季副本同样只展示 10 人难度', () => {
    const result = buildWeeklyRaidGroups(
      [
        raid('阆风悬城', 10, '普通', { seasonId: 4, version: '丝路风雨' }),
        raid('阆风悬城', 25, '普通', { seasonId: 4, version: '丝路风雨' }),
        raid('阆风悬城', 25, '英雄', { seasonId: 4, version: '丝路风雨' }),
      ],
      ['阆风悬城'],
      new Set(),
      CURRENT_SEASON,
    );
    expect(result[0].scope).toBe('history');
    expect(result[0].raids.map(r => `${r.playerCount}人${r.difficulty}`)).toEqual(['10人普通']);
  });

  it('历史赛季副本缺少 10 人配置时回退展示现有难度并标记', () => {
    const result = buildWeeklyRaidGroups(
      [raid('缚罪之渊', 25, '挑战', { seasonId: 4, version: '丝路风雨' })],
      ['缚罪之渊'],
      new Set(),
      CURRENT_SEASON,
    );
    expect(result[0].scope).toBe('history');
    expect(result[0].missingTenPlayerConfig).toBe(true);
    expect(result[0].raids).toHaveLength(1);
  });
});

describe('buildWeeklyRaidGroups - 收藏与异常路径', () => {
  it('已收藏的周常副本仍然出现在本周名单中，并排在最前面', () => {
    const result = buildWeeklyRaidGroups(
      [raid('洛阳之战', 10, '普通', { seasonId: 5 }), raid('敖龙岛', 10, '普通', { version: '世外蓬莱' })],
      ['洛阳之战', '敖龙岛'],
      new Set(['洛阳之战']),
      CURRENT_SEASON,
    );
    expect(result.map(group => group.name)).toEqual(['洛阳之战', '敖龙岛']);
    expect(result[0].isFavorite).toBe(true);
    expect(result[1].isFavorite).toBe(false);
  });

  it('本地没有该副本任何配置时标记 missingConfig', () => {
    const result = buildWeeklyRaidGroups(
      [raid('洛阳之战', 10, '普通', { seasonId: 5 })],
      ['不存在的副本'],
      new Set(),
      CURRENT_SEASON,
    );
    expect(result[0].missingConfig).toBe(true);
    expect(result[0].raids).toEqual([]);
  });

  it('忽略空名称和空列表', () => {
    expect(buildWeeklyRaidGroups([], ['', '  '], new Set(), CURRENT_SEASON)).toEqual([]);
    expect(buildWeeklyRaidGroups([raid('敖龙岛', 10, '普通', {})], [], new Set(), CURRENT_SEASON)).toEqual([]);
  });
});

describe('normalizeWeeklyRaidNames', () => {
  it('去空白、去空名并保留首次出现的顺序', () => {
    expect(normalizeWeeklyRaidNames([' 洛阳之战 ', '', '敖龙岛', '洛阳之战'])).toEqual(['洛阳之战', '敖龙岛']);
  });
});
