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
type TrialCardRecordLike = Pick<TrialPlaceRecord,
  'card1' | 'card2' | 'card3' | 'card4' | 'card5' | 'flippedIndex'
> & Partial<Record<LegacyTrialCardKey, string | number>>;
type TrialRecordLike = TrialPlaceRecord & TrialCardRecordLike;

/**
 * 一次可靠入包对应一次翻牌。父记录只描述一次通关；新记录把每次翻牌放在
 * `draws` 内，避免连续翻牌时把父记录的旧字段重复计入统计。
 */
export interface TrialFlipSample {
  card1: string;
  card2: string;
  card3: string;
  card4: string;
  card5: string;
  flippedIndex: number;
  /**
   * 日志收据与手工补录同时存在时，不能根据顺序猜测二者的对应关系。
   * 缺失来源的历史子项沿用自动收据兼容行为。
   */
  source?: 'manual' | 'auto';
}

type TrialRecordWithDraws = TrialRecordLike & {
  draws?: TrialFlipSample[];
};

/**
 * 旧版手工记录没有状态字段，按已确认记录兼容。
 * 自动扫描产生的待确认记录不参与正式进度与翻牌统计。
 */
export const isConfirmedTrialRecord = (record: Pick<TrialPlaceRecord, 'status'>): boolean => {
  return record.status !== 'pending';
};

/** 只有已确认且明确知道翻牌位的记录才是可统计的翻牌样本。 */
export const hasRecordedTrialFlip = (record: Pick<TrialPlaceRecord, 'status' | 'flippedIndex'>): boolean => {
  return isConfirmedTrialRecord(record)
    && Number.isInteger(record.flippedIndex)
    && record.flippedIndex >= 1
    && record.flippedIndex <= TRIAL_CARD_KEYS.length;
};

/** 翻牌样本本身不带状态，确认状态由其父通关记录统一控制。 */
export const hasRecordedTrialFlipSample = (sample: Pick<TrialFlipSample, 'flippedIndex'>): boolean => {
  return Number.isInteger(sample.flippedIndex)
    && sample.flippedIndex >= 1
    && sample.flippedIndex <= TRIAL_CARD_KEYS.length;
};

/**
 * 返回一轮通关中可统计的翻牌样本。
 *
 * - 有逐次入包子项时只认子项；
 * - 同一轮同时有日志收据和待归属手工子项时，仅日志收据进入统计，
 *   防止一笔实际翻牌被重复计数；手工子项保留给详情页核对；
 * - 没有子项时，兼容旧版父字段中已明确的单次翻牌（包括早期自动记录）；
 *   新自动记录的父翻牌位默认是 0，因此不会凭空生成翻牌；
 * - 有子项时绝不同时返回父字段，避免首张牌被重复计数。
 */
export const getTrialRecordFlipSamples = (record: TrialRecordLike): TrialFlipSample[] => {
  if (!isConfirmedTrialRecord(record)) {
    return [];
  }

  const draws = (record as TrialRecordWithDraws).draws;
  if (Array.isArray(draws)) {
    if (draws.length > 0) {
      const autoDraws = draws.filter(draw => draw.source !== 'manual');
      return autoDraws.length > 0 ? autoDraws : draws;
    }
  }

  return hasRecordedTrialFlip(record) ? [record] : [];
};

export interface TrialRecordEquipmentEntry {
  cardIndex: number;
  equipId: string;
  isFlipped: boolean;
}

const getCardValue = (record: TrialCardRecordLike, cardIndex: number): string => {
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

/** 返回某张牌已记录的装备 ID。无效牌位不会映射到任意卡片。 */
export const getTrialCardEquipmentId = (record: TrialCardRecordLike, cardIndex: number): string => {
  if (!Number.isInteger(cardIndex) || cardIndex < 1 || cardIndex > TRIAL_CARD_KEYS.length) {
    return '';
  }
  return getCardValue(record, cardIndex);
};

/**
 * 是否已经观察到翻中的装备。
 *
 * 这只表示“看到了翻中装备”，不能与没有装备的记录一起构成无偏概率样本。
 */
export const hasObservedTrialFlippedEquipment = (record: TrialRecordLike): boolean => {
  return getTrialRecordFlipSamples(record).some(sample => (
    hasRecordedTrialFlipSample(sample)
      && Boolean(getTrialCardEquipmentId(sample, sample.flippedIndex))
  ));
};

/** 只有已确认牌位的翻牌才可用于翻中率、位置出现率和 Boss 出装备统计。 */
export const isTrialEquipmentAppearanceReviewed = (record: TrialRecordLike): boolean => {
  return getTrialRecordFlipSamples(record).some(sample => isTrialFlipSampleEquipmentAppearanceReviewed(sample));
};

/** 只有已确认牌位的单次翻牌才可进入概率统计。 */
export const isTrialFlipSampleEquipmentAppearanceReviewed = (sample: TrialFlipSample): boolean => {
  return hasRecordedTrialFlipSample(sample);
};

export const getTrialRecordEquipmentEntries = (record: TrialCardRecordLike): TrialRecordEquipmentEntry[] => {
  const cardEntries = TRIAL_CARD_KEYS
    .map((_, index) => {
      const cardIndex = index + 1;
      return {
        cardIndex,
        equipId: getTrialCardEquipmentId(record, cardIndex),
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
