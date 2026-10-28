import { Season, TrialPlaceRecord } from '../types';

export interface TrialEquipmentLevelRange {
  min: number;
  max: number;
}

/** 赛季是否已配置合法的试炼装备品级范围。 */
export const getTrialEquipmentLevelRange = (
  season: Pick<Season, 'trialEquipLevelMin' | 'trialEquipLevelMax'> | null | undefined
): TrialEquipmentLevelRange | null => {
  const min = season?.trialEquipLevelMin;
  const max = season?.trialEquipLevelMax;
  if (typeof min !== 'number' || typeof max !== 'number'
    || !Number.isInteger(min) || !Number.isInteger(max) || min <= 0 || max < min) {
    return null;
  }
  return { min, max };
};

/**
 * 从实际装备数据推导品级范围。
 *
 * 新赛季上线时尚未配置赛季品级范围，此时按同步到的装备自身品级推导，
 * 避免回退到旧赛季范围或写死默认值。
 */
export const deriveTrialEquipmentLevelRange = (
  levels: readonly (number | null | undefined)[]
): TrialEquipmentLevelRange | null => {
  const valid = levels.filter(
    (level): level is number => typeof level === 'number' && Number.isFinite(level) && level > 0
  );
  if (valid.length === 0) {
    return null;
  }
  return { min: Math.min(...valid), max: Math.max(...valid) };
};

const TRIAL_CARD_KEYS = ['card1', 'card2', 'card3', 'card4', 'card5'] as const;
const LEGACY_TRIAL_CARD_KEYS = ['card_1', 'card_2', 'card_3', 'card_4', 'card_5'] as const;

type TrialCardKey = typeof TRIAL_CARD_KEYS[number];
type LegacyTrialCardKey = typeof LEGACY_TRIAL_CARD_KEYS[number];
type TrialRecordLike = TrialPlaceRecord & Partial<Record<LegacyTrialCardKey, string | number>>;

export interface TrialRecordEquipmentEntry {
  cardIndex: number;
  equipId: string;
  isFlipped: boolean;
}

const getCardValue = (record: TrialRecordLike, cardIndex: number): string => {
  const modernKey = `card${cardIndex}` as TrialCardKey;
  const legacyKey = `card_${cardIndex}` as LegacyTrialCardKey;
  const modernValue = record[modernKey];
  const legacyValue = record[legacyKey];

  if (modernValue !== undefined && modernValue !== null) {
    const normalizedModernValue = String(modernValue).trim();
    if (normalizedModernValue) {
      return normalizedModernValue;
    }
  }

  if (legacyValue === undefined || legacyValue === null) {
    return '';
  }

  return String(legacyValue).trim();
};

export const getTrialRecordEquipmentEntries = (record: TrialRecordLike): TrialRecordEquipmentEntry[] => {
  const cardEntries = TRIAL_CARD_KEYS
    .map((_, index) => {
      const cardIndex = index + 1;
      return {
        cardIndex,
        equipId: getCardValue(record, cardIndex),
        isFlipped: cardIndex === record.flippedIndex
      };
    })
    .filter((entry): entry is TrialRecordEquipmentEntry => Boolean(entry.equipId));

  if (cardEntries.length === 0) {
    return [];
  }

  const flippedEntryIndex = cardEntries.findIndex(entry => entry.cardIndex === record.flippedIndex);
  if (flippedEntryIndex <= 0) {
    return cardEntries;
  }

  const flippedEntry = cardEntries[flippedEntryIndex];
  const otherEntries = cardEntries.filter((_, index) => index !== flippedEntryIndex);
  return [flippedEntry, ...otherEntries];
};

export const getTrialRecordEquipmentIds = (record: TrialRecordLike): string[] => {
  return getTrialRecordEquipmentEntries(record).map(entry => entry.equipId);
};
