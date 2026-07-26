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
 *
 * 日志规范：
 * - 统一前缀 `[AutoScan]`，便于在控制台过滤自动扫描流程
 * - 三种扫描场景：polling（30秒轮询）/ exit（JX3退出后）/ manual（手动刷新）
 * - 关键节点记录耗时、账号数、副本实例数等摘要信息
 */

/** 自动扫描日志前缀 */
const LOG_TAG = '[AutoScan]';

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
  /**
   * 扫描本周（周一7:00到下周一7:00）所有账号数据，不依赖 JX3 运行状态
   *
   * @param processStartMs JX3 进程启动时间（毫秒），> 0 时按 mtime 过滤 JCL 文件，仅扫描本次会话产生的文件
   * @returns 本次扫描处理的副本实例总数
   */
  scanThisWeek: (processStartMs?: number) => Promise<number>;
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
  // 记录上一次的 jx3Running 状态，用于检测 true → false 跳变
  const prevJx3RunningRef = useRef<boolean | null>(null);

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
      console.log(`${LOG_TAG} [polling] 跳过：已有扫描进行中`);
      return;
    }

    const startedAt = Date.now();
    isScanningRef.current = true;
    setIsScanning(true);

    try {
      const scanResults = await dropScannerService.scanAllActiveRaidDrops();
      const totalScanned = scanResults.reduce((sum, r) => sum + (r.instanceCount ?? 0), 0);
      const successCount = scanResults.filter(r => r.success).length;
      const failedCount = scanResults.length - successCount;
      const elapsedMs = Date.now() - startedAt;

      setLastScanAt(Date.now());
      setLastError(null);

      if (totalScanned > 0) {
        onRecordsUpdatedRef.current?.(totalScanned);
      }

      console.log(
        `${LOG_TAG} [polling] 完成：账号 ${successCount}/${scanResults.length}（失败 ${failedCount}），副本实例 ${totalScanned} 个，耗时 ${elapsedMs}ms`,
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      const elapsedMs = Date.now() - startedAt;
      console.error(`${LOG_TAG} [polling] 失败：${message}，耗时 ${elapsedMs}ms`);
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

    console.log(`${LOG_TAG} [polling] JX3 已启动，开启 30 秒轮询（首次延迟 2 秒）`);

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
      console.log(`${LOG_TAG} [polling] JX3 已退出，停止 30 秒轮询`);
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
   * @param processStartMs JX3 进程启动时间（毫秒），> 0 时按 mtime 过滤 JCL 文件，仅扫描本次会话产生的文件
   * @returns 本次扫描处理的副本实例总数
   */
  const scanThisWeek = useCallback(async (processStartMs: number = 0): Promise<number> => {
    if (isScanningRef.current) {
      console.log(`${LOG_TAG} [manual] 跳过：已有扫描进行中`);
      return 0;
    }

    const now = new Date();
    const startMs = getLastMonday(now).getTime();
    const endMs = getNextMonday(now).getTime();
    const startedAt = Date.now();

    isScanningRef.current = true;
    setIsScanning(true);

    try {
      const scanResults = await dropScannerService.scanRaidsInRange(startMs, endMs, processStartMs);
      const totalScanned = scanResults.reduce((sum, r) => sum + (r.instanceCount ?? 0), 0);
      const successCount = scanResults.filter(r => r.success).length;
      const failedCount = scanResults.length - successCount;
      const elapsedMs = Date.now() - startedAt;

      setLastScanAt(Date.now());
      setLastError(null);

      if (totalScanned > 0) {
        onRecordsUpdatedRef.current?.(totalScanned);
      }

      console.log(
        `${LOG_TAG} [manual] 完成：账号 ${successCount}/${scanResults.length}（失败 ${failedCount}），副本实例 ${totalScanned} 个，耗时 ${elapsedMs}ms`,
      );

      return totalScanned;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      const elapsedMs = Date.now() - startedAt;
      console.error(`${LOG_TAG} [manual] 失败：${message}，耗时 ${elapsedMs}ms`);
      setLastError(message);
      throw err;
    } finally {
      isScanningRef.current = false;
      setIsScanning(false);
    }
  }, []);

  // JX3 退出时触发一次"最终扫描"（离线扫描）
  // 原因：茗伊 chatlog 插件在 JX3 运行时不会实时 flush 聊天记录到磁盘，
  //       工资发放记录在 JX3 退出时才落盘。自动轮询扫描（JX3 运行时）读不到工资，
  //       需要在 JX3 退出后触发一次离线扫描来读取 flush 后的 chatlog 记录。
  //
  // 范围：自动扫描只处理历史进程会话时间范围内的 JCL 文件。
  //       遍历 BatchActiveResult.processSessions 中所有会话，对每个会话的
  //       [startTimeUnix, endTimeUnix] 时间范围分别调用 scanRaidsInRange。
  //       无历史会话则不处理（扫描本周由 UI 按钮手动触发，不属于自动扫描范畴）。
  //
  // 注意：依赖项使用 activeResult?.jx3Running 而非整个 activeResult 对象，
  //       避免 activeResult 对象引用变化导致 effect 频繁重跑、清除 5 秒定时器，
  //       造成退出后最终扫描永不触发的 bug。
  useEffect(() => {
    if (!autoScanEnabled) return;
    const jx3Running = activeResult?.jx3Running;
    if (jx3Running === undefined) return;

    const prevRunning = prevJx3RunningRef.current;
    const currentRunning = jx3Running;
    prevJx3RunningRef.current = currentRunning;

    // 检测 true → false 跳变（JX3 退出）
    if (prevRunning === true && !currentRunning) {
      // 获取所有历史进程会话
      const sessions = activeResult?.processSessions ?? [];

      // 无历史会话则不处理（扫描本周由 UI 按钮手动触发）
      if (sessions.length === 0) {
        console.log(`${LOG_TAG} [exit] 无历史会话，跳过退出扫描`);
        return;
      }

      console.log(
        `${LOG_TAG} [exit] JX3 退出，5 秒后扫描 ${sessions.length} 个历史会话的 JCL 文件`,
      );

      // 延迟 5 秒触发，等待 JX3 进程完全退出后 chatlog 文件 flush 完成
      const exitTimer = window.setTimeout(() => {
        void (async () => {
          const totalStartedAt = Date.now();
          let totalInstances = 0;
          let totalAccounts = 0;
          let totalFailed = 0;

          try {
            // 遍历所有历史会话，对每个会话的时间范围进行 JCL 处理
            for (let i = 0; i < sessions.length; i++) {
              const session = sessions[i];
              const sessionStartMs = session.startTimeUnix * 1000;
              const sessionEndMs = session.endTimeUnix
                ? session.endTimeUnix * 1000
                : Date.now();

              console.log(
                `${LOG_TAG} [exit] 会话 ${i + 1}/${sessions.length}：${new Date(sessionStartMs).toLocaleString('zh-CN')} ~ ${new Date(sessionEndMs).toLocaleString('zh-CN')}`,
              );

              // 等待上一次扫描完成（防止并发）
              while (isScanningRef.current) {
                await new Promise(resolve => setTimeout(resolve, 100));
              }

              isScanningRef.current = true;
              setIsScanning(true);
              const sessionStartedAt = Date.now();
              try {
                // 使用会话时间范围扫描，process_start_ms 设为会话启动时间用于 mtime 过滤
                const scanResults = await dropScannerService.scanRaidsInRange(
                  sessionStartMs,
                  sessionEndMs,
                  sessionStartMs,
                );
                const sessionScanned = scanResults.reduce(
                  (sum, r) => sum + (r.instanceCount ?? 0),
                  0,
                );
                const sessionSuccess = scanResults.filter(r => r.success).length;
                const sessionFailed = scanResults.length - sessionSuccess;
                const sessionElapsedMs = Date.now() - sessionStartedAt;

                totalInstances += sessionScanned;
                totalAccounts += sessionSuccess;
                totalFailed += sessionFailed;

                if (sessionScanned > 0) {
                  onRecordsUpdatedRef.current?.(sessionScanned);
                }
                setLastScanAt(Date.now());
                setLastError(null);

                console.log(
                  `${LOG_TAG} [exit] 会话 ${i + 1}/${sessions.length} 完成：账号 ${sessionSuccess}/${scanResults.length}（失败 ${sessionFailed}），副本实例 ${sessionScanned} 个，耗时 ${sessionElapsedMs}ms`,
                );
              } finally {
                isScanningRef.current = false;
                setIsScanning(false);
              }
            }

            const totalElapsedMs = Date.now() - totalStartedAt;
            console.log(
              `${LOG_TAG} [exit] 全部完成：${sessions.length} 个会话，账号 ${totalAccounts}（失败 ${totalFailed}），副本实例 ${totalInstances} 个，总耗时 ${totalElapsedMs}ms`,
            );
          } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            const totalElapsedMs = Date.now() - totalStartedAt;
            console.error(`${LOG_TAG} [exit] 失败：${message}，总耗时 ${totalElapsedMs}ms`);
            setLastError(message);
          } finally {
            isScanningRef.current = false;
            setIsScanning(false);
          }
        })();
      }, 5000);

      return () => {
        window.clearTimeout(exitTimer);
      };
    }
  }, [activeResult?.jx3Running, activeResult?.processSessions, autoScanEnabled, scanThisWeek]);

  return {
    isScanning,
    lastError,
    lastScanAt,
    refresh,
    scanThisWeek,
  };
};
