/**
 * 分析日志服务
 * 用于记录分析过程中的详细信息，方便调试和问题定位
 */

type LogLevel = 'error' | 'warn' | 'info' | 'debug';

interface LogEntry {
  timestamp: string;
  level: LogLevel;
  module: string;
  message: string;
  data?: unknown;
}

class AnalyzerLogger {
  private logs: LogEntry[] = [];
  private maxLogs = 1000;
  private enabled = true;

  /**
   * 设置日志开关
   */
  setEnabled(enabled: boolean) {
    this.enabled = enabled;
  }

  /**
   * 记录日志
   */
  log(level: LogLevel, module: string, message: string, data?: unknown) {
    if (!this.enabled && level !== 'error') return;

    const entry: LogEntry = {
      timestamp: new Date().toISOString(),
      level,
      module,
      message,
      data,
    };

    this.logs.push(entry);

    // 保持日志数量限制
    if (this.logs.length > this.maxLogs) {
      this.logs.shift();
    }

    // 控制台输出
    const prefix = `[${entry.timestamp.slice(11, 23)}] [${level.toUpperCase().padEnd(5)}] [${module}]`;
    const logFn = level === 'error' ? console.error
      : level === 'warn' ? console.warn
        : level === 'debug' ? console.debug
          : console.log;

    if (data !== undefined) {
      logFn(prefix, message, data);
    } else {
      logFn(prefix, message);
    }
  }

  error(module: string, message: string, data?: unknown) {
    this.log('error', module, message, data);
  }

  warn(module: string, message: string, data?: unknown) {
    this.log('warn', module, message, data);
  }

  info(module: string, message: string, data?: unknown) {
    this.log('info', module, message, data);
  }

  debug(module: string, message: string, data?: unknown) {
    this.log('debug', module, message, data);
  }

  /**
   * 获取所有日志
   */
  getLogs(): LogEntry[] {
    return [...this.logs];
  }

  /**
   * 按级别过滤日志
   */
  getLogsByLevel(level: LogLevel): LogEntry[] {
    return this.logs.filter(log => log.level === level);
  }

  /**
   * 按模块过滤日志
   */
  getLogsByModule(module: string): LogEntry[] {
    return this.logs.filter(log => log.module === module);
  }

  /**
   * 导出日志为文本
   */
  exportLogs(): string {
    return this.logs.map(entry => {
      const dataStr = entry.data ? ` | ${JSON.stringify(entry.data)}` : '';
      return `[${entry.timestamp}] [${entry.level.toUpperCase().padEnd(5)}] [${entry.module}] ${entry.message}${dataStr}`;
    }).join('\n');
  }

  /**
   * 清空日志
   */
  clear() {
    this.logs = [];
  }

  /**
   * 获取日志数量
   */
  getCount(): number {
    return this.logs.length;
  }

  /**
   * 获取错误日志数量
   */
  getErrorCount(): number {
    return this.logs.filter(log => log.level === 'error').length;
  }

  /**
   * 获取警告日志数量
   */
  getWarnCount(): number {
    return this.logs.filter(log => log.level === 'warn').length;
  }
}

// 单例实例
export const analyzerLogger = new AnalyzerLogger();

// 模块名称常量
export const LOG_MODULES = {
  ANALYZER: 'RaidAnalyzer',
  SCANNER: 'FileScanner',
  PARSER: 'ChatLogParser',
  MAPPER: 'DropMapper',
  FILLER: 'RecordFiller',
  UI: 'AutoAnalyzer',
} as const;
