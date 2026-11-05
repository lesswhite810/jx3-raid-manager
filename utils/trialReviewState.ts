import type { TrialDraw, TrialPlaceRecord } from '../types';

/** 未知牌位或牌面尚未由用户完整核对，都需要继续处理。 */
export const trialDrawNeedsReview = (draw: Pick<TrialDraw, 'flippedIndex' | 'equipmentUnreviewed'>): boolean => (
  !Number.isInteger(draw.flippedIndex) || draw.flippedIndex < 1 || draw.flippedIndex > 5 || draw.equipmentUnreviewed === true
);

/** 保留通关状态和翻牌状态各自的含义，不通过修改通关状态来标记牌面。 */
export const trialRecordNeedsReview = (record: TrialPlaceRecord): boolean => {
  if (record.status === 'pending') return true;
  const draws = Array.isArray(record.draws) ? record.draws : [];
  if (draws.length > 0) {
    const hasManual = draws.some(draw => draw.source === 'manual');
    const hasAutomatic = draws.some(draw => draw.source !== 'manual');
    return (hasManual && hasAutomatic) || draws.some(trialDrawNeedsReview);
  }
  return trialDrawNeedsReview(record);
};
