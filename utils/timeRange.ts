/**
 * 时间范围工具函数
 */

import { TimeRangeType, TimeRange } from '../types';

/**
 * 获取时间范围的起止时间戳（毫秒）
 */
export function getTimeRangeBounds(range: TimeRangeType): TimeRange {
  const now = Date.now();

  switch (range) {
    case 'week': {
      // 最近7天
      const start = new Date();
      start.setDate(start.getDate() - 7);
      start.setHours(0, 0, 0, 0);
      return { start: start.getTime(), end: now };
    }
    case 'month': {
      // 最近30天
      const start = new Date();
      start.setDate(start.getDate() - 30);
      start.setHours(0, 0, 0, 0);
      return { start: start.getTime(), end: now };
    }
    case 'all':
    default:
      // 使用当前时间 + 10 年作为结束时间，覆盖所有历史数据
      // 注意：不能使用 i64::MAX，因为会超出 JavaScript 的 Number.MAX_SAFE_INTEGER 导致精度丢失
      return { start: 0, end: Date.now() + 10 * 365 * 24 * 60 * 60 * 1000 };
  }
}

/**
 * 格式化时间戳为日期时间字符串
 * @param ms 毫秒时间戳
 * @returns 格式化后的字符串 (MM-DD HH:mm)
 */
export function formatDateTime(ms: number): string {
  if (!ms || ms === 0) return '--';
  const date = new Date(ms);
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  return `${month}-${day} ${hours}:${minutes}`;
}

/**
 * 格式化时间戳为时间字符串
 * @param ms 毫秒时间戳
 * @returns 格式化后的字符串 (HH:mm)
 */
export function formatTime(ms: number): string {
  if (!ms || ms === 0) return '--';
  const date = new Date(ms);
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  return `${hours}:${minutes}`;
}

/**
 * 格式化时间戳为完整日期字符串
 * @param ms 毫秒时间戳
 * @returns 格式化后的字符串 (YYYY-MM-DD HH:mm)
 */
export function formatFullDateTime(ms: number): string {
  if (!ms || ms === 0) return '--';
  const date = new Date(ms);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  return `${year}-${month}-${day} ${hours}:${minutes}`;
}

/**
 * 判断两个时间范围是否有交集
 */
export function hasTimeIntersection(
  start1: number,
  end1: number,
  start2: number,
  end2: number
): boolean {
  return start1 <= end2 && end1 >= start2;
}

/**
 * 获取时间范围描述文本
 */
export function getTimeRangeLabel(range: TimeRangeType): string {
  switch (range) {
    case 'week':
      return '最近一周';
    case 'month':
      return '最近一个月';
    case 'all':
      return '全部';
    default:
      return '未知';
  }
}
