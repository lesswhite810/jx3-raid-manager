import { describe, expect, it } from 'vitest';
import { getTrialWeeklyDrawProgress } from './trialWeeklyDrawProgress';
import type { TrialPlaceRecord } from '../types';

const WEEK_START = new Date(1793548800000);
const PROGRESS_PERIOD = { start: 1793228400000 };

type Stats = { weeklyDrawCount: number; pendingCount: number };

/** 复刻 TrialPlaceManager 中的三态判定（与组件内 getTrialStatus 保持一致） */
const getTrialStatus = (weeklyDrawCount: number): 'none' | 'partial' | 'complete' => {
  if (weeklyDrawCount === 0) return 'none';
  if (weeklyDrawCount >= 3) return 'complete';
  return 'partial';
};

/** 复刻组件的 roleStats 计算：统计本周翻牌次数 */
const calcStats = (records: TrialPlaceRecord[]): Stats => {
  const progress = getTrialWeeklyDrawProgress(records, {
    weekStart: WEEK_START,
    progressPeriod: PROGRESS_PERIOD as never,
  });
  return { weeklyDrawCount: progress.drawCount, pendingCount: progress.pendingRunCount };
};

const mkRecord = (over: Partial<TrialPlaceRecord> = {}): TrialPlaceRecord => ({
  id: 'r1',
  accountId: 'a1',
  roleId: 'role1',
  roleName: '少年白了发',
  server: '梦江南',
  layer: 6,
  bosses: [],
  card1: '',
  card2: '',
  card3: '',
  card4: '',
  card5: '',
  flippedIndex: 0,
  recordType: 'trial',
  date: '2026-11-03T23:10:59+08:00',
  notes: '',
  updatedAt: '2026-11-06T01:29:23+08:00',
  source: 'auto',
  status: 'confirmed',
  draws: [],
  equipmentUnreviewed: true,
  ...over,
} as TrialPlaceRecord);

describe('试炼三态按本周翻牌次数判定', () => {
  it('少年白了发：1 次通关含 4 笔翻牌 -> 完全清，不再是部分清', () => {
    // 真实数据：一条本周通关记录，4 笔自动子笔，每笔收据都在本周
    const draws = [1, 2, 3, 4].map((seq) => ({
      id: `d${seq}`,
      sourceKey: `k${seq}`,
      sequence: seq,
      receiptTime: 1793718660000 + seq * 20000,
      flippedIndex: 2,
      card1: '', card2: '', card3: '', card4: '', card5: '',
      equipmentUnreviewed: false,
      source: 'auto' as const,
    }));

    const record = mkRecord({ draws: draws as never });
    const stats = calcStats([record]);

    expect(stats.weeklyDrawCount).toBe(4);
    expect(stats.pendingCount).toBe(0);
    // 旧口径按通关次数（1 条）会判为 partial；新口径按翻牌 4 次判为 complete
    expect(getTrialStatus(stats.weeklyDrawCount)).toBe('complete');
  });

  it('我的大老婆：2 次通关含 3 笔翻牌 -> 完全清', () => {
    const record = mkRecord({
      id: 'r2',
      draws: [1, 2, 3].map((seq) => ({
        id: `d${seq}`,
        sourceKey: `k${seq}`,
        sequence: seq,
        receiptTime: 1794029401000 + seq * 20000,
        flippedIndex: 2,
        card1: '', card2: '', card3: '', card4: '', card5: '',
        equipmentUnreviewed: false,
        source: 'auto' as const,
      })) as never,
    });

    const stats = calcStats([record]);
    expect(stats.weeklyDrawCount).toBe(3);
    expect(getTrialStatus(stats.weeklyDrawCount)).toBe('complete');
  });

  it('翻牌 1-2 次仍判为部分清（保留中间态语义）', () => {
    const record = mkRecord({
      id: 'r3',
      draws: [1, 2].map((seq) => ({
        id: `d${seq}`,
        sourceKey: `k${seq}`,
        sequence: seq,
        receiptTime: 1793718660000 + seq * 20000,
        flippedIndex: 2,
        card1: '', card2: '', card3: '', card4: '', card5: '',
        equipmentUnreviewed: false,
        source: 'auto' as const,
      })) as never,
    });

    const stats = calcStats([record]);
    expect(stats.weeklyDrawCount).toBe(2);
    expect(getTrialStatus(stats.weeklyDrawCount)).toBe('partial');
  });

  it('无翻牌记录判为未清', () => {
    const stats = calcStats([]);
    expect(stats.weeklyDrawCount).toBe(0);
    expect(getTrialStatus(stats.weeklyDrawCount)).toBe('none');
  });

  it('旧记录无子笔但牌位已知时，兼容计为 1 笔（发白少场景，不会误判为未清）', () => {
    const record = mkRecord({
      id: 'r4',
      roleName: '发白少',
      draws: [],
      flippedIndex: 2,
      status: 'confirmed',
    });

    const stats = calcStats([record]);
    expect(stats.weeklyDrawCount).toBe(1);
    expect(getTrialStatus(stats.weeklyDrawCount)).toBe('partial');
  });
});
