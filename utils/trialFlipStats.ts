import { TrialFlipPositionStats, TrialFlipStatsSummary, TrialPlaceRecord } from '../types';
import { getTrialRecordEquipmentEntries } from './trialRecordUtils';

const TRIAL_POSITIONS = [1, 2, 3, 4, 5] as const;

const createEmptyPositionStats = (position: number): TrialFlipPositionStats => ({
  position,
  flipCount: 0,
  flippedEquipmentCount: 0,
  appearanceCount: 0,
  flipEquipmentRate: 0,
  appearanceRate: 0
});

export const calculateTrialFlipStats = (records: TrialPlaceRecord[]): TrialFlipStatsSummary => {
  const positions = TRIAL_POSITIONS.map(position => createEmptyPositionStats(position));
  const statsMap = new Map<number, TrialFlipPositionStats>(positions.map(item => [item.position, item]));

  records.forEach(record => {
    const flippedStats = statsMap.get(record.flippedIndex);
    if (flippedStats) {
      flippedStats.flipCount += 1;
    }

    const equipmentEntries = getTrialRecordEquipmentEntries(record);
    equipmentEntries.forEach(entry => {
      const positionStats = statsMap.get(entry.cardIndex);
      if (!positionStats) {
        return;
      }

      positionStats.appearanceCount += 1;
      if (entry.isFlipped) {
        positionStats.flippedEquipmentCount += 1;
      }
    });
  });

  const totalRecords = records.length;
  positions.forEach(positionStats => {
    positionStats.flipEquipmentRate =
      positionStats.flipCount > 0
        ? positionStats.flippedEquipmentCount / positionStats.flipCount
        : 0;
    positionStats.appearanceRate =
      totalRecords > 0
        ? positionStats.appearanceCount / totalRecords
        : 0;
  });

  const bestFlipPosition = positions.reduce<TrialFlipPositionStats | null>((best, current) => {
    if (current.flipCount === 0 && current.flippedEquipmentCount === 0) {
      return best;
    }
    if (!best || current.flipEquipmentRate > best.flipEquipmentRate) {
      return current;
    }
    if (best && current.flipEquipmentRate === best.flipEquipmentRate && current.flipCount > best.flipCount) {
      return current;
    }
    return best;
  }, null);

  const bestAppearancePosition = positions.reduce<TrialFlipPositionStats | null>((best, current) => {
    if (current.appearanceCount === 0) {
      return best;
    }
    if (!best || current.appearanceRate > best.appearanceRate) {
      return current;
    }
    if (best && current.appearanceRate === best.appearanceRate && current.appearanceCount > best.appearanceCount) {
      return current;
    }
    return best;
  }, null);

  return {
    totalRecords,
    positions,
    bestFlipPosition,
    bestAppearancePosition
  };
};
