import { describe, it, expect } from 'vitest';
import {
    getCardClass,
    getAddButtonClass,
    getViewButtonClass,
    SECONDARY_BUTTON_CLASS,
    type CardStatus,
} from './cardStatusStyles';

const ALL: CardStatus[] = ['none', 'partial', 'pending', 'complete'];

/**
 * 这些测试锁定三个副本模块共用的状态配色契约。
 * 它们存在的理由：团队副本曾把 warning 当作「部分清」、把 sky 当作「待确认」，
 * 与试炼/百战正好相反，同一个棕色在两个页面含义不同。共享模块 + 测试是
 * 防止再次分叉的唯一可靠手段。
 */
describe('cardStatusStyles 四态配色契约', () => {
    describe('卡片描边必须用 soft 档，不能用 DEFAULT 饱和色', () => {
        it.each(ALL)('%s 的静态描边是 soft 档', (status) => {
            const cls = getCardClass(status);
            // 提取所有非 hover / 非 dark 的 border-ds-* 静态类
            const borders = cls
                .split(/\s+/)
                .filter((c) => c.startsWith('border-ds-') && !c.includes('hover:'));
            expect(borders.length).toBeGreaterThan(0);
            for (const b of borders) {
                expect(b, `${status} 的描边 ${b} 必须是 soft 档`).toMatch(/-soft$/);
            }
        });

        it.each(ALL)('%s 的 hover 描边提升到 DEFAULT', (status) => {
            const cls = getCardClass(status);
            const hoverBorders = cls
                .split(/\s+/)
                .filter((c) => c.startsWith('hover:border-ds-'));
            expect(hoverBorders.length).toBeGreaterThan(0);
            for (const b of hoverBorders) {
                expect(b, `${status} 的 hover 描边 ${b} 不应是 soft 档`).not.toMatch(/-soft$/);
            }
        });
    });

    describe('四态底色语义固定', () => {
        it('未清是 success 系', () => {
            expect(getCardClass('none')).toContain('bg-ds-success-soft');
        });

        it('部分清是 info 系（不是 warning）', () => {
            const cls = getCardClass('partial');
            expect(cls).toContain('bg-ds-info-soft');
            expect(cls, '部分清不能用 warning —— 历史上团队副本正是这样错的').not.toContain('warning');
        });

        it('待确认是 warning 系', () => {
            expect(getCardClass('pending')).toContain('bg-ds-warning-soft');
        });

        it('完全清是中性底 + 页面同底', () => {
            const cls = getCardClass('complete');
            expect(cls).toContain('bg-base');
            expect(cls).toContain('border-ds-neutral-soft');
        });

        it('四态底色两两不同', () => {
            const seen = new Set(ALL.map((s) => getCardClass(s)));
            expect(seen.size).toBe(4);
        });
    });

    describe('实心按钮底色用 *-strong 配 fg-inverse', () => {
        it('未清是实心 success-strong + fg-inverse', () => {
            const cls = getAddButtonClass('none');
            expect(cls).toContain('bg-ds-success-strong');
            expect(cls).toContain('text-ds-fg-inverse');
        });

        it('完全清是实心 neutral + fg-inverse', () => {
            const cls = getAddButtonClass('complete');
            expect(cls).toContain('bg-ds-neutral');
            expect(cls).toContain('text-ds-fg-inverse');
        });

        it('不使用 DEFAULT 饱和色作实心底色', () => {
            for (const status of ALL) {
                const cls = getAddButtonClass(status);
                // bg-ds-success（无 -strong）会让白字对比度掉到 4.07
                expect(cls, `${status} 用了 DEFAULT 作实心底`).not.toMatch(/bg-ds-(success|warning|info)\b(?!-)/);
            }
        });

        it('不使用写死的 text-white', () => {
            for (const status of ALL) {
                expect(getAddButtonClass(status)).not.toContain('text-white');
            }
        });

        it('hover 不换实心底色', () => {
            for (const status of ALL) {
                const cls = getAddButtonClass(status);
                expect(cls, `${status} 的 hover 不应换底色`).not.toMatch(/hover:bg-ds-(success|warning|info|neutral)\b(?!-)/);
            }
        });

        it('部分清 / 待确认为描边，不做实心', () => {
            for (const status of ['partial', 'pending'] as CardStatus[]) {
                const cls = getAddButtonClass(status);
                expect(cls).toContain('bg-surface');
                expect(cls, `${status} 不应是实心`).not.toContain('text-ds-fg-inverse');
            }
        });
    });

    describe('查看按钮统一描边且底色跟随主题', () => {
        it.each(ALL)('%s 用 bg-surface 而非 bg-white', (status) => {
            const cls = getViewButtonClass(status);
            expect(cls).toContain('bg-surface');
            expect(cls, 'bg-white 在深色主题仍是纯白，浅字看不清').not.toContain('bg-white');
        });

        it.each(ALL)('%s 文字用 *-strong 保证对比度', (status) => {
            const cls = getViewButtonClass(status);
            expect(cls).toMatch(/text-ds-(success|info|warning)-strong|text-ds-neutral/);
        });
    });

    describe('禁用态不参与状态配色', () => {
        it('disabled 时统一返回禁用外观', () => {
            for (const status of ALL) {
                const cls = getAddButtonClass(status, true);
                expect(cls).toContain('cursor-not-allowed');
                expect(cls).toContain('text-muted');
                expect(cls, '禁用态不应带状态底色').not.toMatch(/bg-ds-(success|info|warning)-soft/);
            }
        });

        it('disabled 优先于状态', () => {
            expect(getAddButtonClass('none', true)).toBe(getAddButtonClass('complete', true));
        });
    });

    describe('次级按钮中性化，不抢主操作', () => {
        it('不含状态语义色', () => {
            expect(SECONDARY_BUTTON_CLASS).not.toMatch(/ds-(success|warning|info)/);
            expect(SECONDARY_BUTTON_CLASS).toContain('bg-surface');
            expect(SECONDARY_BUTTON_CLASS).toContain('border-base');
        });
    });

    describe('卡片 hover 反馈一致', () => {
        it.each(ALL)('%s 都有 shadow-ds-stack 上浮反馈', (status) => {
            expect(getCardClass(status)).toContain('hover:shadow-ds-stack');
        });
    });
});
