import { describe, expect, it } from 'vitest';
import { Raid } from '../types';
import { buildWeeklyRaidGroups } from './weeklyRaids';

const raid = (name: string, playerCount: 10 | 25): Raid => ({ name, playerCount, difficulty: '普通', version: '苍生铸世', isActive: true });

describe('buildWeeklyRaidGroups', () => {
  it('只保留精确匹配的10人配置并去重名称', () => {
    const result = buildWeeklyRaidGroups([raid('洛阳之战', 10), raid('洛阳之战', 25), raid('敖龙岛', 10)], ['洛阳之战', '洛阳之战', '敖龙岛', '不存在的副本'], new Set());
    expect(result.map(group => group.name)).toEqual(['洛阳之战', '敖龙岛', '不存在的副本']);
    expect(result[0].raids).toHaveLength(1);
    expect(result[2].missingTenPlayerConfig).toBe(true);
  });
  it('已收藏的副本不作为临时周常推荐显示', () => {
    const result = buildWeeklyRaidGroups([raid('洛阳之战', 10), raid('敖龙岛', 10)], ['洛阳之战', '敖龙岛'], new Set(['洛阳之战']));
    expect(result.map(group => group.name)).toEqual(['敖龙岛']);
  });
  it('忽略空名称和空列表', () => {
    expect(buildWeeklyRaidGroups([raid('敖龙岛', 10)], ['', '  '], new Set())).toEqual([]);
    expect(buildWeeklyRaidGroups([raid('敖龙岛', 10)], [], new Set())).toEqual([]);
  });
});
