import { beforeEach, describe, expect, it, vi } from 'vitest';

const { fetchMock } = vi.hoisted(() => ({ fetchMock: vi.fn() }));

vi.mock('@tauri-apps/plugin-http', () => ({ fetch: fetchMock }));

import {
    getNextWeeklyRaidRefreshDelay,
    getWeeklyRaidDungeons,
    isWeeklyRaidDateCurrent,
} from './weeklyRaidApi';

describe('getWeeklyRaidDungeons', () => {
    beforeEach(() => fetchMock.mockReset());

    it('读取团队秘境周常并去除重复名称', async () => {
        fetchMock.mockResolvedValue({
            ok: true,
            status: 200,
            json: async () => ({
                code: 200,
                data: {
                    date: '2026-11-05',
                    weekly: { raid: ['洛阳之战', '敖龙岛', '洛阳之战'] },
                },
            }),
        });

        await expect(getWeeklyRaidDungeons()).resolves.toEqual({
            date: '2026-11-05',
            raids: ['洛阳之战', '敖龙岛'],
        });
        expect(fetchMock).toHaveBeenCalledWith(
            'https://www.jx3api.com/active/calendar?num=0',
            expect.objectContaining({
                method: 'GET',
                signal: expect.any(AbortSignal),
            }),
        );
    });

    it('HTTP 错误和网络异常返回空结果', async () => {
        fetchMock.mockResolvedValueOnce({ ok: false, status: 503 });
        await expect(getWeeklyRaidDungeons()).resolves.toBeNull();

        fetchMock.mockRejectedValueOnce(new Error('offline'));
        await expect(getWeeklyRaidDungeons()).resolves.toBeNull();
    });

    it('业务码或字段类型无效时不返回部分数据', async () => {
        fetchMock.mockResolvedValue({
            ok: true,
            status: 200,
            json: async () => ({ code: 200, data: { date: '2026-11-05', weekly: { raid: ['洛阳之战', 828] } } }),
        });
        await expect(getWeeklyRaidDungeons()).resolves.toBeNull();

        fetchMock.mockResolvedValueOnce({
            ok: true,
            status: 200,
            json: async () => ({ code: 200, data: { date: '2026-02-30', weekly: { raid: ['洛阳之战'] } } }),
        });
        await expect(getWeeklyRaidDungeons()).resolves.toBeNull();

        fetchMock.mockResolvedValueOnce({
            ok: true,
            status: 200,
            json: async () => ({ code: 500, data: { date: '2026-11-05', weekly: { raid: ['洛阳之战'] } } }),
        });
        await expect(getWeeklyRaidDungeons()).resolves.toBeNull();
    });

    it('按北京时间周一 07:00 判断接口日期是否属于当前周', () => {
        expect(isWeeklyRaidDateCurrent('2026-11-08', new Date('2026-11-08T22:59:00Z'))).toBe(true);
        expect(isWeeklyRaidDateCurrent('2026-11-09', new Date('2026-11-08T22:59:00Z'))).toBe(false);
        expect(isWeeklyRaidDateCurrent('2026-11-09', new Date('2026-11-08T23:00:00Z'))).toBe(true);
        expect(isWeeklyRaidDateCurrent('2026-11-08', new Date('2026-11-08T23:00:00Z'))).toBe(false);
        expect(isWeeklyRaidDateCurrent('2026-11-10', new Date('2026-11-13T00:00:00Z'))).toBe(true);
        expect(isWeeklyRaidDateCurrent('2026-11-14', new Date('2026-11-13T00:00:00Z'))).toBe(false);
        expect(isWeeklyRaidDateCurrent('2026-11-12', new Date('2026-11-12T16:58:00Z'))).toBe(true);
        expect(isWeeklyRaidDateCurrent('2026-11-02', new Date('2026-11-12T16:58:00Z'))).toBe(false);
    });

    it('计算北京时间下一个周一 07:00 的刷新倒计时', () => {
        const week = 7 * 24 * 60 * 60 * 1000;
        expect(getNextWeeklyRaidRefreshDelay(new Date('2026-11-08T22:59:00Z'))).toBe(60 * 1000);
        expect(getNextWeeklyRaidRefreshDelay(new Date('2026-11-08T23:00:00Z'))).toBe(week);
        expect(getNextWeeklyRaidRefreshDelay(new Date('2026-11-11T00:00:00Z'))).toBe(
            4 * 24 * 60 * 60 * 1000 + 23 * 60 * 60 * 1000,
        );
    });
});
