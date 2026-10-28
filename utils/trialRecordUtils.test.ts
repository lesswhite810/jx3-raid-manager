import { describe, expect, it } from 'vitest';
import { TrialPlaceRecord } from '../types';
import {
  deriveTrialEquipmentLevelRange,
  getTrialEquipmentLevelRange,
  getTrialRecordEquipmentEntries,
  getTrialRecordEquipmentIds,
} from './trialRecordUtils';

describe('赛季试炼装备品级范围', () => {
  it('新赛季未配置范围时不回退到旧赛季品级', () => {
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 0, trialEquipLevelMax: 0 })).toBeNull();
    expect(getTrialEquipmentLevelRange({})).toBeNull();
    expect(getTrialEquipmentLevelRange(null)).toBeNull();
  });

  it('允许等级压缩后的低品级，以及单一品级', () => {
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 100, trialEquipLevelMax: 500 }))
      .toEqual({ min: 100, max: 500 });
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 100, trialEquipLevelMax: 100 }))
      .toEqual({ min: 100, max: 100 });
  });

  it('保留旧赛季有效范围，拒绝反向、负数和非整数范围', () => {
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 25500, trialEquipLevelMax: 41400 }))
      .toEqual({ min: 25500, max: 41400 });
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 500, trialEquipLevelMax: 100 })).toBeNull();
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: -1, trialEquipLevelMax: 100 })).toBeNull();
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 1.5, trialEquipLevelMax: 100 })).toBeNull();
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 1, trialEquipLevelMax: Infinity })).toBeNull();
  });
});

describe('按实际装备数据推导品级范围', () => {
  it('新赛季未配置范围时，由同步到的装备品级推导区间', () => {
    expect(deriveTrialEquipmentLevelRange([19800, 41400, 25500]))
      .toEqual({ min: 19800, max: 41400 });
  });

  it('忽略缺失、非正数与非法品级', () => {
    expect(deriveTrialEquipmentLevelRange([null, undefined, 0, -1, 23500, 19800]))
      .toEqual({ min: 19800, max: 23500 });
    expect(deriveTrialEquipmentLevelRange([Number.NaN, Number.POSITIVE_INFINITY]))
      .toBeNull();
  });

  it('没有可用品级时返回 null，而不是回退到默认值', () => {
    expect(deriveTrialEquipmentLevelRange([])).toBeNull();
    expect(deriveTrialEquipmentLevelRange([null, undefined])).toBeNull();
  });

  it('单一品级时区间上下限相同', () => {
    expect(deriveTrialEquipmentLevelRange([23500])).toEqual({ min: 23500, max: 23500 });
  });

  it('赛季已配置范围时优先使用赛季范围，不用实际数据覆盖', () => {
    const seasonRange = getTrialEquipmentLevelRange({ trialEquipLevelMin: 25500, trialEquipLevelMax: 41400 });
    const dataRange = deriveTrialEquipmentLevelRange([19800, 23500]);
    expect(seasonRange ?? dataRange).toEqual({ min: 25500, max: 41400 });

    const unconfiguredRange = getTrialEquipmentLevelRange({ trialEquipLevelMin: 0, trialEquipLevelMax: 0 });
    expect(unconfiguredRange ?? dataRange).toEqual({ min: 19800, max: 23500 });
  });
});

const createTrialRecord = (overrides: Partial<TrialPlaceRecord> = {}): TrialPlaceRecord => ({
  id: 'record-1',
  accountId: 'account-1',
  roleId: 'role-1',
  roleName: '阿试',
  server: '梦江南',
  date: Date.now(),
  layer: 10,
  bosses: ['一号', '二号', '三号'],
  card1: '',
  card2: '',
  card3: '',
  card4: '',
  card5: '',
  flippedIndex: 1,
  type: 'trial',
  ...overrides
});

describe('getTrialRecordEquipmentIds', () => {
  it('returns recorded equipments even when flipped card is not selected', () => {
    const record = createTrialRecord({
      card2: '2001',
      card4: '2002',
      flippedIndex: 1
    });

    expect(getTrialRecordEquipmentIds(record)).toEqual(['2001', '2002']);
  });

  it('keeps the flipped equipment first when it exists', () => {
    const record = createTrialRecord({
      card1: '1001',
      card3: '1003',
      flippedIndex: 3
    });

    expect(getTrialRecordEquipmentIds(record)).toEqual(['1003', '1001']);
  });

  it('reads legacy snake_case card fields', () => {
    const record = {
      ...createTrialRecord(),
      card1: '',
      card2: '',
      card3: '',
      card4: '',
      card5: '',
      flippedIndex: 1,
      card_1: '3001',
      card_3: '3003'
    } as TrialPlaceRecord & {
      card_1?: string;
      card_3?: string;
    };

    expect(getTrialRecordEquipmentIds(record)).toEqual(['3001', '3003']);
  });

  it('preserves duplicate equipment ids from different card slots', () => {
    const record = createTrialRecord({
      card1: '4001',
      card3: '4001',
      flippedIndex: 1
    });

    expect(getTrialRecordEquipmentIds(record)).toEqual(['4001', '4001']);
  });

  it('returns card slot metadata and flipped state for duplicate equipments', () => {
    const record = createTrialRecord({
      card1: '5001',
      card3: '5001',
      flippedIndex: 1
    });

    expect(getTrialRecordEquipmentEntries(record)).toEqual([
      { cardIndex: 1, equipId: '5001', isFlipped: true },
      { cardIndex: 3, equipId: '5001', isFlipped: false }
    ]);
  });
});
