import { describe, expect, it } from 'vitest';
import { IncomeCandidate } from '../types';
import {
  formatIncomeCandidateAmount,
  hasUnreviewedIncomeCandidates,
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
});
