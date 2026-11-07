import { IncomeCandidate } from '../types';

/**
 * 是否为整金入账（0 银 0 铜）。
 *
 * 待选收入只应展示可以整金计入工资的入账；带银或铜的金额是零钱拾取，
 * 既不能当作工资结算，混进候选列表还会让用户误选。
 * 旧数据没有 `amountCopper` 时按整金口径处理，保持兼容。
 */
export function isWholeGoldIncomeCandidate(candidate: IncomeCandidate): boolean {
  if (candidate.amountCopper === undefined) return true;
  const copper = Math.max(0, Math.trunc(candidate.amountCopper));
  return copper % 10_000 === 0;
}

/** 过滤出可以展示和选择的待选收入：只保留 0 银 0 铜的整金入账。 */
export function selectableIncomeCandidates(
  candidates?: IncomeCandidate[],
): IncomeCandidate[] {
  return (candidates ?? []).filter(isWholeGoldIncomeCandidate);
}

export function hasUnreviewedIncomeCandidates(record: {
  incomeCandidates?: IncomeCandidate[];
  selectedIncomeCandidateIds?: string[];
}): boolean {
  return (
    selectableIncomeCandidates(record.incomeCandidates).length > 0
    && record.selectedIncomeCandidateIds === undefined
  );
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
  return selectableIncomeCandidates(candidates)
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
