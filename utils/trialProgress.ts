import { GameVersion, Season, TrialPlaceRecord } from '../types';

export interface TrialProgressPeriod {
  start: number;
  end?: number;
}

const DEFAULT_TRIAL_LAYER = 1;

/** 将秒、毫秒或 ISO 日期统一成毫秒时间戳。 */
export const toTrialProgressTimestamp = (value: string | number): number | null => {
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) return null;
    const timestamp = Math.abs(value) < 100_000_000_000 ? value * 1000 : value;
    return Number.isFinite(timestamp) ? timestamp : null;
  }

  const timestamp = new Date(value).getTime();
  return Number.isFinite(timestamp) ? timestamp : null;
};

/**
 * 试炼进度以大版本为周期：从该版本最早赛季开始，到后续大版本最早赛季开始为止。
 * 赛季内部切换不切断进度；结束边界为半开区间，防止交界记录同时归属两个版本。
 */
export const getTrialProgressPeriod = (
  currentSeason: Season | null | undefined,
  seasons: readonly Season[],
  versions: readonly GameVersion[]
): TrialProgressPeriod | null => {
  const currentVersionId = currentSeason?.versionId;
  if (!Number.isInteger(currentVersionId)) return null;

  const currentVersion = versions.find(version => version.id === currentVersionId);
  if (!currentVersion || !Number.isFinite(currentVersion.sortOrder)) return null;

  const getSeasonStart = (season: Season): number | null => {
    const start = toTrialProgressTimestamp(season.startDate);
    return start !== null && start > 0 ? start : null;
  };

  const currentStarts = seasons
    .filter(season => season.versionId === currentVersionId)
    .map(getSeasonStart)
    .filter((start): start is number => start !== null);
  if (currentStarts.length === 0) return null;

  const start = Math.min(...currentStarts);
  const laterVersionIds = new Set(
    versions
      .filter(version => Number.isFinite(version.sortOrder) && version.sortOrder > currentVersion.sortOrder)
      .map(version => version.id)
      .filter((id): id is number => Number.isInteger(id))
  );
  const futureStarts = seasons
    .filter(season => laterVersionIds.has(season.versionId))
    .map(getSeasonStart)
    .filter((candidate): candidate is number => candidate !== null && candidate > start);
  const end = futureStarts.length > 0 ? Math.min(...futureStarts) : undefined;

  return end === undefined ? { start } : { start, end };
};

export const isTrialRecordInProgressPeriod = (
  record: Pick<TrialPlaceRecord, 'date'>,
  period: TrialProgressPeriod | null | undefined
): boolean => {
  if (!period) return false;
  const timestamp = toTrialProgressTimestamp(record.date);
  if (timestamp === null || timestamp < period.start) return false;
  return period.end === undefined || timestamp < period.end;
};

export const getLatestTrialRecordForRole = (
  records: readonly TrialPlaceRecord[],
  roleId: string,
  period: TrialProgressPeriod | null | undefined,
  minimumTimestamp?: number
): TrialPlaceRecord | undefined => {
  if (!period || !Number.isFinite(minimumTimestamp ?? 0)) return undefined;

  let latest: TrialPlaceRecord | undefined;
  let latestTimestamp = Number.NEGATIVE_INFINITY;
  for (const record of records) {
    if (record.roleId !== roleId || !isTrialRecordInProgressPeriod(record, period)) continue;
    const timestamp = toTrialProgressTimestamp(record.date);
    if (timestamp === null || (minimumTimestamp !== undefined && timestamp < minimumTimestamp)) continue;
    if (timestamp > latestTimestamp) {
      latest = record;
      latestTimestamp = timestamp;
    }
  }
  return latest;
};

export const getTrialLayerForRole = (
  records: readonly TrialPlaceRecord[],
  roleId: string,
  period: TrialProgressPeriod | null | undefined
): number => {
  const latest = getLatestTrialRecordForRole(records, roleId, period);
  return latest && Number.isFinite(latest.layer) && latest.layer >= DEFAULT_TRIAL_LAYER
    ? latest.layer
    : DEFAULT_TRIAL_LAYER;
};
