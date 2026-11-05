import type { TrialDraw, TrialPlaceRecord } from '../types';
import { TrialProgressPeriod, toTrialProgressTimestamp } from './trialProgress';
import { trialRecordNeedsReview } from './trialReviewState';

export interface TrialWeeklyDrawProgress {
  /** 本周当前大版本内已观察到的翻牌次数；一次通关可以有多次。 */
  drawCount: number;
  /** 需要用户处理的通关轮次数；同一轮无论有几项问题只计一次。 */
  pendingRunCount: number;
  /** 因手工翻牌没有日志收据时间，按父通关日期归入本周的次数。 */
  manualDateFallbackDrawCount: number;
}

export interface TrialWeeklyDrawProgressOptions {
  /** 游戏周起点，含该时刻。 */
  weekStart: Date | number;
  /** 当前试炼大版本的半开时间范围。 */
  progressPeriod: TrialProgressPeriod | null | undefined;
}

interface TimedTrialDraw {
  draw: Pick<TrialDraw, 'flippedIndex' | 'equipmentUnreviewed'>;
  timestamp: number | null;
  usedManualDateFallback: boolean;
}

/** 收据时间有时从 SQLite 以数字字符串返回，先按数值解析再处理秒/毫秒。 */
const toReceiptTimestamp = (value: string | number): number | null => {
  if (typeof value === 'string') {
    const normalized = value.trim();
    if (/^[+-]?\d+(?:\.\d+)?$/.test(normalized)) {
      return toTrialProgressTimestamp(Number(normalized));
    }
  }
  return toTrialProgressTimestamp(value);
};

const isKnownFlippedIndex = (flippedIndex: number): boolean => (
  Number.isInteger(flippedIndex) && flippedIndex >= 1 && flippedIndex <= 5
);

const isTimestampInProgressPeriod = (
  timestamp: number | null,
  period: TrialProgressPeriod | null | undefined,
): timestamp is number => {
  if (timestamp === null || !period || timestamp < period.start) return false;
  return period.end === undefined || timestamp < period.end;
};

/**
 * 返回同一轮应参与自动统计的子笔。
 *
 * 自动收据与手工补录同时存在时，手工子笔还没有可靠归属，不能和日志收据
 * 相加；只统计自动收据，待确认状态仍由父轮统一保留。
 */
const getSelectedDraws = (record: TrialPlaceRecord): TrialDraw[] => {
  const draws = Array.isArray(record.draws) ? record.draws : [];
  if (draws.length === 0) return [];

  const automaticDraws = draws.filter(draw => draw.source !== 'manual');
  return automaticDraws.length > 0 ? automaticDraws : draws;
};

const getTimedDraws = (record: TrialPlaceRecord): TimedTrialDraw[] => {
  const selectedDraws = getSelectedDraws(record);
  const parentTimestamp = toTrialProgressTimestamp(record.date);

  if (selectedDraws.length > 0) {
    return selectedDraws.map(draw => {
      // 手工翻牌没有可信的收据时间，即使历史草稿曾写入值也按父通关日期归类。
      const usedManualDateFallback = draw.source === 'manual';
      const receiptTimestamp = usedManualDateFallback ? null : toReceiptTimestamp(draw.receiptTime);
      return {
        draw,
        // 自动子笔没有可信收据时不能回退父日期，否则会伪造本周归属。
        timestamp: usedManualDateFallback ? parentTimestamp : receiptTimestamp,
        usedManualDateFallback,
      };
    });
  }

  // 旧记录没有子笔时，只有已知父牌位才可兼容为一笔真实翻牌。
  if (!isKnownFlippedIndex(record.flippedIndex)) return [];
  return [{
    draw: record,
    timestamp: parentTimestamp,
    usedManualDateFallback: false,
  }];
};

const toWeekBoundaryTimestamp = (value: Date | number): number | null => {
  const timestamp = typeof value === 'number' ? value : value.getTime();
  return Number.isFinite(timestamp) ? timestamp : null;
};

const isTimestampInWeek = (timestamp: number | null, weekStart: number, weekEnd: number): timestamp is number => (
  timestamp !== null && timestamp >= weekStart && timestamp < weekEnd
);

/** 基于本地日历推导下周起点，保持游戏周切换时刻并避免 DST 的毫秒偏移。 */
export const getTrialWeekEnd = (weekStart: Date | number): number | null => {
  const weekStartTimestamp = toWeekBoundaryTimestamp(weekStart);
  if (weekStartTimestamp === null) return null;
  const nextWeekStart = new Date(weekStartTimestamp);
  nextWeekStart.setDate(nextWeekStart.getDate() + 7);
  return nextWeekStart.getTime();
};

/**
 * 详情的“本周”展示按整轮处理：父通关日期在周内，或任一可信子笔收据在周内，
 * 都展示该轮。自动子笔缺少收据不会以父日期冒充其翻牌时间；纯手工子笔按父
 * 通关日期回退，旧无子笔记录也沿用父日期。
 */
export const isTrialRecordInWeek = (
  record: TrialPlaceRecord,
  weekStart: Date | number,
  weekEnd: Date | number,
): boolean => {
  const weekStartTimestamp = toWeekBoundaryTimestamp(weekStart);
  const weekEndTimestamp = toWeekBoundaryTimestamp(weekEnd);
  if (weekStartTimestamp === null || weekEndTimestamp === null || weekEndTimestamp <= weekStartTimestamp) {
    return false;
  }

  if (isTimestampInWeek(toTrialProgressTimestamp(record.date), weekStartTimestamp, weekEndTimestamp)) {
    return true;
  }
  return getTimedDraws(record).some(({ timestamp }) => (
    isTimestampInWeek(timestamp, weekStartTimestamp, weekEndTimestamp)
  ));
};

/**
 * 统计角色本周已观察到的翻牌，而不改变通关次数的统计口径。
 *
 * 子笔优先按真实日志收据时间归属游戏周及当前大版本；手工子笔回退父通关
 * 日期并把回退次数返回给界面提示。父轮处于 pending 时，已入包的自动收据
 * 仍是已观察翻牌，因此照常计数，只将该轮标记为待确认。
 */
export const getTrialWeeklyDrawProgress = (
  records: readonly TrialPlaceRecord[],
  { weekStart, progressPeriod }: TrialWeeklyDrawProgressOptions,
): TrialWeeklyDrawProgress => {
  const weekStartTimestamp = typeof weekStart === 'number'
    ? weekStart
    : weekStart.getTime();
  const weekEndTimestamp = getTrialWeekEnd(weekStartTimestamp);
  if (!Number.isFinite(weekStartTimestamp) || weekEndTimestamp === null || !progressPeriod) {
    return { drawCount: 0, pendingRunCount: 0, manualDateFallbackDrawCount: 0 };
  }

  let drawCount = 0;
  let pendingRunCount = 0;
  let manualDateFallbackDrawCount = 0;

  for (const record of records) {
    const currentDraws = getTimedDraws(record).filter(({ timestamp }) => (
      isTimestampInProgressPeriod(timestamp, progressPeriod)
      && isTimestampInWeek(timestamp, weekStartTimestamp, weekEndTimestamp)
    ));

    drawCount += currentDraws.length;
    manualDateFallbackDrawCount += currentDraws.filter(draw => draw.usedManualDateFallback).length;

    // 无子笔的旧轮仍要显示待确认；父日期是唯一可用的归属证据。若已有
    // 自动子笔，则绝不把父日期重新用于跨周/跨版本归属。
    const parentTimestamp = toTrialProgressTimestamp(record.date);
    const hasChildDraws = getSelectedDraws(record).length > 0;
    const isCurrentLegacyReview = !hasChildDraws
      && isTimestampInProgressPeriod(parentTimestamp, progressPeriod)
      && parentTimestamp !== null
      && isTimestampInWeek(parentTimestamp, weekStartTimestamp, weekEndTimestamp);

    if ((currentDraws.length > 0 || isCurrentLegacyReview) && trialRecordNeedsReview(record)) {
      pendingRunCount += 1;
    }
  }

  return { drawCount, pendingRunCount, manualDateFallbackDrawCount };
};
