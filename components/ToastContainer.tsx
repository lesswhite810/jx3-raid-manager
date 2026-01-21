import React, { useState, useEffect, useCallback } from 'react';
import { ToastMessage } from '../types';
import { CheckCircle2, XCircle, AlertCircle, Info, X } from 'lucide-react';
import { toast, TOAST_EVENT, TOAST_DISMISS_EVENT } from '../utils/toastManager';

// Toast 配置映射
const TOAST_CONFIG = {
  success: {
    bgColor: 'bg-emerald-600 dark:bg-emerald-700',
    textColor: 'text-white',
    icon: CheckCircle2
  },
  error: {
    bgColor: 'bg-red-600 dark:bg-red-700',
    textColor: 'text-white',
    icon: XCircle
  },
  warning: {
    bgColor: 'bg-amber-600 dark:bg-amber-700',
    textColor: 'text-white',
    icon: AlertCircle
  },
  info: {
    bgColor: 'bg-blue-600 dark:bg-blue-700',
    textColor: 'text-white',
    icon: Info
  }
};

export const ToastContainer: React.FC = () => {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  // 移除 Toast
  const removeToast = useCallback((toastId: string) => {
    setToasts(prev => prev.filter(t => t.id !== toastId));
  }, []);

  // 添加 Toast
  const addToast = useCallback((event: Event) => {
    const customEvent = event as CustomEvent<ToastMessage>;
    const newToast = customEvent.detail;

    setToasts(prev => [...prev, newToast]);

    // 自动移除
    if (newToast.duration && newToast.duration > 0) {
      setTimeout(() => {
        removeToast(newToast.id);
      }, newToast.duration);
    }
  }, [removeToast]);

  // 处理关闭事件
  const handleDismiss = useCallback((event: Event) => {
    const customEvent = event as CustomEvent<string>;
    const toastId = customEvent.detail;

    if (toastId === 'all') {
      setToasts([]);
    } else {
      removeToast(toastId);
    }
  }, [removeToast]);

  // 注册事件监听
  useEffect(() => {
    window.addEventListener(TOAST_EVENT, addToast);
    window.addEventListener(TOAST_DISMISS_EVENT, handleDismiss);

    // 标记 Toast 系统已初始化
    toast.markInitialized();

    return () => {
      window.removeEventListener(TOAST_EVENT, addToast);
      window.removeEventListener(TOAST_DISMISS_EVENT, handleDismiss);
      toast.markUninitialized();
    };
  }, [addToast, handleDismiss]);

  return (
    <div className="fixed bottom-8 left-1/2 -translate-x-1/2 z-[9999] flex flex-col-reverse gap-2 pointer-events-none">
      {toasts.map(toastItem => {
        const config = TOAST_CONFIG[toastItem.type];
        const IconComponent = config.icon;

        return (
          <div
            key={toastItem.id}
            className={`${config.bgColor} ${config.textColor} px-6 py-3 rounded-xl shadow-lg flex items-center gap-2 text-sm font-medium animate-in pointer-events-auto`}
            style={{
              animation: 'fadeInUp 0.3s ease-out'
            }}
          >
            <IconComponent className="w-5 h-5 flex-shrink-0" />
            <span className="flex-1">{toastItem.message}</span>

            {toastItem.dismissible && (
              <button
                onClick={() => removeToast(toastItem.id)}
                className="ml-2 hover:opacity-80 transition-opacity flex-shrink-0"
                aria-label="关闭"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
};
