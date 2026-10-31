import { describe, expect, it } from 'vitest';
import { TrialPlaceRecord } from '../types';
import {
  deriveTrialEquipmentLevelRange,
  getTrialEquipmentLevelRange,
  getTrialRecordEquipmentEntries,
  getTrialRecordEquipmentIds,
  isTrialEquipmentLevelInRange,
} from './trialRecordUtils';

describe('赛季试炼装备品级范围', () => {
  it('存在赛季但未配置边界时以空配置表示', () => {
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 0, trialEquipLevelMax: 0 })).toEqual({});
    expect(getTrialEquipmentLevelRange({})).toEqual({});
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: null, trialEquipLevelMax: null })).toEqual({});
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 900, trialEquipLevelMax: null }))
      .toEqual({ min: 900 });
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: null, trialEquipLevelMax: 900 }))
      .toEqual({ max: 900 });
  });

  it('当前赛季元数据不可用时返回 null，不能降级读取旧装备', () => {
    expect(getTrialEquipmentLevelRange(null)).toBeNull();
    expect(getTrialEquipmentLevelRange(undefined)).toBeNull();
  });

  it('允许等级压缩后的低品级、单一品级和单边配置', () => {
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 100, trialEquipLevelMax: 500 }))
      .toEqual({ min: 100, max: 500 });
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 100, trialEquipLevelMax: 100 }))
      .toEqual({ min: 100, max: 100 });
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 900 }))
      .toEqual({ min: 900 });
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMax: 900 }))
      .toEqual({ max: 900 });
  });

  it('保留旧赛季有效范围，拒绝反向、负数和非整数范围', () => {
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 25500, trialEquipLevelMax: 41400 }))
      .toEqual({ min: 25500, max: 41400 });
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 500, trialEquipLevelMax: 100 })).toBeNull();
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: -1, trialEquipLevelMax: 100 })).toBeNull();
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 1.5, trialEquipLevelMax: 100 })).toBeNull();
    expect(getTrialEquipmentLevelRange({ trialEquipLevelMin: 1, trialEquipLevelMax: Infinity })).toBeNull();
  });

  it('最低品级 900 先过滤旧品级，再按匹配结果推导未知上限', () => {
    const range = getTrialEquipmentLevelRange({ trialEquipLevelMin: 900 });
    expect(range).toEqual({ min: 900 });
    if (!range) throw new Error('最低品级配置应合法');

    const eligibleLevels = [899, 900, 1200, 25500]
      .filter(level => isTrialEquipmentLevelInRange(level, range));
    expect(eligibleLevels).toEqual([900, 1200, 25500]);

    const actual = deriveTrialEquipmentLevelRange(eligibleLevels);
    expect(actual).toEqual({ min: 900, max: 25500 });
    expect({ min: range.min ?? actual?.min, max: range.max ?? actual?.max })
      .toEqual({ min: 900, max: 25500 });
  });

  it('没有符合单边范围的装备时不回退到旧装备', () => {
    const range = getTrialEquipmentLevelRange({ trialEquipLevelMin: 900 });
    if (!range) throw new Error('最低品级配置应合法');
    const eligibleLevels = [100, 899].filter(level => isTrialEquipmentLevelInRange(level, range));
    expect(eligibleLevels).toEqual([]);
    expect(deriveTrialEquipmentLevelRange(eligibleLevels)).toBeNull();
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

  it('赛季已配置范围优先固定已知边界，未知边界由实际数据补齐', () => {
    const seasonRange = getTrialEquipmentLevelRange({ trialEquipLevelMin: 25500, trialEquipLevelMax: 41400 });
    const dataRange = deriveTrialEquipmentLevelRange([19800, 23500]);
    expect(seasonRange).toEqual({ min: 25500, max: 41400 });
    expect(dataRange).toEqual({ min: 19800, max: 23500 });

    const unconfiguredRange = getTrialEquipmentLevelRange({ trialEquipLevelMin: 0, trialEquipLevelMax: 0 });
    expect(unconfiguredRange).toEqual({});
    expect(dataRange).toEqual({ min: 19800, max: 23500 });
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
