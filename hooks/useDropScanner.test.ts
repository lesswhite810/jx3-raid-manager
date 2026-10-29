import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { AccountScanResult } from '../services/dropScanner';
import type { BatchActiveResult, ProcessSession } from '../types';
import type { UseDropScannerOptions, UseDropScannerReturn } from './useDropScanner';
import { useDropScanner } from './useDropScanner';

/**
 * useDropScanner 测试说明
 *
 * 仓库未安装 jsdom / happy-dom / @testing-library/react / react-test-renderer，
 * 测试环境为 node，因此无法用标准渲染器挂载 Hook。
 *
 * 这里用一个极简的 React Hooks 运行时（hookRuntime）替代 `react` 模块：
 * 按 React 的调用顺序维护 state / ref / callback / effect 槽位，
 * 依赖数组用 Object.is 逐项比较，effect 在依赖变化时先执行 cleanup 再执行新 effect。
 * 这样可以在无 DOM 环境下真实驱动 useDropScanner 的定时器链路。
 *
 * useActivePoller 与 dropScannerService 通过 vi.mock 替换为可控桩。
 */

// useDropScanner 通过 window.setTimeout / window.setInterval 注册定时器，
// node 环境没有 window，这里把 globalThis 暴露为 window，使 vi 的假定时器可以接管。
Object.defineProperty(globalThis, 'window', {
    value: globalThis,
    configurable: true,
    writable: true,
});

const hookRuntime = vi.hoisted(() => {
    type Deps = readonly unknown[];
    type EffectFn = () => void | (() => void);

    const stateSlots: unknown[] = [];
    const refSlots: { current: unknown }[] = [];
    const callbackSlots: { fn: unknown; deps?: Deps }[] = [];
    const effectSlots: { deps?: Deps; cleanup?: () => void }[] = [];
    const pendingEffects: { index: number; fn: EffectFn; deps?: Deps }[] = [];

    let stateCursor = 0;
    let refCursor = 0;
    let callbackCursor = 0;
    let effectCursor = 0;
    let renderFn: (() => void) | null = null;
    let rendering = false;
    let flushing = false;
    let needsRender = false;

    /** 依赖数组比较：任一侧缺失视为变化（与 React 一致，无依赖数组的 effect 每次渲染都执行） */
    const depsEqual = (a: Deps | undefined, b: Deps | undefined): boolean => {
        if (!a || !b || a.length !== b.length) return false;
        for (let i = 0; i < a.length; i++) {
            if (!Object.is(a[i], b[i])) return false;
        }
        return true;
    };

    const flushEffects = (): void => {
        const cleanups: (() => void)[] = [];
        const toRun: { index: number; fn: EffectFn }[] = [];

        for (const pending of pendingEffects) {
            const previous = effectSlots[pending.index];
            if (previous && depsEqual(previous.deps, pending.deps)) continue;
            if (previous?.cleanup) cleanups.push(previous.cleanup);
            effectSlots[pending.index] = { deps: pending.deps, cleanup: undefined };
            toRun.push({ index: pending.index, fn: pending.fn });
        }
        pendingEffects.length = 0;

        flushing = true;
        try {
            for (const cleanup of cleanups) cleanup();
            for (const item of toRun) {
                const returned = item.fn();
                if (typeof returned === 'function') {
                    effectSlots[item.index].cleanup = returned;
                }
            }
        } finally {
            flushing = false;
        }
    };

    const renderOnce = (): void => {
        if (!renderFn) return;
        stateCursor = 0;
        refCursor = 0;
        callbackCursor = 0;
        effectCursor = 0;
        rendering = true;
        try {
            renderFn();
        } finally {
            rendering = false;
        }
        flushEffects();
    };

    const render = (): void => {
        if (!renderFn) return;
        let guard = 0;
        do {
            needsRender = false;
            renderOnce();
            guard += 1;
        } while (needsRender && guard < 20);
    };

    const useState = <T>(initial: T | (() => T)): [T, (next: T | ((previous: T) => T)) => void] => {
        const index = stateCursor++;
        if (!(index in stateSlots)) {
            stateSlots[index] = typeof initial === 'function' ? (initial as () => T)() : initial;
        }
        const setState = (next: T | ((previous: T) => T)): void => {
            const previous = stateSlots[index] as T;
            const value = typeof next === 'function' ? (next as (p: T) => T)(previous) : next;
            if (Object.is(value, previous)) return;
            stateSlots[index] = value;
            if (rendering || flushing) {
                needsRender = true;
                return;
            }
            render();
        };
        return [stateSlots[index] as T, setState];
    };

    const useRef = <T>(initial: T): { current: T } => {
        const index = refCursor++;
        if (!(index in refSlots)) {
            refSlots[index] = { current: initial };
        }
        return refSlots[index] as { current: T };
    };

    const useCallback = <T extends (...args: never[]) => unknown>(fn: T, deps?: Deps): T => {
        const index = callbackCursor++;
        const previous = callbackSlots[index];
        if (previous && depsEqual(previous.deps, deps)) {
            return previous.fn as T;
        }
        callbackSlots[index] = { fn, deps };
        return fn;
    };

    const useEffect = (fn: EffectFn, deps?: Deps): void => {
        pendingEffects.push({ index: effectCursor++, fn, deps });
    };

    /** 清理全部 effect（相当于卸载），并清空槽位 */
    const reset = (): void => {
        for (const slot of effectSlots) {
            slot.cleanup?.();
        }
        effectSlots.length = 0;
        pendingEffects.length = 0;
        stateSlots.length = 0;
        refSlots.length = 0;
        callbackSlots.length = 0;
        stateCursor = 0;
        refCursor = 0;
        callbackCursor = 0;
        effectCursor = 0;
        renderFn = null;
        rendering = false;
        flushing = false;
        needsRender = false;
    };

    const mount = (component: () => void): void => {
        reset();
        renderFn = component;
        render();
    };

    const rerender = (): void => {
        render();
    };

    const unmount = (): void => {
        for (const slot of effectSlots) {
            slot.cleanup?.();
        }
        effectSlots.length = 0;
        pendingEffects.length = 0;
        renderFn = null;
    };

    return { useState, useRef, useCallback, useEffect, mount, rerender, unmount, reset };
});

/** 可控的活跃检测结果（替代 useActivePoller） */
const pollerState = vi.hoisted(() => ({
    result: null as BatchActiveResult | null,
}));

/** 可控的掉落扫描服务（替代真实 invoke 调用） */
const dropScannerMock = vi.hoisted(() => ({
    scanAllActiveRaidDrops: vi.fn<() => Promise<AccountScanResult[]>>(),
    scanRaidsInRange: vi.fn<
        (startMs: number, endMs: number, processStartMs?: number) => Promise<AccountScanResult[]>
    >(),
}));

vi.mock('react', () => ({
    useState: hookRuntime.useState,
    useRef: hookRuntime.useRef,
    useCallback: hookRuntime.useCallback,
    useEffect: hookRuntime.useEffect,
}));

vi.mock('../contexts/ActivePollerContext', () => ({
    useActivePoller: () => ({
        result: pollerState.result,
        isPolling: false,
        lastError: null,
        refresh: async () => {},
    }),
}));

vi.mock('../services/dropScanner', () => ({
    dropScannerService: dropScannerMock,
}));

// ─── 测试辅助 ────────────────────────────────────────────────────────────────

/** 2026-02-16 为周一；2026-02-18 为周三 */
const NOW = new Date(2026, 1, 18, 12, 0, 0, 0);

const makeActiveResult = (overrides: Partial<BatchActiveResult> = {}): BatchActiveResult => ({
    jx3Running: true,
    jx3StartTime: null,
    jx3EndTime: null,
    jx3ProcessCount: 1,
    multiInstanceDetected: false,
    multiInstanceHint: null,
    roles: [],
    scanDurationMs: 0,
    processSessions: [],
    ...overrides,
});

const makeSession = (startTimeUnix: number, endTimeUnix: number | null): ProcessSession => ({
    startTimeUnix,
    endTimeUnix,
    startTime: new Date(startTimeUnix * 1000).toISOString(),
    endTime: endTimeUnix === null ? null : new Date(endTimeUnix * 1000).toISOString(),
});

const makeScanResult = (accountId: string, instanceCount: number): AccountScanResult => ({
    accountId,
    success: true,
    instanceCount,
});

/** 手动控制 resolve 时机的 Promise，用于构造“扫描进行中”场景 */
const deferred = <T,>() => {
    let resolve: (value: T) => void = () => {};
    const promise = new Promise<T>(res => {
        resolve = res;
    });
    return { promise, resolve };
};

interface HookHarness {
    /** 最近一次渲染返回的 Hook 结果 */
    result: () => UseDropScannerReturn;
    /** 触发一次重渲染（用于模拟 useActivePoller 结果变化） */
    rerender: () => void;
    /** 卸载并执行全部 effect cleanup */
    unmount: () => void;
}

const mountHook = (options: UseDropScannerOptions = {}): HookHarness => {
    let current: UseDropScannerReturn | null = null;
    hookRuntime.mount(() => {
        current = useDropScanner(options);
    });
    const read = (): UseDropScannerReturn => {
        if (!current) {
            throw new Error('useDropScanner 未完成首次渲染');
        }
        return current;
    };
    return {
        result: read,
        rerender: () => hookRuntime.rerender(),
        unmount: () => hookRuntime.unmount(),
    };
};

beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    pollerState.result = null;
    dropScannerMock.scanAllActiveRaidDrops.mockReset();
    dropScannerMock.scanRaidsInRange.mockReset();
    // Hook 每次扫描都会 console.log / console.error，测试中静音避免污染输出
    vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
    hookRuntime.reset();
    vi.restoreAllMocks();
    vi.useRealTimers();
});

// ─── 初始状态与自动扫描开关 ─────────────────────────────────────────────────

describe('useDropScanner 初始状态与自动扫描开关', () => {
    it('未开启自动扫描：初始状态为空且不注册任何定时器', () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        const harness = mountHook();

        expect(harness.result().isScanning).toBe(false);
        expect(harness.result().lastError).toBeNull();
        expect(harness.result().lastScanAt).toBeNull();
        expect(vi.getTimerCount()).toBe(0);
        expect(dropScannerMock.scanAllActiveRaidDrops).not.toHaveBeenCalled();
    });

    it('开启自动扫描但 JX3 未运行：不注册定时器', () => {
        pollerState.result = makeActiveResult({ jx3Running: false });
        mountHook({ autoScanEnabled: true });

        expect(vi.getTimerCount()).toBe(0);
    });

    it('开启自动扫描且 JX3 运行：注册首次延迟定时器 + 30 秒轮询定时器', () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([]);
        mountHook({ autoScanEnabled: true });

        expect(vi.getTimerCount()).toBe(2);
    });

    it('活跃检测结果为空时不注册定时器', () => {
        pollerState.result = null;
        mountHook({ autoScanEnabled: true });

        expect(vi.getTimerCount()).toBe(0);
    });
});

// ─── 定时器链路：首次 2 秒延迟 + 30 秒轮询 ──────────────────────────────────

describe('useDropScanner 定时器链路', () => {
    it('首次扫描延迟 2 秒触发（1999ms 不触发，2000ms 触发一次）', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([]);
        mountHook({ autoScanEnabled: true });

        await vi.advanceTimersByTimeAsync(1999);
        expect(dropScannerMock.scanAllActiveRaidDrops).not.toHaveBeenCalled();

        await vi.advanceTimersByTimeAsync(1);
        expect(dropScannerMock.scanAllActiveRaidDrops).toHaveBeenCalledTimes(1);
    });

    it('每 30 秒轮询一次扫描', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([]);
        mountHook({ autoScanEnabled: true });

        await vi.advanceTimersByTimeAsync(2000);
        expect(dropScannerMock.scanAllActiveRaidDrops).toHaveBeenCalledTimes(1);

        await vi.advanceTimersByTimeAsync(30 * 1000);
        expect(dropScannerMock.scanAllActiveRaidDrops).toHaveBeenCalledTimes(2);

        await vi.advanceTimersByTimeAsync(30 * 1000);
        expect(dropScannerMock.scanAllActiveRaidDrops).toHaveBeenCalledTimes(3);
    });

    it('轮询触发时若 JX3 已退出，则跳过扫描', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([]);
        const harness = mountHook({ autoScanEnabled: true });

        // 尚未到 2 秒就把活跃结果改为未运行，轮询回调内部会再次校验
        pollerState.result = makeActiveResult({ jx3Running: false });
        harness.rerender();

        await vi.advanceTimersByTimeAsync(2000);
        expect(dropScannerMock.scanAllActiveRaidDrops).not.toHaveBeenCalled();
    });

    it('卸载后清理定时器，不再触发扫描', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([]);
        const harness = mountHook({ autoScanEnabled: true });

        expect(vi.getTimerCount()).toBe(2);
        harness.unmount();
        expect(vi.getTimerCount()).toBe(0);

        await vi.advanceTimersByTimeAsync(60 * 1000);
        expect(dropScannerMock.scanAllActiveRaidDrops).not.toHaveBeenCalled();
    });
});

// ─── 扫描结果与错误处理 ─────────────────────────────────────────────────────

describe('useDropScanner 扫描结果与错误处理', () => {
    it('扫描成功且实例数 > 0 时回调 onRecordsUpdated 并记录 lastScanAt', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([
            makeScanResult('a1', 2),
            makeScanResult('a2', 3),
        ]);
        const onRecordsUpdated = vi.fn();
        const harness = mountHook({ autoScanEnabled: true, onRecordsUpdated });

        await vi.advanceTimersByTimeAsync(2000);

        expect(onRecordsUpdated).toHaveBeenCalledTimes(1);
        expect(onRecordsUpdated).toHaveBeenCalledWith(5);
        // lastScanAt 记录的是扫描完成时刻，即首次延迟 2 秒之后的系统时间
        expect(harness.result().lastScanAt).toBe(NOW.getTime() + 2000);
        expect(harness.result().lastError).toBeNull();
    });

    it('实例数为 0 时不回调 onRecordsUpdated，但仍刷新 lastScanAt', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([makeScanResult('a1', 0)]);
        const onRecordsUpdated = vi.fn();
        const harness = mountHook({ autoScanEnabled: true, onRecordsUpdated });

        await vi.advanceTimersByTimeAsync(2000);

        expect(onRecordsUpdated).not.toHaveBeenCalled();
        expect(harness.result().lastScanAt).toBe(NOW.getTime() + 2000);
    });

    it('扫描抛错时记录 lastError，且 isScanning 复位为 false', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockRejectedValue(new Error('扫描失败'));
        const harness = mountHook({ autoScanEnabled: true });

        await vi.advanceTimersByTimeAsync(2000);

        expect(harness.result().lastError).toBe('扫描失败');
        expect(harness.result().isScanning).toBe(false);
        expect(harness.result().lastScanAt).toBeNull();
    });

    it('非 Error 类型的异常按 String 记录', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockRejectedValue('字符串错误');
        const harness = mountHook({ autoScanEnabled: true });

        await vi.advanceTimersByTimeAsync(2000);

        expect(harness.result().lastError).toBe('字符串错误');
    });

    it('扫描进行中 isScanning 为 true，完成后复位为 false', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        const scan = deferred<AccountScanResult[]>();
        dropScannerMock.scanAllActiveRaidDrops.mockImplementation(() => scan.promise);
        const harness = mountHook({ autoScanEnabled: true });

        await vi.advanceTimersByTimeAsync(2000);
        expect(harness.result().isScanning).toBe(true);

        scan.resolve([]);
        await vi.advanceTimersByTimeAsync(0);
        expect(harness.result().isScanning).toBe(false);
    });

    it('JX3 未运行时手动刷新不会触发扫描', async () => {
        pollerState.result = makeActiveResult({ jx3Running: false });
        const harness = mountHook();

        await harness.result().refresh();

        expect(dropScannerMock.scanAllActiveRaidDrops).not.toHaveBeenCalled();
    });
});

// ─── 并发保护 ───────────────────────────────────────────────────────────────

describe('useDropScanner 并发保护', () => {
    it('扫描进行中再次手动刷新会被跳过（只调用一次服务）', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        const scan = deferred<AccountScanResult[]>();
        dropScannerMock.scanAllActiveRaidDrops.mockImplementation(() => scan.promise);
        const harness = mountHook();
        const { refresh } = harness.result();

        const first = refresh();
        expect(harness.result().isScanning).toBe(true);
        const second = refresh();

        expect(dropScannerMock.scanAllActiveRaidDrops).toHaveBeenCalledTimes(1);

        scan.resolve([]);
        await first;
        await second;
        expect(harness.result().isScanning).toBe(false);
    });
});

// ─── scanThisWeek ───────────────────────────────────────────────────────────

describe('useDropScanner scanThisWeek', () => {
    it('按「本周一 07:00 ~ 下周一 07:00」范围扫描并返回实例总数', async () => {
        pollerState.result = makeActiveResult({ jx3Running: false });
        dropScannerMock.scanRaidsInRange.mockResolvedValue([makeScanResult('a1', 4)]);
        const harness = mountHook();

        const total = await harness.result().scanThisWeek(123456);

        expect(total).toBe(4);
        expect(dropScannerMock.scanRaidsInRange).toHaveBeenCalledTimes(1);
        const [startMs, endMs, processStartMs] = dropScannerMock.scanRaidsInRange.mock.calls[0];
        expect(startMs).toBe(new Date(2026, 1, 16, 7, 0, 0, 0).getTime());
        expect(endMs).toBe(new Date(2026, 1, 23, 7, 0, 0, 0).getTime());
        expect(processStartMs).toBe(123456);
    });

    it('未传 processStartMs 时默认按 0 传递（不启用 mtime 过滤）', async () => {
        pollerState.result = makeActiveResult({ jx3Running: false });
        dropScannerMock.scanRaidsInRange.mockResolvedValue([]);
        const harness = mountHook();

        await harness.result().scanThisWeek();

        const [, , processStartMs] = dropScannerMock.scanRaidsInRange.mock.calls[0];
        expect(processStartMs).toBe(0);
    });

    it('扫描到实例时回调 onRecordsUpdated', async () => {
        pollerState.result = makeActiveResult({ jx3Running: false });
        dropScannerMock.scanRaidsInRange.mockResolvedValue([
            makeScanResult('a1', 2),
            makeScanResult('a2', 1),
        ]);
        const onRecordsUpdated = vi.fn();
        const harness = mountHook({ onRecordsUpdated });

        const total = await harness.result().scanThisWeek();

        expect(total).toBe(3);
        expect(onRecordsUpdated).toHaveBeenCalledWith(3);
    });

    it('扫描失败时向上抛出并记录 lastError', async () => {
        pollerState.result = makeActiveResult({ jx3Running: false });
        dropScannerMock.scanRaidsInRange.mockRejectedValue(new Error('范围扫描失败'));
        const harness = mountHook();

        await expect(harness.result().scanThisWeek()).rejects.toThrow('范围扫描失败');
        expect(harness.result().lastError).toBe('范围扫描失败');
        expect(harness.result().isScanning).toBe(false);
    });

    it('已有扫描进行中时直接返回 0，不调用服务', async () => {
        pollerState.result = makeActiveResult({ jx3Running: false });
        const rangeScan = deferred<AccountScanResult[]>();
        dropScannerMock.scanRaidsInRange.mockImplementation(() => rangeScan.promise);
        const harness = mountHook();

        const first = harness.result().scanThisWeek();
        const skipped = await harness.result().scanThisWeek();

        expect(skipped).toBe(0);
        expect(dropScannerMock.scanRaidsInRange).toHaveBeenCalledTimes(1);

        rangeScan.resolve([]);
        await first;
    });
});

// ─── JX3 退出后的延迟扫描 ───────────────────────────────────────────────────

describe('useDropScanner JX3 退出后的延迟扫描', () => {
    it('jx3Running 由 true 变 false 时，5 秒后按历史会话时间范围扫描', async () => {
        const session = makeSession(1_760_000_000, 1_760_003_600);
        pollerState.result = makeActiveResult({ jx3Running: true, processSessions: [session] });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([]);
        dropScannerMock.scanRaidsInRange.mockResolvedValue([makeScanResult('a1', 2)]);
        const onRecordsUpdated = vi.fn();
        const harness = mountHook({ autoScanEnabled: true, onRecordsUpdated });

        pollerState.result = makeActiveResult({ jx3Running: false, processSessions: [session] });
        harness.rerender();

        await vi.advanceTimersByTimeAsync(4999);
        expect(dropScannerMock.scanRaidsInRange).not.toHaveBeenCalled();

        await vi.advanceTimersByTimeAsync(1);
        expect(dropScannerMock.scanRaidsInRange).toHaveBeenCalledTimes(1);

        const [startMs, endMs, processStartMs] = dropScannerMock.scanRaidsInRange.mock.calls[0];
        expect(startMs).toBe(session.startTimeUnix * 1000);
        expect(endMs).toBe((session.endTimeUnix ?? 0) * 1000);
        expect(processStartMs).toBe(session.startTimeUnix * 1000);
        expect(onRecordsUpdated).toHaveBeenCalledWith(2);
    });

    it('无历史会话时跳过退出扫描', async () => {
        pollerState.result = makeActiveResult({ jx3Running: true });
        dropScannerMock.scanAllActiveRaidDrops.mockResolvedValue([]);
        const harness = mountHook({ autoScanEnabled: true });

        pollerState.result = makeActiveResult({ jx3Running: false, processSessions: [] });
        harness.rerender();

        await vi.advanceTimersByTimeAsync(10 * 1000);
        expect(dropScannerMock.scanRaidsInRange).not.toHaveBeenCalled();
    });

    it('退出扫描遇到进行中的扫描时，忙等循环会让出并在其结束后继续（不卡死）', async () => {
        const session = makeSession(1_760_000_000, 1_760_003_600);
        pollerState.result = makeActiveResult({ jx3Running: true, processSessions: [session] });
        const activeScan = deferred<AccountScanResult[]>();
        dropScannerMock.scanAllActiveRaidDrops.mockImplementation(() => activeScan.promise);
        dropScannerMock.scanRaidsInRange.mockResolvedValue([]);
        const harness = mountHook({ autoScanEnabled: true });

        // 让一次轮询扫描保持在飞行状态
        const inflight = harness.result().refresh();
        expect(harness.result().isScanning).toBe(true);

        // JX3 退出，5 秒后进入退出扫描流程
        pollerState.result = makeActiveResult({ jx3Running: false, processSessions: [session] });
        harness.rerender();
        await vi.advanceTimersByTimeAsync(5000);

        // 忙等循环在等待前一次扫描，因此尚未发起范围扫描
        expect(dropScannerMock.scanRaidsInRange).not.toHaveBeenCalled();

        // 结束飞行中的扫描，忙等循环在下一个 100ms tick 后继续
        activeScan.resolve([]);
        await inflight;
        await vi.advanceTimersByTimeAsync(200);

        expect(dropScannerMock.scanRaidsInRange).toHaveBeenCalledTimes(1);
    });
});
