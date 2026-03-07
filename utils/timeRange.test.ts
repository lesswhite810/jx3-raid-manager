/**
 * timeRange 工具函数单元测试
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  getTimeRangeBounds,
  formatDateTime,
  formatTime,
  formatFullDateTime,
  hasTimeIntersection,
  getTimeRangeLabel,
} from './timeRange';
import { TimeRangeType } from '../types';

describe('timeRange', () => {
  // 固定当前时间为 2026-03-01 (周日) 12:00:00
  const MOCK_NOW = new Date(2026, 2, 1, 12, 0, 0).getTime();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(MOCK_NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ============================================================================
  // getTimeRangeBounds
  // ============================================================================

  describe('getTimeRangeBounds', () => {
    it('本周 - 应返回本周一 00:00:00 到当前时间', () => {
      const result = getTimeRangeBounds('week');

      // 2026-03-01 是周日，本周一是 2026-02-23
      const expectedStart = new Date(2026, 1, 23, 0, 0, 0, 0).getTime();
      expect(result.start).toBe(expectedStart);
      expect(result.end).toBe(MOCK_NOW);
    });

    it('本月 - 应返回本月1日 00:00:00 到当前时间', () => {
      const result = getTimeRangeBounds('month');

      const expectedStart = new Date(2026, 2, 1, 0, 0, 0, 0).getTime();
      expect(result.start).toBe(expectedStart);
      expect(result.end).toBe(MOCK_NOW);
    });

    it('全部 - 应返回 0 到 Infinity', () => {
      const result = getTimeRangeBounds('all');

      expect(result.start).toBe(0);
      expect(result.end).toBe(Infinity);
    });
  });

  // ============================================================================
  // formatDateTime
  // ============================================================================

  describe('formatDateTime', () => {
    it('应正确格式化日期时间 (MM-DD HH:mm)', () => {
      // 2026-03-01 12:30:45
      const ms = new Date(2026, 2, 1, 12, 30, 45).getTime();
      expect(formatDateTime(ms)).toBe('03-01 12:30');
    });

    it('应补零显示', () => {
      // 2026-01-05 08:05:00
      const ms = new Date(2026, 0, 5, 8, 5, 0).getTime();
      expect(formatDateTime(ms)).toBe('01-05 08:05');
    });

    it('空值应返回 --', () => {
      expect(formatDateTime(0)).toBe('--');
      expect(formatDateTime(null as unknown as number)).toBe('--');
      expect(formatDateTime(undefined as unknown as number)).toBe('--');
    });
  });

  // ============================================================================
  // formatTime
  // ============================================================================

  describe('formatTime', () => {
    it('应正确格式化时间 (HH:mm)', () => {
      const ms = new Date(2026, 2, 1, 14, 30, 0).getTime();
      expect(formatTime(ms)).toBe('14:30');
    });

    it('应补零显示', () => {
      const ms = new Date(2026, 2, 1, 8, 5, 0).getTime();
      expect(formatTime(ms)).toBe('08:05');
    });

    it('空值应返回 --', () => {
      expect(formatTime(0)).toBe('--');
    });
  });

  // ============================================================================
  // formatFullDateTime
  // ============================================================================

  describe('formatFullDateTime', () => {
    it('应正确格式化完整日期时间 (YYYY-MM-DD HH:mm)', () => {
      const ms = new Date(2026, 3, 15, 16, 45, 0).getTime();
      expect(formatFullDateTime(ms)).toBe('2026-04-15 16:45');
    });

    it('空值应返回 --', () => {
      expect(formatFullDateTime(0)).toBe('--');
    });
  });

  // ============================================================================
  // hasTimeIntersection
  // ============================================================================

  describe('hasTimeIntersection', () => {
    it('完全重叠应返回 true', () => {
      expect(hasTimeIntersection(100, 200, 100, 200)).toBe(true);
    });

    it('部分重叠应返回 true', () => {
      expect(hasTimeIntersection(100, 200, 150, 250)).toBe(true);
      expect(hasTimeIntersection(150, 250, 100, 200)).toBe(true);
    });

    it('边界接触应返回 true', () => {
      expect(hasTimeIntersection(100, 200, 200, 300)).toBe(true);
      expect(hasTimeIntersection(200, 300, 100, 200)).toBe(true);
    });

    it('不重叠应返回 false', () => {
      expect(hasTimeIntersection(100, 199, 200, 300)).toBe(false);
      expect(hasTimeIntersection(200, 300, 100, 199)).toBe(false);
    });

    it('包含关系应返回 true', () => {
      expect(hasTimeIntersection(100, 300, 150, 200)).toBe(true);
      expect(hasTimeIntersection(150, 200, 100, 300)).toBe(true);
    });
  });

  // ============================================================================
  // getTimeRangeLabel
  // ============================================================================

  describe('getTimeRangeLabel', () => {
    it('应返回正确的中文标签', () => {
      expect(getTimeRangeLabel('week')).toBe('本周');
      expect(getTimeRangeLabel('month')).toBe('本月');
      expect(getTimeRangeLabel('all')).toBe('全部');
    });

    it('未知类型应返回"未知"', () => {
      expect(getTimeRangeLabel('unknown' as TimeRangeType)).toBe('未知');
    });
  });
});
