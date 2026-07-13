import { useCallback, useEffect, useRef, useState } from 'react';
import { useActivePoller } from '../contexts/ActivePollerContext';
import { dropScannerService } from '../services/dropScanner';
import { getLastMonday, getNextMonday } from '../utils/cooldownManager';

/**
 * 副本掉落自动扫描 Hook（增量扫描架构）
 *
 * 工作原理：
 * 1. 订阅 useActivePoller 的活跃检测结果
 * 2. JX3 运行时，每 30 秒调用后端 `scan_all_active_raid_drops`：
 *    - 后端自动识别活跃账号（combat_logs mtime > 进程启动时间）
 *    - 后端基于 chatlog mtime 判断角色在线状态
 *    - 后端为每个副本实例计算 record_status：
 *      - `scanning`：副本进行中（BOSS未全击杀 / 无底薪 / 进程在跑 / 角色在线），UI 锁定
 *      - `pending`：副本已完成（BOSS全击杀 / 有底薪 / 进程退出 / 角色离线），UI 可确认
 * 3. 扫描到新记录即通过 onRecordsUpdated 通知 UI 刷新
 *
 * 单一扫描路径：前端不再逐账号调用 scanRaidDrops，统一走批量扫描。
 * 后端 scan_all_active_raid_drops 内部已做 combat_logs mtime 快速过滤，
 * 未活跃的账号不会触发 JCL 解析，扫描文件范围最小化。
 */

/** 扫描轮询间隔（毫秒） */
const SCAN_POLL_INTERVAL_MS = 30 * 1000; // 30 秒

export interface UseDropScannerOptions {
  /** 扫描完成且至少有一条新/更新记录时回调（用于触发 UI 刷新） */
  onRecordsUpdated?: (scannedCount: number) => void;
  /** 自动扫描是否启用，关闭时不触发轮询扫描 */
  autoScanEnabled?: boolean;
}

export interface UseDropScannerReturn {
  /** 是否正在扫描 */
  isScanning: boolean;
  /** 最近一次扫描的错误信息 */
  lastError: string | null;
  /** 最近一次扫描时间（毫秒时间戳） */
  lastScanAt: number | null;
  /** 手动触发一次扫描（绕过节流） */
  refresh: () => Promise<void>;
  /** 扫描本周（周一7:00到下周一7:00）所有账号数据，不依赖 JX3 运行状态 */
  scanThisWeek: () => Promise<number>;
}

export const useDropScanner = (options: UseDropScannerOptions = {}): UseDropScannerReturn => {
  const { onRecordsUpdated, autoScanEnabled = false } = options;
  const { result: activeResult } = useActivePoller();

  const [isScanning, setIsScanning] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);
  const [lastScanAt, setLastScanAt] = useState<number | null>(null);

  // 防止并发扫描
  const isScanningRef = useRef(false);
  // 保存最新的 activeResult，避免 stale closure
  const activeResultRef = useRef(activeResult);
  // 保存最新的回调
  const onRecordsUpdatedRef = useRef(onRecordsUpdated);

  useEffect(() => {
    activeResultRef.current = activeResult;
  }, [activeResult]);

  useEffect(() => {
    onRecordsUpdatedRef.current = onRecordsUpdated;
  }, [onRecordsUpdated]);

  const scanAll = useCallback(async () => {
    const result = activeResultRef.current;
    if (!result || !result.jx3Running) {
      return;
    }

    if (isScanningRef.current) {
      return;
    }

    isScanningRef.current = true;
    setIsScanning(true);

    try {
      const scanResults = await dropScannerService.scanAllActiveRaidDrops();
      // 统计本次扫描处理的副本实例总数
      const totalScanned = scanResults.reduce((sum, r) => sum + (r.instanceCount ?? 0), 0);

      if (totalScanned > 0) {
        setLastScanAt(Date.now());
        onRecordsUpdatedRef.current?.(totalScanned);
      } else {
        // 即使没有新记录，也更新 lastScanAt 用于 UI 显示最近扫描时间
        setLastScanAt(Date.now());
      }
      setLastError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('[DropScanner] 批量扫描失败:', message);
      setLastError(message);
    } finally {
      isScanningRef.current = false;
      setIsScanning(false);
    }
  }, []);

  // 订阅活跃检测结果 + 30 秒轮询（仅在自动扫描开启时）
  useEffect(() => {
    if (!autoScanEnabled) return;
    if (!activeResult) return;
    if (!activeResult.jx3Running) return;

    // JX3 启动后延迟 2 秒触发首次扫描，避免与活跃检测 IO 抢占
    const initialTimer = window.setTimeout(() => {
      void scanAll();
    }, 2000);

    // 30 秒轮询
    const intervalId = window.setInterval(() => {
      const result = activeResultRef.current;
      if (!result || !result.jx3Running) {
        return;
      }
      void scanAll();
    }, SCAN_POLL_INTERVAL_MS);

    return () => {
      window.clearTimeout(initialTimer);
      window.clearInterval(intervalId);
    };
  }, [activeResult?.jx3Running, scanAll, autoScanEnabled]);

  // 手动刷新
  const refresh = useCallback(async () => {
    await scanAll();
  }, [scanAll]);

  /**
   * 扫描本周（周一7:00到下周一7:00）所有账号数据
   *
   * 离线扫描，不依赖 JX3 进程运行状态：
   * 1. 使用 cooldownManager 的 getLastMonday/getNextMonday 计算本周一7:00到下周一7:00
   * 2. 调用 scanRaidsInRange 扫描所有账号在该时间范围内的副本
   * 3. 扫描完成后通知 UI 刷新
   *
   * @returns 本次扫描处理的副本实例总数
   */
  const scanThisWeek = useCallback(async (): Promise<number> => {
    if (isScanningRef.current) {
      return 0;
    }

    const now = new Date();
    const startMs = getLastMonday(now).getTime();
    const endMs = getNextMonday(now).getTime();

    isScanningRef.current = true;
    setIsScanning(true);

    try {
      const scanResults = await dropScannerService.scanRaidsInRange(startMs, endMs);
      const totalScanned = scanResults.reduce((sum, r) => sum + (r.instanceCount ?? 0), 0);

      setLastScanAt(Date.now());
      setLastError(null);

      if (totalScanned > 0) {
        onRecordsUpdatedRef.current?.(totalScanned);
      }

      return totalScanned;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('[DropScanner] 扫描本周失败:', message);
      setLastError(message);
      throw err;
    } finally {
      isScanningRef.current = false;
      setIsScanning(false);
    }
  }, []);

  return {
    isScanning,
    lastError,
    lastScanAt,
    refresh,
    scanThisWeek,
  };
};
