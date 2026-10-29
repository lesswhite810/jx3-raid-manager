// 统一 IPC 出口：检测宿主环境，浏览器预览时自动回落到 mockInvoke，
// 避免纯浏览器 `npm run dev` 下直接调用 Tauri invoke 抛未捕获异常。
//
// 约定：
// 1. 业务代码一律通过本文件调用后端命令，不要再直接 import '@tauri-apps/api/core'。
// 2. 本文件必须自成一体：不能 import services/db.ts（db.ts 会 import 本文件，否则循环依赖），
//    也不依赖任何业务模块。
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { mockInvoke } from './mockInvoke';

/**
 * 是否运行在 Tauri 宿主内。
 *
 * __TAURI_INTERNALS__ 由 Tauri 注入，纯浏览器 / Node 环境下不存在，
 * 类型声明见 src/tauri.d.ts。
 */
export const isTauriRuntime = (): boolean =>
  typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__);

/**
 * 调用 Tauri 后端命令的唯一入口。
 *
 * 浏览器环境下自动走 mockInvoke，返回与后端 camelCase 结构一致的 mock 数据；
 * 其余环境直接透传给 Tauri invoke，错误照常向上抛出。
 *
 * @param cmd 后端命令名（snake_case，与 src-tauri 中的 #[tauri::command] 一致）
 * @param args 命令参数（camelCase）
 */
export const invokeCommand = async <T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> => {
  if (!isTauriRuntime()) {
    return mockInvoke<T>(cmd, args);
  }
  return tauriInvoke<T>(cmd, args);
};
