import { useEffect, useRef } from 'react';

/**
 * 可取消的副作用 hook。
 *
 * 解决两类常见问题：
 * 1. 组件卸载后异步回调仍然 setState（React 会警告，且可能写入已失效状态）
 * 2. 依赖快速变化时，旧请求的结果后到达并覆盖新结果（竞态）
 *
 * 用法：
 * ```tsx
 * useCancellableEffect((signal) => {
 *   db.getAccounts().then(list => {
 *     if (signal.cancelled) return;
 *     setAccounts(list);
 *   });
 * }, [someDep]);
 * ```
 *
 * @param effect 副作用函数，接收一个 signal；清理时机由本 hook 管理
 * @param deps 依赖数组，语义与 useEffect 一致
 */
export function useCancellableEffect(
  effect: (signal: CancellationSignal) => void | (() => void),
  deps: readonly unknown[],
): void {
  const cleanupRef = useRef<void | (() => void)>(undefined);

  useEffect(() => {
    const signal: CancellationSignal = { cancelled: false };

    const result = effect(signal);
    cleanupRef.current = result;

    return () => {
      signal.cancelled = true;
      if (typeof result === 'function') {
        result();
      }
      cleanupRef.current = undefined;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}

/** 副作用取消信号：清理后 `cancelled` 变为 true */
export interface CancellationSignal {
  cancelled: boolean;
}
