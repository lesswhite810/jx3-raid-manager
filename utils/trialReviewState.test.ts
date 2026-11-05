import { describe, expect, it } from 'vitest';
import type { TrialDraw, TrialPlaceRecord } from '../types';
import { trialDrawNeedsReview, trialRecordNeedsReview } from './trialReviewState';

const draw = (overrides: Partial<TrialDraw> = {}): TrialDraw => ({
  id: 'draw', sourceKey: 'receipt', sequence: 1, receiptTime: 1, flippedIndex: 2,
  rewardItems: [], card1: '', card2: '', card3: '', card4: '', card5: '',
  equipmentUnreviewed: false, ...overrides,
});
const record = (draws: TrialDraw[], status: TrialPlaceRecord['status'] = 'confirmed'): TrialPlaceRecord => ({
  id: 'run', accountId: 'a', roleId: 'r', roleName: 'r', server: 's', date: 1, layer: 1,
  bosses: ['', '', ''], flippedIndex: 2, type: 'trial', source: 'auto', status,
  card1: '', card2: '', card3: '', card4: '', card5: '', draws,
});

describe('试炼翻牌待确认状态', () => {
  it('牌位未知或牌面未核对时保持待确认', () => {
    expect(trialDrawNeedsReview(draw({ flippedIndex: 0 }))).toBe(true);
    expect(trialDrawNeedsReview(draw({ equipmentUnreviewed: true }))).toBe(true);
    expect(trialDrawNeedsReview(draw())).toBe(false);
  });
  it('通关待确认和手工明细待归属不因牌面完整而消失', () => {
    expect(trialRecordNeedsReview(record([draw()], 'pending'))).toBe(true);
    expect(trialRecordNeedsReview(record([draw({ source: 'auto' }), draw({ id: 'm', source: 'manual' })]))).toBe(true);
    expect(trialRecordNeedsReview(record([draw()]))).toBe(false);
  });
  it('旧单次记录依据父记录的实际牌位和核对状态判断', () => {
    expect(trialRecordNeedsReview({ ...record([]), flippedIndex: 0 })).toBe(true);
    expect(trialRecordNeedsReview({ ...record([]), equipmentUnreviewed: true })).toBe(true);
    expect(trialRecordNeedsReview(record([]))).toBe(false);
  });
});
