/**
 * bossCooldownManager 单元测试
 *
 * 10人本每周刷新两次（周一 07:00、周五 07:00），BOSS CD 必须跟着这两个刷新点走，
 * 否则周五刷新后卡片仍显示为已打完。
 */

import { describe, expect, it } from 'vitest';
import { BossRecord, Raid } from '../types';
import { injectDefaultBosses } from '../data/raidBosses';
import { calculateBossCooldowns, getRaidOverallCooldownStatus } from './bossCooldownManager';

const BOSS_IDS = [
  'luoyangzhizhan_1',
  'luoyangzhizhan_2',
  'luoyangzhizhan_3',
  'luoyangzhizhan_4',
  'luoyangzhizhan_5',
];

const BOSS_NAMES = ['突利和顺', '田承嗣', '伊曼·寂夜', '阿史那承庆', '史朝义'];

function makeRaid(playerCount: 10 | 25): Raid {
  return injectDefaultBosses({
    name: '洛阳之战',
    version: '苍生铸世',
    playerCount,
    difficulty: '普通',
    isActive: true,
  });
}

/** 一条记录了全部 5 个 BOSS 的副本记录 */
function makeFullRecord(date: Date, status: BossRecord['status'] = 'confirmed'): BossRecord {
  return {
    id: `record-${date.getTime()}`,
    raidRecordId: `record-${date.getTime()}`,
    accountId: 'account',
    roleId: 'role',
    bossId: BOSS_IDS[0],
    bossName: BOSS_NAMES[0],
    bossIds: BOSS_IDS,
    bossNames: BOSS_NAMES,
    date: date.getTime(),
    status,
  };
}

describe('calculateBossCooldowns - 10人本半周刷新', () => {
  it('周四记录在周五 07:00 刷新后不再计入本周 BOSS CD', () => {
    const raid = makeRaid(10);
    // 2026-11-05 周四 23:13 打完全通
    const record = makeFullRecord(new Date(2026, 10, 5, 23, 13));
    // 2026-11-07 周六 17:00，已过周五 07:00 刷新点
    const now = new Date(2026, 10, 7, 17, 0);

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);

    expect(cooldowns).toHaveLength(5);
    expect(cooldowns.every(boss => !boss.hasRecord)).toBe(true);
    expect(cooldowns.every(boss => boss.canAddRecord)).toBe(true);
    expect(getRaidOverallCooldownStatus(cooldowns)).toEqual({
      allInCooldown: false,
      completedCount: 0,
      totalCount: 5,
    });
  });

  it('周四记录在周五 07:00 刷新前仍计入本周 BOSS CD', () => {
    const raid = makeRaid(10);
    const record = makeFullRecord(new Date(2026, 10, 5, 23, 13));
    // 2026-11-06 周五 06:59，还差一分钟刷新
    const now = new Date(2026, 10, 6, 6, 59);

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);

    expect(cooldowns.every(boss => boss.hasRecord)).toBe(true);
    expect(cooldowns.every(boss => boss.canAddRecord)).toBe(false);
    expect(getRaidOverallCooldownStatus(cooldowns)).toEqual({
      allInCooldown: true,
      completedCount: 5,
      totalCount: 5,
    });
  });

  it('周五 07:00 整点即视为新 CD 周期', () => {
    const raid = makeRaid(10);
    const record = makeFullRecord(new Date(2026, 10, 5, 23, 13));
    const now = new Date(2026, 10, 6, 7, 0);

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);

    expect(cooldowns.every(boss => !boss.hasRecord)).toBe(true);
  });

  it('周一下半周（周五之后）的记录计入当前周期', () => {
    const raid = makeRaid(10);
    // 2026-11-06 周五 20:00
    const record = makeFullRecord(new Date(2026, 10, 6, 20, 0));
    // 2026-11-08 周日 20:00，仍在下半周周期内
    const now = new Date(2026, 10, 8, 20, 0);

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);

    expect(cooldowns.every(boss => boss.hasRecord)).toBe(true);
  });

  it('周一 07:00 刷新后，上周五的记录不再计入', () => {
    const raid = makeRaid(10);
    const record = makeFullRecord(new Date(2026, 10, 6, 20, 0));
    // 2026-11-09 周一 07:01
    const now = new Date(2026, 10, 9, 7, 1);

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);

    expect(cooldowns.every(boss => !boss.hasRecord)).toBe(true);
  });

  it('已拒绝的记录不参与 BOSS CD 计算', () => {
    const raid = makeRaid(10);
    const record = makeFullRecord(new Date(2026, 10, 7, 12, 0), 'rejected');
    const now = new Date(2026, 10, 7, 17, 0);

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);

    expect(cooldowns.every(boss => !boss.hasRecord)).toBe(true);
  });
});

describe('calculateBossCooldowns - 25人本整周刷新', () => {
  it('周四记录在周六仍计入本周 BOSS CD', () => {
    const raid = makeRaid(25);
    const record = makeFullRecord(new Date(2026, 10, 5, 23, 13));
    const now = new Date(2026, 10, 7, 17, 0);

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);

    expect(cooldowns.every(boss => boss.hasRecord)).toBe(true);
    expect(getRaidOverallCooldownStatus(cooldowns)).toEqual({
      allInCooldown: true,
      completedCount: 5,
      totalCount: 5,
    });
  });

  it('周一 07:00 刷新后上周记录不再计入', () => {
    const raid = makeRaid(25);
    const record = makeFullRecord(new Date(2026, 10, 5, 23, 13));
    const now = new Date(2026, 10, 9, 7, 1);

    const cooldowns = calculateBossCooldowns(raid, [record], 'role', now);

    expect(cooldowns.every(boss => !boss.hasRecord)).toBe(true);
  });
});
