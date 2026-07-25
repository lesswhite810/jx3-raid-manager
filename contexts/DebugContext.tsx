import React, { createContext, useContext, useState } from 'react';

/**
 * Debug 模式上下文
 *
 * 用途：用户不可见的隐藏开关，仅本次会话有效（不持久化，重启后自动关闭）。
 * 触发方式：在「系统配置」→「版本与更新」→「当前版本」文字上连续点击 7 次。
 *
 * 当前启用 debug 模式后可见的功能：
 * - 系统配置：JCL 缓存清理按钮
 * - 待确认记录面板：「扫描本月」下拉选项
 */
interface DebugContextType {
  /** 是否启用 debug 模式 */
  debugEnabled: boolean;
  /** 开启或关闭 debug 模式 */
  setDebugEnabled: (enabled: boolean) => void;
  /** 切换 debug 模式（开启 ↔ 关闭） */
  toggleDebug: () => void;
}

const DebugContext = createContext<DebugContextType | undefined>(undefined);

export const DebugProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [debugEnabled, setDebugEnabled] = useState<boolean>(false);

  const toggleDebug = () => setDebugEnabled(prev => !prev);

  return (
    <DebugContext.Provider value={{ debugEnabled, setDebugEnabled, toggleDebug }}>
      {children}
    </DebugContext.Provider>
  );
};

export const useDebug = (): DebugContextType => {
  const context = useContext(DebugContext);
  if (context === undefined) {
    throw new Error('useDebug must be used within a DebugProvider');
  }
  return context;
};
