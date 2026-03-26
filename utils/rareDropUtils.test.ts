import { describe, expect, it } from 'vitest';
import { RaidRecord } from '../types';
import { buildSpecialDropRecords } from './rareDropUtils';

describe('buildSpecialDropRecords', () => {
  it('会展开一条记录中的全部特殊掉落类型', () => {
    const records = [
      {
        id: 'raid-1',
        accountId: 'account-1',
        roleId: 'role-1',
        raidName: '太极宫',
        date: '2026-03-26T10:00:00.000Z',
        goldIncome: 1000,
        hasXuanjing: true,
        hasPet: true,
      },
      {
        id: 'raid-2',
        accountId: 'account-2',
        roleId: '',
        raidName: '河阳之战',
        date: '2026-03-26T11:00:00.000Z',
        goldIncome: 800,
        hasXuanjing: false,
        hasMount: true,
      },
    ] satisfies RaidRecord[];

    expect(buildSpecialDropRecords(records, null)).toEqual([
      expect.objectContaining({
        id: 'raid-1-hasXuanjing',
        type: '玄晶',
        roleId: 'role-1',
      }),
      expect.objectContaining({
        id: 'raid-1-hasPet',
        type: '宠物',
        roleId: 'role-1',
      }),
      expect.objectContaining({
        id: 'raid-2-hasMount',
        type: '坐骑',
        roleId: 'account-2',
      }),
    ]);
  });

  it('会按时间范围过滤特殊掉落记录', () => {
    const records = [
      {
        id: 'old-record',
        accountId: 'account-1',
        roleId: 'role-1',
        raidName: '旧副本',
        date: '2026-03-01T10:00:00.000Z',
        goldIncome: 100,
        hasXuanjing: false,
        hasAppearance: true,
      },
      {
        id: 'new-record',
        accountId: 'account-1',
        roleId: 'role-1',
        raidName: '新副本',
        date: '2026-03-20T10:00:00.000Z',
        goldIncome: 200,
        hasXuanjing: true,
      },
    ] satisfies RaidRecord[];

    const periodStartTime = new Date('2026-03-15T00:00:00.000Z').getTime();

    expect(buildSpecialDropRecords(records, periodStartTime)).toEqual([
      expect.objectContaining({
        id: 'new-record-hasXuanjing',
        type: '玄晶',
      }),
    ]);
  });
});
