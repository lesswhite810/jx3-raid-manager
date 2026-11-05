import { IncomeCandidate } from '../types';

export function hasUnreviewedIncomeCandidates(record: {
  incomeCandidates?: IncomeCandidate[];
  selectedIncomeCandidateIds?: string[];
}): boolean {
  return (record.incomeCandidates?.length ?? 0) > 0 && record.selectedIncomeCandidateIds === undefined;
}

export function toggleIncomeCandidateSelection(
  selectedIds: string[],
  candidateId: string,
): string[] {
  return selectedIds.includes(candidateId)
    ? selectedIds.filter(id => id !== candidateId)
    : [...selectedIds, candidateId];
}

export function sumIncomeCandidates(
  candidates: IncomeCandidate[],
  selectedIds: string[],
): number {
  const selected = new Set(selectedIds);
  return candidates
    .filter(candidate => selected.has(candidate.id))
    .reduce((total, candidate) => total + candidate.amount, 0);
}

/** 展示原始精度；记录保存仍使用 RaidRecord.goldIncome 的整金口径。 */
export function formatIncomeCandidateAmount(candidate: IncomeCandidate): string {
  if (candidate.amountCopper === undefined) return `${candidate.amount}金`;
  const copper = Math.max(0, Math.trunc(candidate.amountCopper));
  const gold = Math.floor(copper / 10_000);
  const silver = Math.floor((copper % 10_000) / 100);
  const remainCopper = copper % 100;
  return `${gold}金${silver}银${remainCopper}铜`;
}
