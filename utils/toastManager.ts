import { ToastMessage, ToastOptions, ToastType } from '../types';
import { generateUUID } from './uuid';

// 自定义事件名称
const TOAST_EVENT = 'app:toast:show';
const TOAST_DISMISS_EVENT = 'app:toast:dismiss';

// 默认显示时长
const DEFAULT_DURATION = 3000;

// Toast 管理器类
class ToastManager {
  private isInitialized = false;

  // 初始化检查
  private ensureInitialized() {
    if (!this.isInitialized) {
      console.warn('[Toast] Toast 系统尚未初始化，可能无法正常显示通知');
    }
  }

  // 标记为已初始化（由 ToastContainer 调用）
  markInitialized() {
    this.isInitialized = true;
    console.log('[Toast] Toast 系统已初始化');
  }

  // 标记为未初始化
  markUninitialized() {
    this.isInitialized = false;
    console.log('[Toast] Toast 系统已卸载');
  }

  // 显示 Toast
  private show(type: ToastType, message: string, options?: ToastOptions) {
    try {
      // 参数验证
      if (!message || typeof message !== 'string') {
        console.error('[Toast] 无效的消息内容:', message);
        return;
      }

      this.ensureInitialized();

      const toast: ToastMessage = {
        id: generateUUID(),
        type: options?.type || type,
        message: message.trim(),
        duration: options?.duration ?? DEFAULT_DURATION,
        dismissible: options?.dismissible ?? true
      };

      // 派发自定义事件
      const event = new CustomEvent(TOAST_EVENT, { detail: toast });
      window.dispatchEvent(event);

      console.log(`[Toast] 显示${type}提示: "${toast.message}" (ID: ${toast.id})`);
    } catch (error) {
      console.error('[Toast] 显示提示失败:', error);
    }
  }

  // 便捷方法 - 成功提示
  success(message: string, duration?: number) {
    this.show('success', message, { duration });
  }

  // 便捷方法 - 错误提示
  error(message: string, duration?: number) {
    this.show('error', message, { duration });
  }

  // 便捷方法 - 警告提示
  warning(message: string, duration?: number) {
    this.show('warning', message, { duration });
  }

  // 便捷方法 - 信息提示
  info(message: string, duration?: number) {
    this.show('info', message, { duration });
  }

  // 手动关闭指定 Toast
  dismiss(toastId: string) {
    try {
      if (!toastId || typeof toastId !== 'string') {
        console.warn('[Toast] 无效的toastId:', toastId);
        return;
      }
      window.dispatchEvent(new CustomEvent(TOAST_DISMISS_EVENT, { detail: toastId }));
    } catch (error) {
      console.error('[Toast] 关闭提示失败:', error);
    }
  }

  // 关闭所有 Toast
  dismissAll() {
    try {
      window.dispatchEvent(new CustomEvent(TOAST_DISMISS_EVENT, { detail: 'all' }));
      console.log('[Toast] 已关闭所有提示');
    } catch (error) {
      console.error('[Toast] 关闭所有提示失败:', error);
    }
  }
}

// 导出单例
export const toast = new ToastManager();

// 导出事件名称供 ToastContainer 使用
export { TOAST_EVENT, TOAST_DISMISS_EVENT, DEFAULT_DURATION };

// 封装通用函数
export interface ShowToastParams {
  message: string;
  type?: 'success' | 'error' | 'warning' | 'info';
  duration?: number;
}

/**
 * 显示Toast提示
 * @param params 参数对象
 * @param params.message 提示消息（必填）
 * @param params.type 提示类型，默认为success
 * @param params.duration 显示时长，默认为3000毫秒
 */
export function showToast(params: ShowToastParams): void;
export function showToast(message: string, type?: ToastType, duration?: number): void;
export function showToast(
  messageOrParams: string | ShowToastParams,
  typeOrDuration?: ToastType | number,
  duration?: number
): void {
  let message: string;
  let toastType: ToastType;
  let toastDuration: number | undefined;

  if (typeof messageOrParams === 'object') {
    message = messageOrParams.message;
    toastType = (messageOrParams.type as ToastType) || 'success';
    toastDuration = messageOrParams.duration;
  } else {
    message = messageOrParams;
    toastType = (typeof typeOrDuration === 'string' ? typeOrDuration : 'success') as ToastType;
    toastDuration = typeof typeOrDuration === 'number' ? typeOrDuration : duration;
  }

  // 使用对应的公开方法
  switch (toastType) {
    case 'success':
      toast.success(message, toastDuration);
      break;
    case 'error':
      toast.error(message, toastDuration);
      break;
    case 'warning':
      toast.warning(message, toastDuration);
      break;
    case 'info':
      toast.info(message, toastDuration);
      break;
  }
}

/**
 * 显示成功提示
 * @param message 提示消息
 * @param duration 显示时长（毫秒）
 */
export function showSuccess(message: string, duration?: number): void {
  toast.success(message, duration);
}

/**
 * 显示错误提示
 * @param message 提示消息
 * @param duration 显示时长（毫秒）
 */
export function showError(message: string, duration?: number): void {
  toast.error(message, duration);
}

/**
 * 显示警告提示
 * @param message 提示消息
 * @param duration 显示时长（毫秒）
 */
export function showWarning(message: string, duration?: number): void {
  toast.warning(message, duration);
}

/**
 * 显示信息提示
 * @param message 提示消息
 * @param duration 显示时长（毫秒）
 */
export function showInfo(message: string, duration?: number): void {
  toast.info(message, duration);
}
