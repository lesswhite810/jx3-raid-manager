import { describe, expect, it } from 'vitest';
import { BossRecord, Raid } from '../types';
import { calculateBossCooldowns, getRaidOverallCooldownStatus } from '../utils/bossCooldownManager';
import staticRaids from '../src-tauri/src/db/static_raids.json';
import { getDefaultBosses, injectDefaultBosses, hasBossTracking } from './raidBosses';

const LUOYANG_BOSS_NAMES = ['突利和顺', '田承嗣', '伊曼·寂夜', '阿史那承庆', '史朝义'];

const createLuoyangRaid = (): Raid => injectDefaultBosses({
  name: '洛阳之战',
  version: '苍生铸世',
  playerCount: 25,
  difficulty: '英雄',
  isActive: true,
});

describe('洛阳之战 BOSS 配置与 CD', () => {
  it('三个难度的后端预置与前端默认 BOSS 使用同一组 ID，不将逐焰单独计数', () => {
    const preset = staticRaids.find(raid => raid.name === '洛阳之战');
    const bosses = getDefaultBosses('洛阳之战');

    expect(preset).toBeDefined();
    expect(preset?.level).toBe(50);
    expect(preset?.version).toBe('苍生铸世');
    expect(preset?.configurations).toEqual([
      { playerCount: 10, difficulty: '普通', isActive: true },
      { playerCount: 25, difficulty: '普通', isActive: true },
      { playerCount: 25, difficulty: '英雄', isActive: true },
    ]);
    expect(preset?.bosses).toEqual(bosses);
    expect(bosses?.map(boss => boss.name)).toEqual(LUOYANG_BOSS_NAMES);
    expect(hasBossTracking(createLuoyangRaid())).toBe(true);
  });

  it('5 个正式 BOSS 都完成时全通，不需要额外完成一个分身 CD', () => {
    const raid = createLuoyangRaid();
    const now = new Date(2026, 10, 3, 12);
    const record: BossRecord = {
      id: 'luoyang-record',
      raidRecordId: 'luoyang-record',
      accountId: 'account',
      roleId: 'role',
      bossId: 'luoyangzhizhan_1',
      bossName: '突利和顺',
      bossIds: raid.bosses?.map(boss => boss.id),
      bossNames: LUOYANG_BOSS_NAMES,
      date: now.getTime(),
      status: 'confirmed',
    };

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);
    expect(getRaidOverallCooldownStatus(cooldowns)).toEqual({
      allInCooldown: true,
      completedCount: 5,
      totalCount: 5,
    });
  });

  it('缺少寂夜时仍未全通，保留用户已配置的 BOSS', () => {
    const raid = createLuoyangRaid();
    const now = new Date(2026, 10, 3, 12);
    const record: BossRecord = {
      id: 'partial-record',
      raidRecordId: 'partial-record',
      accountId: 'account',
      roleId: 'role',
      bossId: 'luoyangzhizhan_1',
      bossName: '突利和顺',
      bossIds: ['luoyangzhizhan_1', 'luoyangzhizhan_2', 'luoyangzhizhan_4', 'luoyangzhizhan_5'],
      date: now.getTime(),
      status: 'confirmed',
    };

    expect(getRaidOverallCooldownStatus(calculateBossCooldowns(raid, [record], 'role', now))).toEqual({
      allInCooldown: false,
      completedCount: 4,
      totalCount: 5,
    });

    const customRaid: Raid = { ...raid, bosses: [{ id: 'custom', name: '自定义首领', order: 1 }] };
    expect(injectDefaultBosses(customRaid)).toBe(customRaid);
  });
});
