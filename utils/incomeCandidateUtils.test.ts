import { describe, expect, it } from 'vitest';
import { IncomeCandidate } from '../types';
import {
  formatIncomeCandidateAmount,
  hasUnreviewedIncomeCandidates,
  isWholeGoldIncomeCandidate,
  selectableIncomeCandidates,
  sumIncomeCandidates,
  toggleIncomeCandidateSelection,
} from './incomeCandidateUtils';

const candidates: IncomeCandidate[] = [
  { id: 'a', time: 1_700_000_000, amount: 27 },
  { id: 'b', time: 1_700_000_001, amount: 1540 },
];

describe('incomeCandidateUtils', () => {
  it('supports selecting and cancelling one candidate', () => {
    expect(toggleIncomeCandidateSelection([], 'a')).toEqual(['a']);
    expect(toggleIncomeCandidateSelection(['a'], 'a')).toEqual([]);
  });

  it('sums multiple selected candidates and ignores unknown ids', () => {
    expect(sumIncomeCandidates(candidates, ['a', 'b'])).toBe(1567);
    expect(sumIncomeCandidates(candidates, ['missing'])).toBe(0);
  });

  it('does not mutate the selected id list', () => {
    const selected = ['a'];
    toggleIncomeCandidateSelection(selected, 'b');
    expect(selected).toEqual(['a']);
  });

  it('marks candidates as needing review until a selection is persisted', () => {
    expect(hasUnreviewedIncomeCandidates({ incomeCandidates: candidates })).toBe(true);
    expect(hasUnreviewedIncomeCandidates({ incomeCandidates: candidates, selectedIncomeCandidateIds: [] })).toBe(false);
    expect(hasUnreviewedIncomeCandidates({})).toBe(false);
  });

  it('shows original gold, silver, copper precision when available', () => {
    expect(formatIncomeCandidateAmount({ id: 'x', time: 1, amount: 27, amountCopper: 277011 })).toBe('27金70银11铜');
    expect(formatIncomeCandidateAmount({ id: 'y', time: 1, amount: 1540 })).toBe('1540金');
  });

  it('treats only whole-gold amounts as selectable income', () => {
    expect(isWholeGoldIncomeCandidate({ id: 'gold', time: 1, amount: 1540, amountCopper: 15_400_000 })).toBe(true);
    expect(isWholeGoldIncomeCandidate({ id: 'silver', time: 1, amount: 27, amountCopper: 277_011 })).toBe(false);
    // 0 银但有铜（如 30金0银1铜）同样不是整金
    expect(isWholeGoldIncomeCandidate({ id: 'copper', time: 1, amount: 30, amountCopper: 300_001 })).toBe(false);
    // 旧数据缺少铜精度时按整金口径兼容
    expect(isWholeGoldIncomeCandidate({ id: 'legacy', time: 1, amount: 27 })).toBe(true);
  });

  it('filters out candidates with non-zero silver or copper', () => {
    const mixed: IncomeCandidate[] = [
      { id: 'a', time: 1, amount: 1540, amountCopper: 15_400_000 },
      { id: 'b', time: 2, amount: 27, amountCopper: 277_011 },
      { id: 'c', time: 3, amount: 30, amountCopper: 300_001 },
    ];
    expect(selectableIncomeCandidates(mixed).map(candidate => candidate.id)).toEqual(['a']);
    expect(selectableIncomeCandidates(undefined)).toEqual([]);
  });

  it('ignores filtered-out candidates when summing a selection', () => {
    const mixed: IncomeCandidate[] = [
      { id: 'a', time: 1, amount: 1540, amountCopper: 15_400_000 },
      { id: 'b', time: 2, amount: 27, amountCopper: 277_011 },
    ];
    // 旧数据里若仍残留带银/铜的选中项，不应把零钱计入工资
    expect(sumIncomeCandidates(mixed, ['a', 'b'])).toBe(1540);
  });

  it('only requires review when a whole-gold candidate is still unreviewed', () => {
    const silverOnly: IncomeCandidate[] = [
      { id: 'b', time: 2, amount: 27, amountCopper: 277_011 },
    ];
    expect(hasUnreviewedIncomeCandidates({ incomeCandidates: silverOnly })).toBe(false);
    expect(hasUnreviewedIncomeCandidates({
      incomeCandidates: [...silverOnly, { id: 'a', time: 1, amount: 1540, amountCopper: 15_400_000 }],
    })).toBe(true);
    expect(hasUnreviewedIncomeCandidates({
      incomeCandidates: silverOnly,
      selectedIncomeCandidateIds: [],
    })).toBe(false);
  });
});
