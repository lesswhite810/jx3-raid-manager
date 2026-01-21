import React, { useState, useEffect, useRef } from 'react';

interface DebugConsoleProps {
  maxLogs?: number;
}

export const DebugConsole: React.FC<DebugConsoleProps> = ({ maxLogs = 50 }) => {
  const [logs, setLogs] = useState<string[]>([]);
  const [isVisible, setIsVisible] = useState(false);
  const originalConsole = useRef({
    log: console.log,
    error: console.error,
    warn: console.warn,
    info: console.info,
    debug: console.debug
  });

  useEffect(() => {
    // 重写console方法，将日志保存到state
    const logTypes = ['log', 'error', 'warn', 'info', 'debug'] as const;

    logTypes.forEach(type => {
      console[type] = (...args: any[]) => {
        // 调用原始方法
        originalConsole.current[type](...args);

        // 格式化日志
        const timestamp = new Date().toLocaleTimeString();
        const formattedArgs = args
          .map(arg => {
            if (typeof arg === 'object' && arg !== null) {
              try {
                return JSON.stringify(arg);
              } catch (e) {
                return String(arg);
              }
            }
            return String(arg);
          })
          .join(' ');

        const logEntry = `${timestamp} [${type.toUpperCase()}]: ${formattedArgs}`;

        // 更新日志列表
        setLogs(prevLogs => {
          const newLogs = [...prevLogs, logEntry];
          // 只保留最新的maxLogs条
          if (newLogs.length > maxLogs) {
            return newLogs.slice(-maxLogs);
          }
          return newLogs;
        });
      };
    });

    // 清理函数，恢复原始console
    return () => {
      console.log = originalConsole.current.log;
      console.error = originalConsole.current.error;
      console.warn = originalConsole.current.warn;
      console.info = originalConsole.current.info;
      console.debug = originalConsole.current.debug;
    };
  }, [maxLogs]);

  // 导出日志到文本文件
  const exportLogs = () => {
    const logText = logs.join('\n');
    const blob = new Blob([logText], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `jx3-raid-manager-logs-${new Date().toISOString().slice(0, 10)}.txt`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  return (
    <>
      {/* 开关按钮 */}
      <button
        onClick={() => setIsVisible(!isVisible)}
        className={`fixed bottom-4 right-4 px-3 py-1.5 text-xs rounded-lg transition-colors z-50 ${isVisible
          ? 'bg-base text-muted hover:bg-surface'
          : 'bg-primary/10 text-primary hover:bg-primary/20'}`}
        title={isVisible ? '隐藏日志' : '显示日志'}
      >
        {isVisible ? '🔇 日志' : '📝 日志'}
      </button>

      {/* 日志面板 */}
      {isVisible && (
        <div className="fixed bottom-16 right-4 w-96 max-h-96 bg-surface border border-base rounded-lg shadow-xl z-50 flex flex-col overflow-hidden">
          <div className="bg-base p-2 flex justify-between items-center text-sm font-medium text-main">
            <span>控制台日志</span>
            <div className="flex gap-1">
              <button
                onClick={exportLogs}
                className="px-2 py-0.5 text-xs bg-surface text-muted rounded hover:bg-base border border-base"
              >
                导出
              </button>
              <button
                onClick={() => setLogs([])}
                className="px-2 py-0.5 text-xs bg-surface text-muted rounded hover:bg-base border border-base"
              >
                清空
              </button>
            </div>
          </div>
          <div className="flex-1 overflow-y-auto p-2 text-sm text-muted font-mono bg-surface">
            {logs.length === 0 ? (
              <div className="text-muted/50 italic">暂无日志</div>
            ) : (
              <div className="space-y-1">
                {logs.map((log, index) => (
                  <div key={index} className={`${log.includes('[ERROR]')
                      ? 'text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20 p-1 rounded'
                      : log.includes('[WARN]')
                        ? 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-900/20 p-1 rounded'
                        : 'text-muted'
                    }`}>
                    {log}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
};
