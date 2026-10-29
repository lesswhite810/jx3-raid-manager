/**
 * cooldownManager 单元测试
 *
 * 副本刷新规则：
 *   25人本：每周一 07:00 刷新（每周一个周期）
 *   10人本：每周一 07:00 和 周五 07:00 各刷新一次（每周两个周期）
 *
 * 边界测试重点：
 *   - 周一 06:59（刷新点前一分钟）
 *   - 周一 07:00（刚好刷新）
 *   - 周一 07:01（刷新后一分钟）
 *   - 周五 06:59（10人本第二刷新点前一分钟）
 *   - 周五 07:00（10人本第二刷新点）
 *   - 周五 07:01（10人本第二刷新后一分钟）
 *   - 周日 23:59（周末末尾）
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
    getLastMonday,
    getNextMonday,
    getTenPersonCycle,
    calculateCooldown,
    getSeasonStartTimeMs,
    getMonthStart,
    getMonthEnd,
    getRaidRefreshInfo,
    getRaidCooldownRules,
    formatRemainingTime,
    formatCountdown,
    logOperation,
    getOperationLogs,
    clearOperationLogs,
} from './cooldownManager';
import { Raid } from '../types';

// ─── 工具函数 ─────────────────────────────────────────────────────────────────

/**
 * 构造一个指定星期几 + 时:分 的 Date
 * dayOfWeek: 0=周日, 1=周一, ..., 6=周六（与 Date.getDay() 一致）
 * 以 2026-02-16（周一）为锚点的这一周
 */
function makeDate(dayOfWeek: 0 | 1 | 2 | 3 | 4 | 5 | 6, hour: number, minute: number = 0): Date {
    // 2026-02-16 = 周一（锚点）
    const ANCHOR_MONDAY = new Date(2026, 1, 16, 0, 0, 0, 0); // 月份从 0 开始，1=2月
    const d = new Date(ANCHOR_MONDAY);
    // dayOfWeek=0(周日) → 加 6 天；dayOfWeek=1(周一) → 加 0 天；...
    const daysOffset = dayOfWeek === 0 ? 6 : dayOfWeek - 1;
    d.setDate(ANCHOR_MONDAY.getDate() + daysOffset);
    d.setHours(hour, minute, 0, 0);
    return d;
}

/** 构造 Raid 对象 */
function makeRaid(playerCount: 10 | 25): Raid {
    return {
        id: `raid-${playerCount}`,
        name: playerCount === 25 ? '燃木京25' : '燃木京10',
        playerCount,
        difficulty: '普通',
        version: 'jx3',
        bossCount: 6,
        bosses: [],
    } as unknown as Raid;
}

/** 构造已打记录（传入 Date） */
function makeRecord(date: Date): { date: string } {
    return { date: date.toISOString() };
}


// ─── 锚点说明（2026-02-16 这周）──────────────────────────────────────────────
// 周一 = 2026-02-16
// 周四 = 2026-02-19
// 周五 = 2026-02-20
// 周日 = 2026-02-22

// ─── getLastMonday 测试 ────────────────────────────────────────────────────
describe('getLastMonday', () => {
    it('周一 07:00 → 返回当天 07:00（刚好在刷新点上）', () => {
        const now = makeDate(1, 7, 0); // 周一 07:00
        const result = getLastMonday(now);
        expect(result.getDay()).toBe(1);
        expect(result.getHours()).toBe(7);
        expect(result.getMinutes()).toBe(0);
        expect(result.toDateString()).toBe(now.toDateString());
    });

    it('周一 06:59 → 返回上周一 07:00（刷新点前一分钟，尚未刷新）', () => {
        const now = makeDate(1, 6, 59); // 周一 06:59
        const result = getLastMonday(now);
        // 应该是上周一（7 天前）
        const expectedMonday = new Date(now);
        expectedMonday.setDate(now.getDate() - 7);
        expectedMonday.setHours(7, 0, 0, 0);
        expect(result.getDay()).toBe(1);
        expect(result.toDateString()).toBe(expectedMonday.toDateString());
        expect(result.getHours()).toBe(7);
    });

    it('周一 07:01 → 返回当天 07:00（刷新点后一分钟）', () => {
        const now = makeDate(1, 7, 1);
        const result = getLastMonday(now);
        expect(result.getDay()).toBe(1);
        expect(result.toDateString()).toBe(now.toDateString());
        expect(result.getHours()).toBe(7);
    });

    it('周三 12:00 → 返回本周一 07:00', () => {
        const now = makeDate(3, 12, 0);
        const result = getLastMonday(now);
        expect(result.getDay()).toBe(1);
        expect(result.getHours()).toBe(7);
        // 比周三早 2 天
        const expectedMonday = new Date(now);
        expectedMonday.setDate(now.getDate() - 2);
        expectedMonday.setHours(7, 0, 0, 0);
        expect(result.toDateString()).toBe(expectedMonday.toDateString());
    });

    it('周五 06:59 → 返回本周一 07:00（10人本第二刷新点前）', () => {
        const now = makeDate(5, 6, 59);
        const result = getLastMonday(now);
        expect(result.getDay()).toBe(1);
        const expectedMonday = new Date(now);
        expectedMonday.setDate(now.getDate() - 4);
        expectedMonday.setHours(7, 0, 0, 0);
        expect(result.toDateString()).toBe(expectedMonday.toDateString());
    });

    it('周日 23:59 → 返回本周一 07:00', () => {
        const now = makeDate(0, 23, 59); // 周日
        const result = getLastMonday(now);
        // 周日往前 6 天是上周一
        expect(result.getDay()).toBe(1);
        expect(result.getHours()).toBe(7);
        const expectedMonday = new Date(now);
        expectedMonday.setDate(now.getDate() - 6);
        expectedMonday.setHours(7, 0, 0, 0);
        expect(result.toDateString()).toBe(expectedMonday.toDateString());
    });
});

// ─── getTenPersonCycle 测试 ───────────────────────────────────────────────────
describe('getTenPersonCycle（10人本双刷新点）', () => {
    // 上半周：周一 07:00 ~ 周五 07:00
    it('周一 07:00 → 处于上半周（起始边界）', () => {
        const now = makeDate(1, 7, 0);
        const cycle = getTenPersonCycle(now);
        expect(cycle.start.getDay()).toBe(1);   // 周一
        expect(cycle.start.getHours()).toBe(7);
        expect(cycle.end.getDay()).toBe(5);      // 周五
        expect(cycle.end.getHours()).toBe(7);
    });

    it('周三 12:00 → 处于上半周', () => {
        const now = makeDate(3, 12, 0);
        const cycle = getTenPersonCycle(now);
        expect(cycle.start.getDay()).toBe(1);
        expect(cycle.end.getDay()).toBe(5);
    });

    it('周五 06:59 → 仍处于上半周（第二刷新点前一分钟）', () => {
        const now = makeDate(5, 6, 59);
        const cycle = getTenPersonCycle(now);
        expect(cycle.start.getDay()).toBe(1);
        expect(cycle.end.getDay()).toBe(5);
        expect(cycle.end.getHours()).toBe(7);
        // 确认当前时间确实在窗口内
        expect(now < cycle.end).toBe(true);
    });

    it('周五 07:00 → 切换到下半周（第二刷新点，起始边界）', () => {
        const now = makeDate(5, 7, 0);
        const cycle = getTenPersonCycle(now);
        expect(cycle.start.getDay()).toBe(5);   // 从周五开始
        expect(cycle.start.getHours()).toBe(7);
        expect(cycle.end.getDay()).toBe(1);     // 到下周一结束
        expect(cycle.end.getHours()).toBe(7);
    });

    it('周五 07:01 → 处于下半周（第二刷新点后一分钟）', () => {
        const now = makeDate(5, 7, 1);
        const cycle = getTenPersonCycle(now);
        expect(cycle.start.getDay()).toBe(5);
        expect(cycle.end.getDay()).toBe(1);
    });

    it('周六 12:00 → 处于下半周', () => {
        const now = makeDate(6, 12, 0);
        const cycle = getTenPersonCycle(now);
        expect(cycle.start.getDay()).toBe(5);
        expect(cycle.end.getDay()).toBe(1);
    });

    it('周日 23:59 → 处于下半周末尾（下周一 07:00 前）', () => {
        const now = makeDate(0, 23, 59);
        const cycle = getTenPersonCycle(now);
        expect(cycle.start.getDay()).toBe(5);
        expect(cycle.end.getDay()).toBe(1);
        expect(now < cycle.end).toBe(true); // 仍在下半周窗口内
    });

    it('下一周的周一 06:59 → 仍属于上周下半周（刷新点前）', () => {
        const now = makeDate(1, 6, 59); // 周一 06:59，刷新点前
        const cycle = getTenPersonCycle(now);
        // getLastMonday 会返回上周一，配合推算出上周五为 start
        expect(cycle.start.getDay()).toBe(5); // 上周五
        expect(cycle.end.getDay()).toBe(1);   // 本周一 07:00
    });
});

// ─── calculateCooldown 测试 - 25人本 ─────────────────────────────────────────
describe('calculateCooldown - 25人本（每周一 07:00 刷新）', () => {
    const raid25 = makeRaid(25);

    it('本周期无记录 → 可添加', () => {
        const now = makeDate(3, 12, 0); // 周三 12:00
        const result = calculateCooldown(raid25, [], now);
        expect(result.canAdd).toBe(true);
    });

    it('本周一 07:00 后有记录 → 不可添加（CD 中）', () => {
        const now = makeDate(3, 12, 0); // 周三 12:00
        // 周二打的记录
        const record = makeRecord(makeDate(2, 20, 0));
        const result = calculateCooldown(raid25, [record], now);
        expect(result.canAdd).toBe(false);
        expect(result.cooldownType).toBe('weekly');
    });

    it('上周记录不影响本周 → 可添加', () => {
        const now = makeDate(3, 12, 0); // 周三 12:00
        // 上周四打的记录（7天前）
        const lastWeekDate = new Date(makeDate(4, 20, 0));
        lastWeekDate.setDate(lastWeekDate.getDate() - 7);
        const record = makeRecord(lastWeekDate);
        const result = calculateCooldown(raid25, [record], now);
        expect(result.canAdd).toBe(true);
    });

    it('边界：周一 06:59 时上周有记录 → 上周期 CD 尚未解除，可添加视为新周期', () => {
        // 周一 06:59 时，getLastMonday 返回上周一，所以"本周期"是上周一07:00~本周一07:00
        // 若上周有记录，则 CD 中
        const now = makeDate(1, 6, 59);
        // 上周三打的记录
        const lastWed = new Date(makeDate(3, 15, 0));
        lastWed.setDate(lastWed.getDate() - 7);
        const record = makeRecord(lastWed);
        const result = calculateCooldown(raid25, [record], now);
        // 上周期（上周一07:00 ~ 本周一07:00）内有记录 → 不可添加
        expect(result.canAdd).toBe(false);
    });

    it('边界：周一 07:00 时上周记录已过期 → 可添加', () => {
        const now = makeDate(1, 7, 0); // 周一 07:00 刷新点
        // 上周三打的记录
        const lastWed = new Date(makeDate(3, 15, 0));
        lastWed.setDate(lastWed.getDate() - 7);
        const record = makeRecord(lastWed);
        const result = calculateCooldown(raid25, [record], now);
        // 新周期刚开始，上周记录不在 [本周一07:00, 下周一07:00) 范围 → 可添加
        expect(result.canAdd).toBe(true);
    });

    it('边界：周一 07:01 有本周一 07:00 后的记录 → 不可添加', () => {
        const now = makeDate(1, 7, 1);
        const record = makeRecord(makeDate(1, 7, 0)); // 恰好在周一 07:00 打
        const result = calculateCooldown(raid25, [record], now);
        expect(result.canAdd).toBe(false);
    });

    it('nextAvailableTime 应指向下周一 07:00', () => {
        const now = makeDate(3, 12, 0);
        const record = makeRecord(makeDate(2, 20, 0));
        const result = calculateCooldown(raid25, [record], now);
        expect(result.nextAvailableTime).not.toBeNull();
        expect(result.nextAvailableTime!.getDay()).toBe(1);   // 下周一
        expect(result.nextAvailableTime!.getHours()).toBe(7);
    });

    it('部分通关（配置了bosses且击杀数小于总数） → 应当视为可打（未全通）', () => {
        const raidWithBosses: Raid = { ...raid25, bosses: [{ id: 'b1', name: 'Boss 1', order: 1 }, { id: 'b2', name: 'Boss 2', order: 2 }, { id: 'b3', name: 'Boss 3', order: 3 }] };
        const now = makeDate(3, 12, 0); // 本周三
        const record = { date: makeDate(2, 20, 0).toISOString(), bossIds: ['b1', 'b2'] }; // 只打了2个Boss
        const result = calculateCooldown(raidWithBosses, [record], now);

        expect(result.canAdd).toBe(true);
        expect(result.hasRecordInCurrentCycle).toBe(false);
        expect(result.message).toBe('本周期可继续打剩余Boss');
    });

    it('全部通关（配置了bosses且击杀数等于或大于总数） → 应当视为已CD（全通）', () => {
        const raidWithBosses: Raid = { ...raid25, bosses: [{ id: 'b1', name: 'Boss 1', order: 1 }, { id: 'b2', name: 'Boss 2', order: 2 }] };
        const now = makeDate(3, 12, 0); // 本周三
        const record1 = { date: makeDate(2, 20, 0).toISOString(), bossIds: ['b1'] };
        const record2 = { date: makeDate(2, 21, 0).toISOString(), bossId: 'b2' }; // 测试拼凑起来能满
        const result = calculateCooldown(raidWithBosses, [record1, record2], now);

        expect(result.canAdd).toBe(true); // 业务逻辑里目前对于 boss 配置的情况，直接返回 canAdd = true 走分配路线
        expect(result.hasRecordInCurrentCycle).toBe(true);
        expect(result.message).toBe('本周期已全通（可继续分配Boss记录）');
    });
});

// ─── calculateCooldown 测试 - 10人本 ─────────────────────────────────────────
describe('calculateCooldown - 10人本（周一/周五 07:00 双刷新）', () => {
    const raid10 = makeRaid(10);

    // ── 上半周（周一 07:00 ~ 周五 07:00）────────────────────────────────────

    it('上半周无记录 → 可添加', () => {
        const now = makeDate(3, 12, 0); // 周三 12:00
        const result = calculateCooldown(raid10, [], now);
        expect(result.canAdd).toBe(true);
    });

    it('上半周已有记录 → 不可添加（CD 中，到周五 07:00 刷新）', () => {
        const now = makeDate(3, 12, 0);
        const record = makeRecord(makeDate(2, 10, 0)); // 周二打的
        const result = calculateCooldown(raid10, [record], now);
        expect(result.canAdd).toBe(false);
        expect(result.cooldownType).toBe('biweekly');
        // nextAvailableTime 应为本周五 07:00
        expect(result.nextAvailableTime!.getDay()).toBe(5);
        expect(result.nextAvailableTime!.getHours()).toBe(7);
    });

    it('边界：周五 06:59，上半周有记录 → 不可添加（还差 1 分钟刷新）', () => {
        const now = makeDate(5, 6, 59);
        const record = makeRecord(makeDate(2, 10, 0)); // 周二打的
        const result = calculateCooldown(raid10, [record], now);
        expect(result.canAdd).toBe(false);
    });

    it('边界：周五 07:00，上半周记录已过期 → 可添加（新半周开始）', () => {
        const now = makeDate(5, 7, 0); // 周五刷新点
        // 上半周（周一07:00~周五07:00）的记录
        const record = makeRecord(makeDate(2, 10, 0)); // 周二
        const result = calculateCooldown(raid10, [record], now);
        // 当前周期变为下半周（周五07:00~下周一07:00），周二记录不在其中
        expect(result.canAdd).toBe(true);
    });

    it('边界：周五 07:01，下半周无记录 → 可添加', () => {
        const now = makeDate(5, 7, 1);
        // 只有上半周的记录
        const record = makeRecord(makeDate(2, 10, 0));
        const result = calculateCooldown(raid10, [record], now);
        expect(result.canAdd).toBe(true);
    });

    // ── 下半周（周五 07:00 ~ 下周一 07:00）──────────────────────────────────

    it('下半周有记录 → 不可添加（CD 中，到下周一 07:00 刷新）', () => {
        const now = makeDate(6, 15, 0); // 周六 15:00
        const record = makeRecord(makeDate(5, 9, 0)); // 周五 09:00 打
        const result = calculateCooldown(raid10, [record], now);
        expect(result.canAdd).toBe(false);
        // nextAvailableTime 应为下周一 07:00
        expect(result.nextAvailableTime!.getDay()).toBe(1);
        expect(result.nextAvailableTime!.getHours()).toBe(7);
    });

    it('下半周无记录，即使上半周有记录 → 可添加', () => {
        const now = makeDate(6, 15, 0); // 周六
        const record = makeRecord(makeDate(2, 10, 0)); // 上半周（周二）的记录
        const result = calculateCooldown(raid10, [record], now);
        expect(result.canAdd).toBe(true);
    });

    it('边界：周一 06:59，下半周（上周五~本周一）有记录 → 不可添加', () => {
        const now = makeDate(1, 6, 59);
        // 上周六打的（在下半周窗口内）
        const lastSat = new Date(makeDate(6, 15, 0));
        lastSat.setDate(lastSat.getDate() - 7);
        const record = makeRecord(lastSat);
        const result = calculateCooldown(raid10, [record], now);
        // 本周一 06:59 时，周期为上周五~本周一07:00，上周六记录在内 → 不可添加
        expect(result.canAdd).toBe(false);
    });

    it('边界：周一 07:00，下半周记录已过期 → 新上半周开始，可添加', () => {
        const now = makeDate(1, 7, 0); // 周一 07:00 刷新
        const lastSat = new Date(makeDate(6, 15, 0));
        lastSat.setDate(lastSat.getDate() - 7);
        const record = makeRecord(lastSat);
        const result = calculateCooldown(raid10, [record], now);
        expect(result.canAdd).toBe(true);
    });

    it('同一半周内已有记录，周五 07:01 后添加新记录应仍不可二次添加', () => {
        const now = makeDate(5, 8, 0); // 周五 08:00（下半周）
        const record = makeRecord(makeDate(5, 7, 30)); // 周五 07:30 打
        const result = calculateCooldown(raid10, [record], now);
        expect(result.canAdd).toBe(false);
    });

    it('cooldownType 为 biweekly', () => {
        const now = makeDate(3, 12, 0);
        const record = makeRecord(makeDate(2, 10, 0));
        const result = calculateCooldown(raid10, [record], now);
        expect(result.cooldownType).toBe('biweekly');
    });
});

// ─── getNextMonday 测试 ────────────────────────────────────────────────────
describe('getNextMonday', () => {
    it('周三 → 返回本周日后的周一（即下周一 07:00）', () => {
        const now = makeDate(3, 12, 0);
        const result = getNextMonday(now);
        expect(result.getDay()).toBe(1);
        expect(result.getHours()).toBe(7);
        // 下周一 = 本周一 + 7天
        const lastMonday = getLastMonday(now);
        const expectedNext = new Date(lastMonday);
        expectedNext.setDate(lastMonday.getDate() + 7);
        expect(result.getTime()).toBe(expectedNext.getTime());
    });

    it('周一 06:59 → 返回本周一 07:00（上一个周期是上周，下一个就是今天 07:00）', () => {
        const now = makeDate(1, 6, 59);
        const result = getNextMonday(now);
        expect(result.getDay()).toBe(1);
        expect(result.toDateString()).toBe(now.toDateString()); // 就是今天
        expect(result.getHours()).toBe(7);
    });

    it('周一 07:00 → 返回下周一 07:00', () => {
        const now = makeDate(1, 7, 0);
        const result = getNextMonday(now);
        expect(result.getDay()).toBe(1);
        const expectedNext = new Date(now);
        expectedNext.setDate(now.getDate() + 7);
        expectedNext.setHours(7, 0, 0, 0);
        expect(result.getTime()).toBe(expectedNext.getTime());
    });
});

// ─── getSeasonStartTimeMs 测试（秒级 / 毫秒级双格式）───────────────────────────
describe('getSeasonStartTimeMs（赛季起始时间双格式兼容）', () => {
    // 2026-02-16 08:00:00.000（本地时区），用作同一时间点的基准
    const SAME_INSTANT = new Date(2026, 1, 16, 8, 0, 0, 0).getTime();
    const AS_SECONDS = Math.floor(SAME_INSTANT / 1000);
    const AS_MILLIS = SAME_INSTANT;

    it('秒级数字 → 乘以 1000 得到毫秒时间戳', () => {
        expect(getSeasonStartTimeMs(AS_SECONDS)).toBe(AS_MILLIS);
    });

    it('毫秒级数字 → 原样返回', () => {
        expect(getSeasonStartTimeMs(AS_MILLIS)).toBe(AS_MILLIS);
    });

    it('同一时间点的秒级与毫秒级输入结果一致', () => {
        const fromSeconds = getSeasonStartTimeMs(AS_SECONDS);
        const fromMillis = getSeasonStartTimeMs(AS_MILLIS);
        expect(fromSeconds).toBe(fromMillis);
        expect(fromSeconds).toBe(SAME_INSTANT);
    });

    it('秒级输入换算后与 new Date(秒*1000) 等价', () => {
        expect(getSeasonStartTimeMs(AS_SECONDS)).toBe(new Date(AS_SECONDS * 1000).getTime());
    });

    it('ISO 字符串 → 解析为毫秒时间戳', () => {
        const iso = new Date(SAME_INSTANT).toISOString();
        expect(getSeasonStartTimeMs(iso)).toBe(SAME_INSTANT);
    });

    it('边界：0 视为合法秒级时间戳（1970-01-01 00:00:00）', () => {
        expect(getSeasonStartTimeMs(0)).toBe(0);
    });

    it('边界：undefined → 返回 null', () => {
        expect(getSeasonStartTimeMs(undefined)).toBeNull();
    });

    it('边界：NaN / Infinity 等非有限数字 → 返回 null', () => {
        expect(getSeasonStartTimeMs(Number.NaN)).toBeNull();
        expect(getSeasonStartTimeMs(Number.POSITIVE_INFINITY)).toBeNull();
        expect(getSeasonStartTimeMs(Number.NEGATIVE_INFINITY)).toBeNull();
    });

    it('边界：非法字符串 → 返回 null（不抛出）', () => {
        expect(getSeasonStartTimeMs('not-a-date')).toBeNull();
        expect(getSeasonStartTimeMs('')).toBeNull();
    });

    it('1e12 边界：刚好等于阈值时按毫秒级处理', () => {
        // 实现用 `> 1e12` 判断，1e12 本身走秒级分支（*1000）
        expect(getSeasonStartTimeMs(1e12)).toBe(1e15);
        expect(getSeasonStartTimeMs(1e12 + 1)).toBe(1e12 + 1);
    });
});

// ─── getMonthStart / getMonthEnd 测试 ────────────────────────────────────────
describe('getMonthStart / getMonthEnd', () => {
    it('getMonthStart → 本月 1 日 00:00:00.000', () => {
        const now = makeDate(3, 12, 34); // 周三 12:34
        const result = getMonthStart(now);
        expect(result.getDate()).toBe(1);
        expect(result.getHours()).toBe(0);
        expect(result.getMinutes()).toBe(0);
        expect(result.getSeconds()).toBe(0);
        expect(result.getMilliseconds()).toBe(0);
        expect(result.getMonth()).toBe(now.getMonth());
    });

    it('getMonthEnd → 下月 1 日 00:00:00.000', () => {
        const now = makeDate(3, 12, 0);
        const result = getMonthEnd(now);
        const expected = new Date(now.getFullYear(), now.getMonth() + 1, 1, 0, 0, 0, 0);
        expect(result.getTime()).toBe(expected.getTime());
    });

    it('getMonthEnd 跨年：12 月 → 次年 1 月 1 日', () => {
        const dec = new Date(2026, 11, 15, 10, 0, 0, 0);
        const result = getMonthEnd(dec);
        expect(result.getFullYear()).toBe(2027);
        expect(result.getMonth()).toBe(0);
        expect(result.getDate()).toBe(1);
    });

    it('getMonthStart 不修改入参（返回新 Date）', () => {
        const now = makeDate(3, 12, 0);
        const snapshot = now.getTime();
        getMonthStart(now);
        expect(now.getTime()).toBe(snapshot);
    });
});

// ─── getRaidRefreshInfo / getRaidCooldownRules 测试 ──────────────────────────
describe('getRaidRefreshInfo / getRaidCooldownRules', () => {
    it('25人本 → 下次刷新为下周一 07:00，refreshCount=1', () => {
        const now = makeDate(3, 12, 0); // 周三
        const info = getRaidRefreshInfo(makeRaid(25), now);
        expect(info.nextRefreshTime?.getDay()).toBe(1);
        expect(info.nextRefreshTime?.getHours()).toBe(7);
        expect(info.refreshCount).toBe(1);
        expect(info.refreshSchedule).toBe('每周一 7:00');
        expect(info.isRefreshing).toBe(false);
    });

    it('10人本上半周 → 下次刷新为本周五 07:00，refreshCount=2', () => {
        const now = makeDate(3, 12, 0); // 周三，处于上半周
        const info = getRaidRefreshInfo(makeRaid(10), now);
        expect(info.nextRefreshTime?.getDay()).toBe(5); // 周五
        expect(info.nextRefreshTime?.getHours()).toBe(7);
        expect(info.refreshCount).toBe(2);
        expect(info.refreshSchedule).toBe('周一/周五 7:00');
    });

    it('10人本下半周 → 下次刷新为下周一 07:00', () => {
        const now = makeDate(6, 15, 0); // 周六，处于下半周
        const info = getRaidRefreshInfo(makeRaid(10), now);
        expect(info.nextRefreshTime?.getDay()).toBe(1);
        expect(info.nextRefreshTime?.getHours()).toBe(7);
    });

    it('getRaidCooldownRules → 25人本 / 10人本文案分支', () => {
        expect(getRaidCooldownRules(makeRaid(25))).toBe('25人本：每周一 7:00 刷新');
        expect(getRaidCooldownRules(makeRaid(10))).toBe('10人本：每周一 7:00 和 周五 7:00 刷新');
    });
});

// ─── formatRemainingTime / formatCountdown 测试 ─────────────────────────────
describe('formatRemainingTime / formatCountdown', () => {
    const MINUTE = 60 * 1000;
    const HOUR = 60 * MINUTE;
    const DAY = 24 * HOUR;

    it('formatRemainingTime：<= 0 返回「可添加」', () => {
        expect(formatRemainingTime(0)).toBe('可添加');
        expect(formatRemainingTime(-1)).toBe('可添加');
    });

    it('formatRemainingTime：不足 1 小时只显示分钟', () => {
        expect(formatRemainingTime(30 * MINUTE)).toBe('30分钟');
    });

    it('formatRemainingTime：不足 1 天显示「x小时x分钟」', () => {
        expect(formatRemainingTime(2 * HOUR + 5 * MINUTE)).toBe('2小时5分钟');
    });

    it('formatRemainingTime：超过 1 天显示「x天x小时」', () => {
        expect(formatRemainingTime(3 * DAY + 4 * HOUR)).toBe('3天4小时');
    });

    it('formatCountdown：<= 0 返回「已刷新」', () => {
        expect(formatCountdown(0)).toBe('已刷新');
        expect(formatCountdown(-1000)).toBe('已刷新');
    });

    it('formatCountdown：不足 1 天只显示 HH:MM:SS 并补零', () => {
        expect(formatCountdown(1 * HOUR + 2 * MINUTE + 3 * 1000)).toBe('01:02:03');
        expect(formatCountdown(59 * 1000)).toBe('00:00:59');
    });

    it('formatCountdown：超过 1 天显示「x天 HH:MM:SS」', () => {
        expect(formatCountdown(2 * DAY + 3 * HOUR + 4 * MINUTE + 5 * 1000)).toBe('2天 03:04:05');
    });
});

// ─── 操作日志（内存态）测试 ──────────────────────────────────────────────────
describe('logOperation / getOperationLogs / clearOperationLogs', () => {
    beforeEach(() => {
        clearOperationLogs();
        // 该模块会 console.log 每条操作日志，测试中静音避免污染输出
        vi.spyOn(console, 'log').mockImplementation(() => {});
    });

    afterEach(() => {
        vi.restoreAllMocks();
    });

    it('初始为空', () => {
        expect(getOperationLogs()).toEqual([]);
    });

    it('logOperation 追加一条日志并带上入参字段', () => {
        logOperation('role-1', '测试角色', '燃木京25', true, undefined, '详情');
        const logs = getOperationLogs();
        expect(logs).toHaveLength(1);
        expect(logs[0].roleId).toBe('role-1');
        expect(logs[0].roleName).toBe('测试角色');
        expect(logs[0].raidName).toBe('燃木京25');
        expect(logs[0].success).toBe(true);
        expect(logs[0].details).toBe('详情');
        expect(logs[0].id).toMatch(/^log_\d+_[a-z0-9]+$/);
        expect(logs[0].timestamp).toBeInstanceOf(Date);
    });

    it('失败日志保留 errorMessage', () => {
        logOperation('role-2', '角色二', '燃木京10', false, '写入失败');
        expect(getOperationLogs()[0].errorMessage).toBe('写入失败');
        expect(getOperationLogs()[0].success).toBe(false);
    });

    it('getOperationLogs 返回副本，外部修改不影响内部存储', () => {
        logOperation('role-1', '角色一', '燃木京25', true);
        const logs = getOperationLogs();
        logs.length = 0;
        expect(getOperationLogs()).toHaveLength(1);
    });

    it('clearOperationLogs 清空全部日志', () => {
        logOperation('role-1', '角色一', '燃木京25', true);
        logOperation('role-2', '角色二', '燃木京10', false, '失败');
        expect(getOperationLogs()).toHaveLength(2);
        clearOperationLogs();
        expect(getOperationLogs()).toEqual([]);
    });
});
