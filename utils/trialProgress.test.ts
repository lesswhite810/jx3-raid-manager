import { describe, expect, it } from 'vitest';
import { GameVersion, Season, TrialPlaceRecord } from '../types';
import {
  getLatestTrialRecordForRole,
  getTrialLayerForRole,
  getTrialProgressPeriod,
  isTrialRecordInProgressPeriod,
  toTrialProgressTimestamp,
} from './trialProgress';

const versions: GameVersion[] = [
  { id: 1, name: '旧版本', sortOrder: 1 },
  { id: 2, name: '当前版本', sortOrder: 2 },
  { id: 3, name: '后续版本', sortOrder: 3 },
];

const seasons: Season[] = [
  { id: 11, name: '旧赛季', versionId: 1, startDate: 1_700_000_000, sortOrder: 1 },
  { id: 21, name: '当前第一赛季', versionId: 2, startDate: 1_710_000_000, sortOrder: 2 },
  { id: 22, name: '当前第二赛季', versionId: 2, startDate: 1_720_000_000, sortOrder: 3 },
  { id: 31, name: '后续第一赛季', versionId: 3, startDate: 1_730_000_000, sortOrder: 4 },
];

const record = (overrides: Partial<TrialPlaceRecord> = {}): TrialPlaceRecord => ({
  id: 'record',
  accountId: 'account',
  roleId: 'role-a',
  roleName: '角色',
  server: '梦江南',
  date: 1_710_000_000_000,
  layer: 12,
  bosses: ['甲', '乙', '丙'],
  card1: '',
  card2: '',
  card3: '',
  card4: '',
  card5: '',
  flippedIndex: 1,
  type: 'trial',
  ...overrides,
});

describe('试炼大版本进度', () => {
  const period = getTrialProgressPeriod(seasons[1], seasons, versions);

  it('以当前大版本最早赛季为起点，并在后续大版本起点半开截止', () => {
    expect(period).toEqual({ start: 1_710_000_000_000, end: 1_730_000_000_000 });
    expect(isTrialRecordInProgressPeriod(record({ date: 1_710_000_000 }), period)).toBe(true);
    expect(isTrialRecordInProgressPeriod(record({ date: 1_730_000_000 }), period)).toBe(false);
  });

  it('旧大版本 60 层不会带入没有当前记录的角色', () => {
    expect(getTrialLayerForRole([record({ date: 1_700_000_100, layer: 60 })], 'role-a', period)).toBe(1);
  });

  it('缺少当前赛季、版本关系或合法起点时不伪造周期', () => {
    expect(getTrialProgressPeriod(null, seasons, versions)).toBeNull();
    expect(getTrialProgressPeriod({ ...seasons[1], versionId: 99 }, seasons, versions)).toBeNull();
    expect(getTrialProgressPeriod(seasons[1], [{ ...seasons[1], startDate: 0 }], versions)).toBeNull();
  });

  it('当前大版本已有记录时使用最近层数，不擅自加层', () => {
    expect(getTrialLayerForRole([record({ date: 1_710_000_100, layer: 12 })], 'role-a', period)).toBe(12);
  });

  it('同大版本跨赛季仍共享进度，角色之间严格隔离', () => {
    const secondSeasonPeriod = getTrialProgressPeriod(seasons[2], seasons, versions);
    const records = [
      record({ id: 'first', date: 1_710_000_100, layer: 9 }),
      record({ id: 'second', date: 1_720_000_100, layer: 18 }),
      record({ id: 'other-role', roleId: 'role-b', date: 1_720_000_200, layer: 88 }),
    ];
    expect(secondSeasonPeriod).toEqual({ start: 1_710_000_000_000, end: 1_730_000_000_000 });
    expect(getTrialLayerForRole([records[0]], 'role-a', secondSeasonPeriod)).toBe(9);
    expect(getTrialLayerForRole(records, 'role-a', period)).toBe(18);
    expect(getTrialLayerForRole(records, 'role-b', period)).toBe(88);
  });

  it('自动填充查询会忽略旧版本同周记录', () => {
    const records = [
      record({ id: 'old', date: 1_709_999_900, layer: 60, bosses: ['旧一', '旧二', '旧三'] }),
      record({ id: 'current', date: 1_710_000_100, layer: 12, bosses: ['新一', '新二', '新三'] }),
    ];
    expect(getLatestTrialRecordForRole(records, 'role-a', period, 1_709_000_000_000)?.id).toBe('current');
    expect(getLatestTrialRecordForRole([records[0]], 'role-a', period, 1_709_000_000_000)).toBeUndefined();
  });

  it('支持秒、毫秒和 ISO 日期，并在没有合法周期或记录时安全默认 1', () => {
    expect(toTrialProgressTimestamp(1_710_000_000)).toBe(1_710_000_000_000);
    expect(toTrialProgressTimestamp(1_710_000_000_000)).toBe(1_710_000_000_000);
    expect(toTrialProgressTimestamp('2024-03-09T16:00:00.000Z')).toBe(1_710_000_000_000);
    expect(getTrialLayerForRole([
      record({ id: 'seconds', date: 1_710_000_001, layer: 4 }),
      record({ id: 'milliseconds', date: 1_710_000_002_000, layer: 8 }),
      record({ id: 'iso', date: '2024-03-09T16:00:03.000Z', layer: 12 }),
    ], 'role-a', period)).toBe(12);
    expect(getTrialLayerForRole([], 'role-a', period)).toBe(1);
    expect(getTrialLayerForRole([record()], 'role-a', null)).toBe(1);
  });

  it('只读取 records，不修改历史记录', () => {
    const records = [record({ date: 1_720_000_100, layer: 18 })];
    const snapshot = JSON.parse(JSON.stringify(records));
    getTrialLayerForRole(records, 'role-a', period);
    getLatestTrialRecordForRole(records, 'role-a', period, 1_710_000_000_000);
    expect(records).toEqual(snapshot);
  });
});
