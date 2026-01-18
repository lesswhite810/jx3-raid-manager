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
          ? 'bg-slate-100 text-slate-600 hover:bg-slate-200' 
          : 'bg-indigo-100 text-indigo-700 hover:bg-indigo-200'}`}
        title={isVisible ? '隐藏日志' : '显示日志'}
      >
        {isVisible ? '🔇 日志' : '📝 日志'}
      </button>
      
      {/* 日志面板 */}
      {isVisible && (
        <div className="fixed bottom-16 right-4 w-96 max-h-96 bg-white border border-slate-200 rounded-lg shadow-xl z-50 flex flex-col overflow-hidden">
          <div className="bg-slate-100 p-2 flex justify-between items-center text-sm font-medium text-slate-700">
            <span>控制台日志</span>
            <div className="flex gap-1">
              <button
                onClick={exportLogs}
                className="px-2 py-0.5 text-xs bg-slate-200 text-slate-600 rounded hover:bg-slate-300"
              >
                导出
              </button>
              <button
                onClick={() => setLogs([])}
                className="px-2 py-0.5 text-xs bg-slate-200 text-slate-600 rounded hover:bg-slate-300"
              >
                清空
              </button>
            </div>
          </div>
          <div className="flex-1 overflow-y-auto p-2 text-sm text-slate-600 font-mono">
            {logs.length === 0 ? (
              <div className="text-slate-400 italic">暂无日志</div>
            ) : (
              <div className="space-y-1">
                {logs.map((log, index) => (
                  <div key={index} className={`${
                    log.includes('[ERROR]')
                      ? 'text-red-600 bg-red-50 p-1 rounded'
                      : log.includes('[WARN]')
                      ? 'text-yellow-600 bg-yellow-50 p-1 rounded'
                      : 'text-slate-600'
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
