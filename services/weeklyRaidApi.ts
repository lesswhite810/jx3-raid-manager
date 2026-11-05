import { fetch } from '@tauri-apps/plugin-http';

const WEEKLY_RAID_URL = 'https://www.jx3api.com/active/calendar?num=0';
const REQUEST_TIMEOUT_MS = 10_000;

export interface WeeklyRaidResult {
    date: string;
    raids: string[];
}

interface WeeklyRaidApiPayload {
    code?: number;
    data?: {
        date?: unknown;
        weekly?: {
            raid?: unknown;
        };
    };
}

const normalizeRaidNames = (value: unknown): string[] | null => {
    if (!Array.isArray(value)) return null;
    const names = value.filter((item): item is string => typeof item === 'string')
        .map(item => item.trim())
        .filter(Boolean);
    return names.length === value.length ? [...new Set(names)] : null;
};

const isValidDate = (value: string): boolean => {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
    const [year, month, day] = value.split('-').map(Number);
    const date = new Date(Date.UTC(year, month - 1, day));
    return date.getUTCFullYear() === year
        && date.getUTCMonth() === month - 1
        && date.getUTCDate() === day;
};

const getChinaDateParts = (value: Date): { date: string; hour: number } => {
    const parts = new Intl.DateTimeFormat('en-CA', {
        timeZone: 'Asia/Shanghai',
        year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', hourCycle: 'h23',
    }).formatToParts(value);
    const get = (type: string): string => parts.find(part => part.type === type)?.value ?? '';
    return { date: `${get('year')}-${get('month')}-${get('day')}`, hour: Number(get('hour')) };
};

/** 判断接口日期是否仍属于北京时间当前周（周一 07:00 重置）。 */
export function isWeeklyRaidDateCurrent(date: string, now: Date = new Date()): boolean {
    if (!isValidDate(date)) return false;
    const china = getChinaDateParts(now);
    const [year, month, day] = china.date.split('-').map(Number);
    const current = new Date(Date.UTC(year, month - 1, day));
    const dayOfWeek = current.getUTCDay();
    const daysSinceMonday = (dayOfWeek + 6) % 7;
    const beforeWeeklyReset = daysSinceMonday === 0 && china.hour < 7;
    if (beforeWeeklyReset) current.setUTCDate(current.getUTCDate() - 7);
    else current.setUTCDate(current.getUTCDate() - daysSinceMonday);
    const weekStart = current.toISOString().slice(0, 10);
    const effectiveDate = beforeWeeklyReset
        ? new Date(Date.UTC(year, month - 1, day - 1)).toISOString().slice(0, 10)
        : china.date;
    return date >= weekStart && date <= effectiveDate;
}

/** 返回距离北京时间下一个周一 07:00 的毫秒数；正好在边界时返回一周。 */
export function getNextWeeklyRaidRefreshDelay(now: Date = new Date()): number {
    const china = getChinaDateParts(now);
    const [year, month, day] = china.date.split('-').map(Number);
    const chinaDate = new Date(Date.UTC(year, month - 1, day));
    const dayOfWeek = chinaDate.getUTCDay();
    const daysUntilMonday = (8 - dayOfWeek) % 7;
    const beforeMondayReset = dayOfWeek === 1 && china.hour < 7;
    const daysToTarget = beforeMondayReset ? 0 : (daysUntilMonday || 7);
    const targetUtc = Date.UTC(year, month - 1, day + daysToTarget, 7) - 8 * 60 * 60 * 1000;
    const delay = targetUtc - now.getTime();
    return delay > 0 ? delay : 7 * 24 * 60 * 60 * 1000;
}

/** 获取当前活动日历中的武林通鉴·团队秘境名单。 */
export async function getWeeklyRaidDungeons(): Promise<WeeklyRaidResult | null> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
    try {
        const response = await fetch(WEEKLY_RAID_URL, {
            method: 'GET',
            signal: controller.signal,
        });
        if (!response.ok) {
            console.warn(`获取周常副本失败：HTTP ${response.status}`);
            return null;
        }

        const payload = await response.json() as WeeklyRaidApiPayload;
        if (payload.code !== 200) {
            console.warn('获取周常副本失败：接口业务状态无效');
            return null;
        }

        const date = typeof payload.data?.date === 'string' ? payload.data.date.trim() : '';
        const raids = normalizeRaidNames(payload.data?.weekly?.raid);
        if (!isValidDate(date) || !raids) {
            console.warn('获取周常副本失败：接口返回结构无效');
            return null;
        }

        return { date, raids };
    } catch (error) {
        console.warn('获取周常副本失败：网络请求异常', error);
        return null;
    } finally {
        clearTimeout(timeout);
    }
}
