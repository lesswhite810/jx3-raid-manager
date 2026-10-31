import { Season, TrialPlaceRecord } from '../types';

export interface TrialEquipmentLevelRange {
  min?: number;
  max?: number;
}

const normalizeTrialEquipmentLevelBound = (
  value: number | null | undefined
): number | undefined | null => {
  if (value === undefined || value === null || value === 0) {
    return undefined;
  }
  if (!Number.isInteger(value) || value < 0) {
    return null;
  }
  return value;
};

/**
 * 解析赛季试炼装备品级边界。
 *
 * `undefined`/0 表示对应边界尚未配置；可只配置最低或最高品级。
 * 返回 null 表示配置非法，空对象表示上下界均未配置。
 */
export const getTrialEquipmentLevelRange = (
  season: Pick<Season, 'trialEquipLevelMin' | 'trialEquipLevelMax'> | null | undefined
): TrialEquipmentLevelRange | null => {
  // `db.getCurrentSeason()` 失败时会返回 null；这不是“未配置边界”，不能
  // 降级为读取旧装备缓存。
  if (!season) {
    return null;
  }
  const min = normalizeTrialEquipmentLevelBound(season?.trialEquipLevelMin);
  const max = normalizeTrialEquipmentLevelBound(season?.trialEquipLevelMax);
  if (min === null || max === null || (min !== undefined && max !== undefined && max < min)) {
    return null;
  }
  return {
    ...(min !== undefined ? { min } : {}),
    ...(max !== undefined ? { max } : {}),
  };
};

/** 在赛季已确认的边界内保留合法装备品级。 */
export const isTrialEquipmentLevelInRange = (
  level: number | null | undefined,
  range: TrialEquipmentLevelRange
): level is number => {
  return typeof level === 'number'
    && Number.isFinite(level)
    && Number.isInteger(level)
    && level > 0
    && (range.min === undefined || level >= range.min)
    && (range.max === undefined || level <= range.max);
};

/**
 * 从实际装备数据推导品级范围。
 *
 * 新赛季上线时尚未配置赛季品级范围，此时按同步到的装备自身品级推导，
 * 避免回退到旧赛季范围或写死默认值。
 */
export const deriveTrialEquipmentLevelRange = (
  levels: readonly (number | null | undefined)[]
): Required<TrialEquipmentLevelRange> | null => {
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
