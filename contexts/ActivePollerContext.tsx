import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import type { BatchActiveResult } from '../types';
import { activeDetectorService } from '../services/activeDetector';
import { useAppConfig } from './AppConfigContext';

interface ActivePollerContextType {
  result: BatchActiveResult | null;
  isPolling: boolean;
  lastError: string | null;
  /** 手动触发一次刷新（重置连续失败计数） */
  refresh: () => Promise<void>;
}

const ActivePollerContext = createContext<ActivePollerContextType | undefined>(undefined);

/** 轮询间隔（毫秒） */
const POLL_INTERVAL_ACTIVE_MS = 30_000; // JX3 运行 + 窗口聚焦
const POLL_INTERVAL_BACKGROUND_MS = 60_000; // JX3 未运行 或 窗口失焦
const POLL_INTERVAL_DEGRADED_MS = 120_000; // 连续失败降级
const MAX_CONSECUTIVE_FAILURES = 3; // 触发降级的失败次数阈值

export const ActivePollerProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { appConfig } = useAppConfig();
  const gameDirectory = appConfig?.gameDirectory ?? '';

  const [result, setResult] = useState<BatchActiveResult | null>(null);
  const [isPolling, setIsPolling] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  // useRef 保存可变状态，避免 re-render 影响定时器
  const timeoutRef = useRef<number | null>(null);
  const consecutiveFailuresRef = useRef(0);
  const isWindowFocusedRef = useRef(document.hasFocus());
  const isRefreshingRef = useRef(false);
  // 保存最新的 gameDirectory，供递归调度使用
  const gameDirectoryRef = useRef(gameDirectory);
  const resultRef = useRef<BatchActiveResult | null>(null);

  useEffect(() => {
    gameDirectoryRef.current = gameDirectory;
  }, [gameDirectory]);

  useEffect(() => {
    resultRef.current = result;
  }, [result]);

  const clearTimer = useCallback(() => {
    if (timeoutRef.current !== null) {
      window.clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  }, []);

  const scheduleNext = useCallback((delayMs: number) => {
    clearTimer();
    timeoutRef.current = window.setTimeout(() => {
      void runDetection();
    }, delayMs);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clearTimer]);

  const runDetection = useCallback(async () => {
    const dir = gameDirectoryRef.current;

    // 未配置游戏目录时不启动检测，但 5 秒后重新检查
    if (!dir) {
      setIsPolling(false);
      scheduleNext(5_000);
      return;
    }

    // 防止并发检测
    if (isRefreshingRef.current) {
      return;
    }

    isRefreshingRef.current = true;
    setIsPolling(true);

    try {
      const next = await activeDetectorService.detectAccountsActive(dir);
      setResult(next);
      setLastError(null);
      consecutiveFailuresRef.current = 0;

      // 选择下一次轮询间隔
      const isFocused = isWindowFocusedRef.current;
      const baseDelay = next.jx3Running && isFocused
        ? POLL_INTERVAL_ACTIVE_MS
        : POLL_INTERVAL_BACKGROUND_MS;
      scheduleNext(baseDelay);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error('[ActivePoller] 检测失败:', message);
      setLastError(message);
      consecutiveFailuresRef.current += 1;

      const isDegraded = consecutiveFailuresRef.current >= MAX_CONSECUTIVE_FAILURES;
      scheduleNext(isDegraded ? POLL_INTERVAL_DEGRADED_MS : POLL_INTERVAL_BACKGROUND_MS);
    } finally {
      isRefreshingRef.current = false;
      setIsPolling(false);
    }
  }, [scheduleNext]);

  // 暴露给外部的手动刷新
  const refresh = useCallback(async () => {
    consecutiveFailuresRef.current = 0;
    await runDetection();
  }, [runDetection]);

  // 监听 gameDirectory 变化：配置完成后立即触发首次检测
  useEffect(() => {
    if (!gameDirectory) {
      // 未配置目录时清空状态
      clearTimer();
      setResult(null);
      setIsPolling(false);
      return;
    }

    // 配置完成后立即触发首次检测
    void runDetection();

    return () => {
      clearTimer();
    };
  }, [gameDirectory, runDetection, clearTimer]);

  // 监听窗口聚焦/失焦
  useEffect(() => {
    const handleFocus = () => {
      isWindowFocusedRef.current = true;
      // 聚焦时如果距离上次检测超过 30s，立即触发一次
      const last = resultRef.current;
      if (last && gameDirectoryRef.current) {
        // 简单策略：聚焦即触发，由 runDetection 内部去重
        void runDetection();
      }
    };
    const handleBlur = () => {
      isWindowFocusedRef.current = false;
    };
    window.addEventListener('focus', handleFocus);
    window.addEventListener('blur', handleBlur);
    return () => {
      window.removeEventListener('focus', handleFocus);
      window.removeEventListener('blur', handleBlur);
    };
  }, [runDetection]);

  // 卸载时清理定时器
  useEffect(() => {
    return () => {
      clearTimer();
    };
  }, [clearTimer]);

  return (
    <ActivePollerContext.Provider
      value={{
        result,
        isPolling,
        lastError,
        refresh,
      }}
    >
      {children}
    </ActivePollerContext.Provider>
  );
};

export const useActivePoller = (): ActivePollerContextType => {
  const context = useContext(ActivePollerContext);
  if (context === undefined) {
    throw new Error('useActivePoller must be used within an ActivePollerProvider');
  }
  return context;
};
