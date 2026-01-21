import { useState, useEffect } from 'react';
import { Database, AlertTriangle, CheckCircle, XCircle, RefreshCw, Trash2, Merge, Search, Shield, FileText, Wrench } from 'lucide-react';
import { diagnoseMigration, forceMigrate, checkLocalStorageData, deduplicateAccounts, analyzeDuplicates, deduplicateRaids, addUniqueConstraint, debugConfig, resetConfig } from '../services/migration';

interface MigrationStatusProps {
  onClose: () => void;
}

interface DiagnosticData {
  localStorage: {
    hasData: boolean;
    accountsCount: number;
    recordsCount: number;
    raidsCount: number;
    hasConfig: boolean;
    dataSize: {
      accounts: number;
      records: number;
      raids: number;
      config: number;
    };
  };
  database: {
    connectionOk: boolean;
    accountsCount: number;
    recordsCount: number;
    raidsCount: number;
    hasConfig: boolean;
  };
  errors: string[];
}

export function MigrationStatus({ onClose }: MigrationStatusProps) {
  const [diagnostic, setDiagnostic] = useState<DiagnosticData | null>(null);
  const [migrating, setMigrating] = useState(false);
  const [migrateResult, setMigrateResult] = useState<any>(null);
  const [deduplicateResult, setDeduplicateResult] = useState<any>(null);
  const [analyzeResult, setAnalyzeResult] = useState<string>('');
  const [configDebugResult, setConfigDebugResult] = useState<string>('');
  const [deduplicating, setDeduplicating] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [constraining, setConstraining] = useState(false);
  const [constraintResult, setConstraintResult] = useState<string>('');
  const [configDebugging, setConfigDebugging] = useState(false);
  const [configResetting, setConfigResetting] = useState(false);
  const [localData, setLocalData] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [debugInfo, setDebugInfo] = useState<string>('');
  const [activeTab, setActiveTab] = useState<'migration' | 'deduplicate' | 'analyze' | 'config'>('migration');

  useEffect(() => {
    runDiagnostics();
  }, []);

  const runDiagnostics = async () => {
    setLoading(true);
    setDebugInfo('正在诊断...');

    try {
      const diag = await diagnoseMigration();
      setDiagnostic(diag);
      setDebugInfo(`诊断完成 - localStorage: ${diag.localStorage.hasData ? '有数据' : '无数据'}, 数据库: ${diag.database.connectionOk ? '已连接' : '未连接'}`);

      const local = await checkLocalStorageData();
      setLocalData(local);
    } catch (error: any) {
      setDebugInfo(`诊断出错: ${error.message}`);
    } finally {
      setLoading(false);
    }
  };

  const handleMigrate = async () => {
    setMigrating(true);
    setMigrateResult(null);

    try {
      const result = await forceMigrate();
      setMigrateResult(result);
      await runDiagnostics();
    } catch (error: any) {
      setMigrateResult({ success: false, message: error.message });
    } finally {
      setMigrating(false);
    }
  };

  const handleDeduplicate = async () => {
    setDeduplicating(true);
    setDeduplicateResult(null);

    try {
      const result = await deduplicateAccounts();
      setDeduplicateResult(result);
      await runDiagnostics();
    } catch (error: any) {
      setDeduplicateResult({ success: false, message: error.message });
    } finally {
      setDeduplicating(false);
    }
  };

  const handleAnalyze = async () => {
    setAnalyzing(true);
    setAnalyzeResult('');

    try {
      const result = await analyzeDuplicates();
      setAnalyzeResult(result);
    } catch (error: any) {
      setAnalyzeResult('分析失败: ' + error.message);
    } finally {
      setAnalyzing(false);
    }
  };

  const handleDeduplicateRaids = async () => {
    setDeduplicating(true);
    setDeduplicateResult(null);

    try {
      const result = await deduplicateRaids();
      setDeduplicateResult({ success: true, message: result });
      await runDiagnostics();
    } catch (error: any) {
      setDeduplicateResult({ success: false, message: error.message });
    } finally {
      setDeduplicating(false);
    }
  };

  const handleAddConstraint = async () => {
    setConstraining(true);
    setConstraintResult('');

    try {
      const result = await addUniqueConstraint();
      setConstraintResult(result);
    } catch (error: any) {
      setConstraintResult('添加约束失败: ' + error.message);
    } finally {
      setConstraining(false);
    }
  };

  const handleConfigDebug = async () => {
    setConfigDebugging(true);
    setConfigDebugResult('');

    try {
      const result = await debugConfig();
      setConfigDebugResult(result);
    } catch (error: any) {
      setConfigDebugResult('调试失败: ' + error.message);
    } finally {
      setConfigDebugging(false);
    }
  };

  const handleConfigReset = async () => {
    setConfigResetting(true);

    try {
      const defaultConfig = {
        fontSize: 14,
        fontSizeSmall: 12,
        fontSizeLarge: 16,
        serverUrl: "https://www.jx3box.com",
        useAiThinking: false,
        autoAnalyzeGkp: true,
        aiModel: "deepseek-chat",
        aiApiUrl: "https://api.deepseek.com",
        aiApiKey: "",
        theme: "system" as const,
        language: "zh" as const,
        autoSave: true,
        notifyOnSave: true,
        compactMode: false,
        showRaidStats: true,
        confirmBeforeDelete: true,
        defaultRaidType: "团队副本",
        dateFormat: "YYYY-MM-DD",
        timeFormat: "24h" as const,
        showDurabilityWarnings: true,
        durabilityThreshold: 30,
        maxRecentRaids: 10,
        enableKeyboardShortcuts: true,
        autoBackupEnabled: false,
        backupIntervalDays: 7
      };

      const result = await resetConfig(JSON.stringify(defaultConfig));
      setConfigDebugResult('重置结果:\n' + result);
      await runDiagnostics();
    } catch (error: any) {
      setConfigDebugResult('重置失败: ' + error.message);
    } finally {
      setConfigResetting(false);
    }
  };

  const handleClearLocalStorage = () => {
    localStorage.removeItem('jx3_accounts');
    localStorage.removeItem('jx3_records');
    localStorage.removeItem('jx3_raids');
    localStorage.removeItem('jx3_config');
    runDiagnostics();
  };

  const handleReloadFromDB = async () => {
    setDebugInfo('重新加载数据库数据...');
    await runDiagnostics();
  };

  return (
    <div className="fixed inset-0 bg-slate-900/60 flex items-center justify-center z-50">
      <div className="bg-surface rounded-lg shadow-xl w-full max-w-2xl max-h-[80vh] overflow-auto">
        <div className="flex items-center justify-between p-4 border-b border-base">
          <h2 className="text-lg font-semibold flex items-center gap-2 text-main">
            <Database className="w-5 h-5" />
            数据迁移与去重管理
          </h2>
          <button
            onClick={onClose}
            className="text-muted hover:text-main"
          >
            ✕
          </button>
        </div>

        <div className="p-4">
          {/* 标签页切换 */}
          <div className="flex border-b border-base mb-4">
            <button
              onClick={() => setActiveTab('migration')}
              className={`px-4 py-2 flex items-center gap-2 ${activeTab === 'migration'
                ? 'border-b-2 border-primary text-primary'
                : 'text-muted hover:text-main'
                }`}
            >
              <RefreshCw className="w-4 h-4" />
              数据迁移
            </button>
            <button
              onClick={() => setActiveTab('deduplicate')}
              className={`px-4 py-2 flex items-center gap-2 ${activeTab === 'deduplicate'
                ? 'border-b-2 border-primary text-primary'
                : 'text-muted hover:text-main'
                }`}
            >
              <Merge className="w-4 h-4" />
              数据去重
            </button>
            <button
              onClick={() => setActiveTab('analyze')}
              className={`px-4 py-2 flex items-center gap-2 ${activeTab === 'analyze'
                ? 'border-b-2 border-primary text-primary'
                : 'text-muted hover:text-main'
                }`}
            >
              <Search className="w-4 h-4" />
              重复分析
            </button>
            <button
              onClick={() => setActiveTab('config')}
              className={`px-4 py-2 flex items-center gap-2 ${activeTab === 'config'
                ? 'border-b-2 border-primary text-primary'
                : 'text-muted hover:text-main'
                }`}
            >
              <Wrench className="w-4 h-4" />
              配置调试
            </button>
          </div>

          {/* 调试信息 */}
          <div className="text-xs text-muted bg-base p-2 rounded font-mono mb-4">
            {debugInfo || '等待诊断...'}
          </div>

          {/* 迁移标签页 */}
          {activeTab === 'migration' && (
            <>
              <div className="flex gap-2 mb-4">
                <button
                  onClick={handleMigrate}
                  disabled={migrating || (localData?.totalItems === 0)}
                  className="flex-1 btn btn-primary flex items-center justify-center gap-2 disabled:opacity-50"
                >
                  {migrating ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : (
                    <Database className="w-4 h-4" />
                  )}
                  {migrating ? '迁移中...' : '执行迁移'}
                </button>

                <button
                  onClick={handleReloadFromDB}
                  className="px-4 py-2 bg-base text-main rounded-lg hover:bg-base/80 flex items-center gap-2 transition-colors border border-base"
                >
                  <RefreshCw className="w-4 h-4" />
                  刷新
                </button>
              </div>

              {/* 迁移结果 */}
              {migrateResult && (
                <div className={`p-3 rounded-lg mb-4 ${migrateResult.success
                  ? 'bg-emerald-50 dark:bg-emerald-900/20 border border-emerald-200 dark:border-emerald-800'
                  : 'bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800'
                  }`}>
                  <div className="flex items-center gap-2">
                    {migrateResult.success ? (
                      <CheckCircle className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
                    ) : (
                      <XCircle className="w-5 h-5 text-red-600 dark:text-red-400" />
                    )}
                    <span className="font-medium text-main">
                      {migrateResult.success ? '迁移成功' : '迁移失败'}
                    </span>
                  </div>
                  <p className="mt-1 text-sm text-muted">
                    {migrateResult.message}
                  </p>
                  {migrateResult.migrated && (
                    <div className="mt-2 text-sm space-y-1">
                      <p className="flex justify-between">
                        <span className="text-muted">新增账号:</span>
                        <span className={migrateResult.migrated.accounts > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                          {migrateResult.migrated.accounts} 个
                        </span>
                      </p>
                      <p className="flex justify-between">
                        <span className="text-muted">新增记录:</span>
                        <span className={migrateResult.migrated.records > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                          {migrateResult.migrated.records} 条
                        </span>
                      </p>
                      <p className="flex justify-between">
                        <span className="text-muted">新增副本:</span>
                        <span className={migrateResult.migrated.raids > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                          {migrateResult.migrated.raids} 个
                        </span>
                      </p>
                    </div>
                  )}
                  {migrateResult.details && (
                    <div className="mt-2 text-xs text-muted border-t border-base pt-2">
                      <p>数据库: {migrateResult.details.dbBefore.accounts} → {migrateResult.details.dbAfter.accounts} 账号</p>
                      {migrateResult.details.skipped.accounts > 0 && (
                        <p className="text-amber-600 dark:text-amber-400">跳过 {migrateResult.details.skipped.accounts} 个重复账号</p>
                      )}
                    </div>
                  )}
                </div>
              )}
            </>
          )}

          {/* 去重标签页 */}
          {activeTab === 'deduplicate' && (
            <>
              <div className="space-y-3 mb-4">
                <button
                  onClick={handleDeduplicate}
                  disabled={deduplicating}
                  className="w-full btn btn-primary flex items-center justify-center gap-2"
                >
                  {deduplicating ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : (
                    <Merge className="w-4 h-4" />
                  )}
                  {deduplicating ? '去重中...' : '账号去重 (保留每个ID第一条)'}
                </button>

                <button
                  onClick={handleDeduplicateRaids}
                  disabled={deduplicating}
                  className="w-full btn btn-secondary flex items-center justify-center gap-2"
                >
                  {deduplicating ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : (
                    <Merge className="w-4 h-4" />
                  )}
                  副本去重 (保留最新日期)'
                </button>

                <button
                  onClick={handleAddConstraint}
                  disabled={constraining}
                  className="w-full btn btn-secondary flex items-center justify-center gap-2"
                >
                  {constraining ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : (
                    <Shield className="w-4 h-4" />
                  )}
                  {constraining ? '添加中...' : '添加唯一性约束 (防止重复)'}
                </button>
              </div>

              {/* 去重结果 */}
              {deduplicateResult && (
                <div className={`p-3 rounded-lg mb-4 ${deduplicateResult.success
                  ? 'bg-emerald-50 dark:bg-emerald-900/20 border border-emerald-200 dark:border-emerald-800'
                  : 'bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800'
                  }`}>
                  <div className="flex items-center gap-2">
                    {deduplicateResult.success ? (
                      <CheckCircle className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
                    ) : (
                      <XCircle className="w-5 h-5 text-red-600 dark:text-red-400" />
                    )}
                    <span className="font-medium text-main">
                      {deduplicateResult.success ? '去重完成' : '去重失败'}
                    </span>
                  </div>
                  <p className="mt-1 text-sm text-muted whitespace-pre-wrap">
                    {deduplicateResult.message}
                  </p>
                  {deduplicateResult.removed > 0 && (
                    <div className="mt-2 text-sm">
                      <p className="text-red-600 dark:text-red-400 font-medium">
                        已删除 {deduplicateResult.removed} 个重复账号
                      </p>
                      <p className="text-emerald-600 dark:text-emerald-400">
                        剩余 {deduplicateResult.remaining} 个唯一账号
                      </p>
                    </div>
                  )}
                </div>
              )}

              {/* 约束结果 */}
              {constraintResult && (
                <div className={`p-3 rounded-lg mb-4 ${constraintResult.includes('✓')
                  ? 'bg-emerald-50 dark:bg-emerald-900/20 border border-emerald-200 dark:border-emerald-800'
                  : 'bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800'
                  }`}>
                  <div className="flex items-center gap-2">
                    {constraintResult.includes('✓') ? (
                      <CheckCircle className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
                    ) : (
                      <AlertTriangle className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                    )}
                    <span className="font-medium text-main">
                      唯一性约束
                    </span>
                  </div>
                  <p className="mt-1 text-sm text-muted whitespace-pre-wrap">
                    {constraintResult}
                  </p>
                </div>
              )}
            </>
          )}

          {/* 分析标签页 */}
          {activeTab === 'analyze' && (
            <>
              <button
                onClick={handleAnalyze}
                disabled={analyzing}
                className="w-full btn btn-primary flex items-center justify-center gap-2 mb-4"
              >
                {analyzing ? (
                  <RefreshCw className="w-4 h-4 animate-spin" />
                ) : (
                  <Search className="w-4 h-4" />
                )}
                {analyzing ? '分析中...' : '执行重复分析'}
              </button>

              {analyzeResult && (
                <div className="bg-surface rounded-lg p-3 mb-4 border border-base">
                  <div className="flex items-center gap-2 mb-2 text-main">
                    <FileText className="w-4 h-4" />
                    <span className="font-medium">分析报告</span>
                  </div>
                  <pre className="text-sm text-muted whitespace-pre-wrap font-mono overflow-auto max-h-64">
                    {analyzeResult}
                  </pre>
                </div>
              )}
            </>
          )}

          {/* 配置调试标签页 */}
          {activeTab === 'config' && (
            <>
              <div className="space-y-3 mb-4">
                <button
                  onClick={handleConfigDebug}
                  disabled={configDebugging}
                  className="w-full btn btn-primary flex items-center justify-center gap-2"
                >
                  {configDebugging ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : (
                    <Search className="w-4 h-4" />
                  )}
                  {configDebugging ? '调试中...' : '调试配置数据'}
                </button>

                <button
                  onClick={handleConfigReset}
                  disabled={configResetting}
                  className="w-full btn btn-danger flex items-center justify-center gap-2"
                >
                  {configResetting ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : (
                    <RefreshCw className="w-4 h-4" />
                  )}
                  {configResetting ? '重置中...' : '重置配置 (恢复默认)'}
                </button>
              </div>

              {configDebugResult && (
                <div className="bg-surface rounded-lg p-3 mb-4 border border-base">
                  <pre className="text-sm text-muted whitespace-pre-wrap font-mono overflow-auto max-h-64">
                    {configDebugResult}
                  </pre>
                </div>
              )}
            </>
          )}

          {/* localStorage 数据状态 */}
          <div className="border border-base rounded-lg overflow-hidden mb-4">
            <div className="bg-base px-3 py-2 font-medium text-sm text-main">
              localStorage 数据 (浏览器存储)
            </div>
            <div className="p-3 space-y-2 text-main">
              {loading ? (
                <p className="text-muted">正在检查...</p>
              ) : localData ? (
                <>
                  <div className="flex justify-between items-center">
                    <span className="text-muted">账号</span>
                    <span className={localData.accountsCount > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                      {localData.accountsCount} 个
                    </span>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-muted">记录</span>
                    <span className={localData.recordsCount > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                      {localData.recordsCount} 条
                    </span>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-muted">副本</span>
                    <span className={localData.raidsCount > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                      {localData.raidsCount} 个
                    </span>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-muted">配置</span>
                    <span className={localData.hasConfig ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                      {localData.hasConfig ? '有' : '无'}
                    </span>
                  </div>
                  {localData.totalItems > 0 && (
                    <button
                      onClick={handleClearLocalStorage}
                      className="mt-2 w-full btn btn-danger btn-sm flex items-center justify-center gap-2"
                    >
                      <Trash2 className="w-4 h-4" />
                      清除 localStorage 数据
                    </button>
                  )}
                </>
              ) : (
                <p className="text-gray-500">无数据</p>
              )}
            </div>
          </div>

          {/* SQLite 数据库状态 */}
          <div className="border border-base rounded-lg overflow-hidden mb-4">
            <div className="bg-base px-3 py-2 font-medium text-sm text-main">
              SQLite 数据库 (C:\Users\[用户]\.jx3-raid-manager\)
            </div>
            <div className="p-3 space-y-2">
              {loading ? (
                <p className="text-gray-500">正在检查...</p>
              ) : diagnostic?.database ? (
                <>
                  <div className={`flex items-center gap-2 ${diagnostic.database.connectionOk ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'
                    }`}>
                    {diagnostic.database.connectionOk ? (
                      <CheckCircle className="w-4 h-4" />
                    ) : (
                      <XCircle className="w-4 h-4" />
                    )}
                    <span className="text-main">
                      {diagnostic.database.connectionOk ? '连接成功' : '连接失败'}
                    </span>
                  </div>

                  <div className="flex justify-between items-center p-2 bg-base rounded">
                    <span className="text-muted">账号</span>
                    <span className={diagnostic.database.accountsCount > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                      {diagnostic.database.accountsCount} 个
                    </span>
                  </div>

                  <div className="flex justify-between items-center p-2 bg-base rounded">
                    <span className="text-muted">记录</span>
                    <span className={diagnostic.database.recordsCount > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                      {diagnostic.database.recordsCount} 条
                    </span>
                  </div>

                  <div className="flex justify-between items-center p-2 bg-base rounded">
                    <span className="text-muted">副本</span>
                    <span className={diagnostic.database.raidsCount > 0 ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                      {diagnostic.database.raidsCount} 个
                    </span>
                  </div>

                  <div className="flex justify-between items-center p-2 bg-base rounded">
                    <span className="text-muted">配置</span>
                    <span className={diagnostic.database.hasConfig ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-muted'}>
                      {diagnostic.database.hasConfig ? '有' : '无'}
                    </span>
                  </div>
                </>
              ) : (
                <p className="text-muted">数据库未连接或出错</p>
              )}
            </div>
          </div>

          {/* 错误信息 */}
          {diagnostic?.errors && diagnostic.errors.length > 0 && (
            <div className="border border-red-200 dark:border-red-800 rounded-lg overflow-hidden">
              <div className="bg-red-50 dark:bg-red-900/20 px-3 py-2 font-medium text-sm text-red-800 dark:text-red-200 flex items-center gap-2">
                <AlertTriangle className="w-4 h-4" />
                错误信息
              </div>
              <div className="p-3 space-y-1">
                {diagnostic.errors.map((error: string, index: number) => (
                  <p key={index} className="text-sm text-red-600 dark:text-red-400">
                    {error}
                  </p>
                ))}
              </div>
            </div>
          )}

          {/* 数据库路径 */}
          <div className="text-xs text-muted text-center mt-4">
            数据库位置: C:\Users\[用户名]\.jx3-raid-manager\jx3-raid-manager.db
          </div>
        </div>
      </div>
    </div>
  );
}
