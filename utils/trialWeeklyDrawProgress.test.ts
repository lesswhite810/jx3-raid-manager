import { describe, expect, it } from 'vitest';
import type { TrialDraw, TrialPlaceRecord } from '../types';
import { getTrialWeekEnd, getTrialWeeklyDrawProgress, isTrialRecordInWeek } from './trialWeeklyDrawProgress';
import { getLastMonday } from './cooldownManager';

const WEEK_START = Date.UTC(2026, 10, 2, 0, 0, 0); // 周一
const PERIOD = {
  start: Date.UTC(2026, 9, 1, 0, 0, 0),
  end: Date.UTC(2026, 11, 1, 0, 0, 0),
};

const createDraw = (overrides: Partial<TrialDraw> = {}): TrialDraw => ({
  id: 'draw',
  sourceKey: 'receipt',
  sequence: 1,
  source: 'auto',
  receiptTime: WEEK_START + 1_000,
  flippedIndex: 2,
  rewardItems: [],
  card1: '',
  card2: '',
  card3: '',
  card4: '',
  card5: '',
  ...overrides,
});

const createRecord = (overrides: Partial<TrialPlaceRecord> = {}): TrialPlaceRecord => ({
  id: 'run',
  accountId: 'account',
  roleId: 'role',
  roleName: '少年白了发',
  server: '梦江南',
  date: WEEK_START + 60_000,
  layer: 6,
  bosses: ['甲', '乙', '丙'],
  card1: '',
  card2: '',
  card3: '',
  card4: '',
  card5: '',
  flippedIndex: 0,
  type: 'trial',
  source: 'auto',
  status: 'confirmed',
  ...overrides,
});

const calculate = (records: TrialPlaceRecord[]) => getTrialWeeklyDrawProgress(records, {
  weekStart: WEEK_START,
  progressPeriod: PERIOD,
});

describe('试炼本周翻牌进度', () => {
  it('在周一 07:00 切换游戏周，并统计 11 月 3 日同一轮四次翻牌', () => {
    const beforeReset = new Date(2026, 10, 2, 6, 59, 59);
    const afterReset = new Date(2026, 10, 2, 7, 0, 0);
    expect(getLastMonday(beforeReset).getTime()).toBe(new Date(2026, 9, 26, 7, 0, 0).getTime());
    expect(getLastMonday(afterReset).getTime()).toBe(afterReset.getTime());
    const receipts = [0, 20, 22, 27].map((seconds, index) => createDraw({
      id: `receipt-${index}`, sequence: index + 1, receiptTime: new Date(2026, 10, 3, 23, 11, seconds).getTime(),
      flippedIndex: index === 0 ? 2 : 0,
    }));
    const result = getTrialWeeklyDrawProgress([createRecord({ date: new Date(2026, 10, 3, 23, 10, 59).getTime(), draws: receipts })], {
      weekStart: getLastMonday(new Date(2026, 10, 5)), progressPeriod: PERIOD,
    });
    expect(result.drawCount).toBe(4);
    expect(result.pendingRunCount).toBe(1);
  });
  it('按自动子笔的真实收据时间跨周归属，不按父通关日期猜测', () => {
    const records = [
      createRecord({
        id: 'before-week',
        date: WEEK_START + 60_000,
        draws: [createDraw({ receiptTime: WEEK_START - 1_000 })],
      }),
      createRecord({
        id: 'after-week',
        date: WEEK_START - 60_000,
        draws: [createDraw({ id: 'monday-receipt', receiptTime: WEEK_START + 1_000 })],
      }),
    ];

    expect(calculate(records)).toEqual({
      drawCount: 1,
      pendingRunCount: 0,
      manualDateFallbackDrawCount: 0,
    });
  });

  it('自动子笔以收据时间跨大版本归属，手工子笔则明确回退父日期', () => {
    const records = [
      createRecord({
        id: 'auto-next-version',
        draws: [createDraw({ receiptTime: PERIOD.end })],
      }),
      createRecord({
        id: 'manual-fallback',
        draws: [createDraw({ id: 'manual', source: 'manual', receiptTime: 0 })],
      }),
      createRecord({
        id: 'auto-current-parent-old',
        date: PERIOD.start - 1_000,
        draws: [createDraw({ id: 'current-receipt', receiptTime: WEEK_START + 1_000 })],
      }),
      createRecord({
        id: 'auto-without-receipt',
        draws: [createDraw({ id: 'missing-receipt', receiptTime: 0 })],
      }),
      createRecord({
        id: 'numeric-receipt',
        draws: [createDraw({ id: 'string-receipt', receiptTime: String(WEEK_START + 2_000) })],
      }),
    ];

    expect(calculate(records)).toEqual({
      drawCount: 3,
      pendingRunCount: 0,
      manualDateFallbackDrawCount: 1,
    });
  });

  it('已入包的未知牌位和父通关待确认仍计为观察到的翻牌，但待确认只按轮计', () => {
    const records = [
      createRecord({
        id: 'pending-auto',
        status: 'pending',
        draws: [
          createDraw({ id: 'unknown', flippedIndex: 0 }),
          createDraw({ id: 'known', sequence: 2, receiptTime: WEEK_START + 2_000, flippedIndex: 5 }),
        ],
      }),
    ];

    expect(calculate(records)).toEqual({
      drawCount: 2,
      pendingRunCount: 1,
      manualDateFallbackDrawCount: 0,
    });
  });

  it('同一轮日志与手工子笔混合时仅统计日志，手工待归属仍使该轮待确认', () => {
    const records = [createRecord({
      draws: [
        createDraw({ id: 'auto', source: 'auto', flippedIndex: 3 }),
        createDraw({ id: 'manual', source: 'manual', sequence: 2, receiptTime: 0, flippedIndex: 1 }),
      ],
    })];

    expect(calculate(records)).toEqual({
      drawCount: 1,
      pendingRunCount: 1,
      manualDateFallbackDrawCount: 0,
    });
  });

  it('旧无子笔记录仅在父牌位已知时兼容一笔，且不改变输入记录', () => {
    const records = [
      createRecord({ id: 'legacy-known', flippedIndex: 4, draws: undefined }),
      createRecord({ id: 'legacy-unknown', flippedIndex: 0, draws: undefined }),
      createRecord({ id: 'pending-without-draw', flippedIndex: 0, status: 'pending', draws: undefined }),
    ];
    const snapshot = JSON.parse(JSON.stringify(records));

    expect(calculate(records)).toEqual({
      drawCount: 1,
      pendingRunCount: 2,
      manualDateFallbackDrawCount: 0,
    });
    expect(records).toEqual(snapshot);
  });

  it('使用游戏周的半开区间，不把下周收据算进本周', () => {
    const nextWeekStart = getTrialWeekEnd(WEEK_START);
    expect(nextWeekStart).not.toBeNull();
    const records = [
      createRecord({ draws: [createDraw({ receiptTime: nextWeekStart! - 1 })] }),
      createRecord({ id: 'next-week', draws: [createDraw({ id: 'next', receiptTime: nextWeekStart! })] }),
    ];

    expect(calculate(records).drawCount).toBe(1);
  });

  it('详情本周范围兼容父通关日期或可信子笔时间，不把自动未知收据当作父日期', () => {
    const nextWeekStart = getTrialWeekEnd(WEEK_START)!;
    expect(isTrialRecordInWeek(createRecord({
      date: WEEK_START - 1_000,
      draws: [createDraw({ receiptTime: WEEK_START + 1_000 })],
    }), WEEK_START, nextWeekStart)).toBe(true);
    expect(isTrialRecordInWeek(createRecord({
      date: WEEK_START - 1_000,
      draws: [createDraw({ receiptTime: 0 })],
    }), WEEK_START, nextWeekStart)).toBe(false);
    expect(isTrialRecordInWeek(createRecord({
      date: WEEK_START + 1_000,
      draws: [createDraw({ source: 'manual', receiptTime: 0 })],
    }), WEEK_START, nextWeekStart)).toBe(true);
  });
});
