import {
  TrialBossEquipmentPositionStats,
  TrialBossEquipmentStats,
  TrialFlipPositionStats,
  TrialFlipStatsSummary,
  TrialPlaceRecord
} from '../types';
import { getTrialRecordEquipmentEntries } from './trialRecordUtils';

const TRIAL_POSITIONS = [1, 2, 3, 4, 5] as const;
const TRIAL_BOSS_KEY_SEPARATOR = '\u001f';

const createEmptyPositionStats = (position: number): TrialFlipPositionStats => ({
  position,
  flipCount: 0,
  flippedEquipmentCount: 0,
  appearanceCount: 0,
  flipEquipmentRate: 0,
  appearanceRate: 0
});

const createEmptyBossPositionStats = (position: number): TrialBossEquipmentPositionStats => ({
  position,
  appearanceCount: 0,
  appearanceRate: 0
});

const createEmptyBossEquipmentStats = (
  bossKey: string,
  bosses: [string, string, string]
): TrialBossEquipmentStats => ({
  bossKey,
  bosses,
  totalRecords: 0,
  equipmentCount: 0,
  positions: TRIAL_POSITIONS.map(position => createEmptyBossPositionStats(position)),
  bestEquipmentPosition: null
});

export const normalizeTrialBossSequence = (
  bosses: readonly string[] | undefined
): [string, string, string] | null => {
  if (!bosses || bosses.length < 3) {
    return null;
  }

  const normalized = bosses.slice(0, 3).map(boss => String(boss || '').trim());
  if (normalized.some(boss => !boss)) {
    return null;
  }

  return [normalized[0], normalized[1], normalized[2]];
};

export const getTrialBossSequenceKey = (bosses: readonly string[] | undefined): string | null => {
  const normalized = normalizeTrialBossSequence(bosses);
  return normalized ? normalized.join(TRIAL_BOSS_KEY_SEPARATOR) : null;
};

export const calculateTrialFlipStats = (records: TrialPlaceRecord[]): TrialFlipStatsSummary => {
  const positions = TRIAL_POSITIONS.map(position => createEmptyPositionStats(position));
  const statsMap = new Map<number, TrialFlipPositionStats>(positions.map(item => [item.position, item]));
  const bossEquipmentStatsMap = new Map<string, TrialBossEquipmentStats>();

  records.forEach(record => {
    const flippedStats = statsMap.get(record.flippedIndex);
    if (flippedStats) {
      flippedStats.flipCount += 1;
    }

    const equipmentEntries = getTrialRecordEquipmentEntries(record);
    const bossSequence = normalizeTrialBossSequence(record.bosses);
    const bossKey = getTrialBossSequenceKey(bossSequence || undefined);
    const bossEquipmentStats = bossSequence && bossKey
      ? (() => {
          const existing = bossEquipmentStatsMap.get(bossKey);
          if (existing) {
            return existing;
          }

          const created = createEmptyBossEquipmentStats(bossKey, bossSequence);
          bossEquipmentStatsMap.set(bossKey, created);
          return created;
        })()
      : null;

    if (bossEquipmentStats) {
      bossEquipmentStats.totalRecords += 1;
    }

    equipmentEntries.forEach(entry => {
      const positionStats = statsMap.get(entry.cardIndex);
      if (!positionStats) {
        return;
      }

      positionStats.appearanceCount += 1;
      if (entry.isFlipped) {
        positionStats.flippedEquipmentCount += 1;
      }

      if (bossEquipmentStats) {
        const bossPositionStats = bossEquipmentStats.positions.find(item => item.position === entry.cardIndex);
        if (bossPositionStats) {
          bossPositionStats.appearanceCount += 1;
          bossEquipmentStats.equipmentCount += 1;
        }
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

  const bossEquipmentStats = Array.from(bossEquipmentStatsMap.values())
    .map(item => {
      item.positions.forEach(positionStats => {
        positionStats.appearanceRate =
          item.totalRecords > 0
            ? positionStats.appearanceCount / item.totalRecords
            : 0;
      });

      item.bestEquipmentPosition = item.positions.reduce<TrialBossEquipmentPositionStats | null>((best, current) => {
        if (current.appearanceCount === 0) {
          return best;
        }
        if (!best || current.appearanceCount > best.appearanceCount) {
          return current;
        }
        if (best && current.appearanceCount === best.appearanceCount && current.position < best.position) {
          return current;
        }
        return best;
      }, null);

      return item;
    })
    .filter(item => item.equipmentCount > 0)
    .sort((a, b) => {
      if (a.equipmentCount !== b.equipmentCount) {
        return b.equipmentCount - a.equipmentCount;
      }
      if (a.totalRecords !== b.totalRecords) {
        return b.totalRecords - a.totalRecords;
      }
      return a.bosses.join('').localeCompare(b.bosses.join(''), 'zh-CN');
    });

  return {
    totalRecords,
    positions,
    bestFlipPosition,
    bestAppearancePosition,
    bossEquipmentStats
  };
};

export const findTrialBossEquipmentStats = (
  records: TrialPlaceRecord[],
  bosses: readonly string[]
): TrialBossEquipmentStats | null => {
  const bossKey = getTrialBossSequenceKey(bosses);
  if (!bossKey) {
    return null;
  }

  return calculateTrialFlipStats(records).bossEquipmentStats.find(item => item.bossKey === bossKey) || null;
};
