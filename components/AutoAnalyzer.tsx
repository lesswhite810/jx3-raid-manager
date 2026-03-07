/**
 * 自动分析组件
 *
 * 功能：
 * 1. 显示数据源状态
 * 2. 选择时间范围和打工角色
 * 3. 执行分析并展示结果
 * 4. 填充记录到数据库
 */

import React, { useState, useMemo, useCallback } from 'react';
import {
  Play,
  Loader2,
  CheckCircle2,
  AlertCircle,
  FolderOpen,
  Folder,
  Filter,
  User,
  List,
  ChevronDown,
  ChevronUp,
  RefreshCw,
  FileText,
  TrendingUp,
  ArrowRightToLine,
  Check,
} from 'lucide-react';
import { Account, Config, TimeRangeType, AnalysisResult, GroupedAnalysisResults } from '../types';
import { formatDateTime } from '../utils/timeRange';
import { RaidAnalyzer, mapToRaidRecord, mapToBaizhanRecord, AnalysisStats } from '../services/raidAnalyzer';
import { analyzerLogger, LOG_MODULES } from '../services/analyzerLogger';
import { getDropTypeTags } from '../services/dropMapping';
import { db } from '../services/db';
import { toast } from '../utils/toastManager';

const MODULE = LOG_MODULES.UI;

// 简单的类名合并工具函数
function cn(...classes: (string | boolean | undefined | null)[]): string {
  return classes.filter(Boolean).join(' ');
}

// ============================================================================
// 类型定义
// ============================================================================

interface AutoAnalyzerProps {
  config?: Config;
  accounts: Account[];
  onRecordAdded: () => void;
  onBaizhanRecordAdded: () => void;
}

interface FlatRole {
  id: string;
  name: string;
  server: string;
  accountId: string;
  accountName: string;
  disabled: boolean;
}

// ============================================================================
// 辅助函数
// ============================================================================

/**
 * 扁平化角色列表（只返回启用的角色）
 */
function flattenRoles(accounts: Account[]): FlatRole[] {
  return accounts.flatMap(account => {
    if (account.disabled) return [];

    const roles = Array.isArray(account.roles) ? account.roles : [];
    return roles
      .filter(role => !role.disabled)
      .map(role => ({
        id: role.id,
        name: role.name,
        server: `${role.region} ${role.server}`,
        accountId: account.id,
        accountName: account.accountName,
        disabled: role.disabled || false,
      }));
  });
}

// ============================================================================
// 组件
// ============================================================================

export const AutoAnalyzer: React.FC<AutoAnalyzerProps> = ({
  config,
  accounts,
  onRecordAdded,
  onBaizhanRecordAdded,
}) => {
  // 获取游戏目录
  const gameDirectory = config?.game?.gameDirectory || '';

  // 扁平化角色
  const activeRoles = useMemo(() => flattenRoles(accounts), [accounts]);

  // 状态
  const [timeRange, setTimeRange] = useState<TimeRangeType>('week');
  const [selectedRoles, setSelectedRoles] = useState<Set<string>>(new Set());
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [progress, setProgress] = useState({ percent: 0, currentFile: '' });
  const [analysisComplete, setAnalysisComplete] = useState(false);
  const [results, setResults] = useState<AnalysisResult[]>([]);
  const [stats, setStats] = useState<AnalysisStats | null>(null);
  const [selectedResultIds, setSelectedResultIds] = useState<Set<string>>(new Set());
  const [typeFilter, setTypeFilter] = useState<'all' | 'raid' | 'baizhan'>('all');
  const [statusFilter, setStatusFilter] = useState<'all' | 'pending' | 'filled'>('all');
  const [expandedResults, setExpandedResults] = useState<Set<string>>(new Set());
  const [showLogModal, setShowLogModal] = useState(false);

  // 角色选择
  const toggleRole = useCallback((roleId: string) => {
    setSelectedRoles(prev => {
      const newSet = new Set(prev);
      if (newSet.has(roleId)) {
        newSet.delete(roleId);
      } else {
        newSet.add(roleId);
      }
      return newSet;
    });
  }, []);

  const handleSelectAllRoles = useCallback(() => {
    setSelectedRoles(new Set(activeRoles.map(r => r.id)));
  }, [activeRoles]);

  const handleClearRoles = useCallback(() => {
    setSelectedRoles(new Set());
  }, []);

  // 开始分析
  const handleStartAnalysis = useCallback(async () => {
    if (!gameDirectory || selectedRoles.size === 0) return;

    setIsAnalyzing(true);
    setAnalysisComplete(false);
    setResults([]);
    setStats(null);
    setSelectedResultIds(new Set());
    analyzerLogger.clear();

    analyzerLogger.info(MODULE, '开始分析', {
      gameDirectory,
      timeRange,
      selectedRolesCount: selectedRoles.size,
    });

    try {
      const analyzer = new RaidAnalyzer({
        gameDir: gameDirectory,
        timeRange,
        selectedRoleIds: selectedRoles,
      });

      const { results: analysisResults, stats: analysisStats } = await analyzer.analyze(
        (p) => setProgress(p)
      );

      setResults(analysisResults);
      setStats(analysisStats);
      setAnalysisComplete(true);

      toast.success(`分析完成！发现 ${analysisResults.length} 条记录`);
    } catch (error) {
      analyzerLogger.error(MODULE, '分析失败', {
        error: error instanceof Error ? error.message : String(error),
      });
      toast.error(`分析失败: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setIsAnalyzing(false);
    }
  }, [gameDirectory, timeRange, selectedRoles]);

  // 结果选择
  const toggleResultSelection = useCallback((uid: string) => {
    setSelectedResultIds(prev => {
      const newSet = new Set(prev);
      if (newSet.has(uid)) {
        newSet.delete(uid);
      } else {
        newSet.add(uid);
      }
      return newSet;
    });
  }, []);

  const toggleResultExpanded = useCallback((uid: string) => {
    setExpandedResults(prev => {
      const newSet = new Set(prev);
      if (newSet.has(uid)) {
        newSet.delete(uid);
      } else {
        newSet.add(uid);
      }
      return newSet;
    });
  }, []);

  // 填充单条记录
  const handleFillResult = useCallback(async (result: AnalysisResult) => {
    try {
      if (result.dungeonType === 'raid') {
        const record = mapToRaidRecord(result);
        await db.addRecord(record);
        onRecordAdded();
      } else {
        const record = mapToBaizhanRecord(result);
        await db.addBaizhanRecord(record);
        onBaizhanRecordAdded();
      }

      // 标记为已填充
      setResults(prev =>
        prev.map(r =>
          r.uid === result.uid ? { ...r, isFilled: true, filledAt: Date.now() } : r
        )
      );

      toast.success(`已填充: ${result.dungeonName}`);
    } catch (error) {
      toast.error(`填充失败: ${error instanceof Error ? error.message : String(error)}`);
    }
  }, [onRecordAdded, onBaizhanRecordAdded]);

  // 批量填充
  const handleBatchFill = useCallback(async () => {
    const toFill = results.filter(
      r => selectedResultIds.has(r.uid) && !r.isFilled
    );

    if (toFill.length === 0) return;

    let successCount = 0;
    let raidCount = 0;
    let baizhanCount = 0;

    for (const result of toFill) {
      try {
        if (result.dungeonType === 'raid') {
          const record = mapToRaidRecord(result);
          await db.addRecord(record);
          raidCount++;
        } else {
          const record = mapToBaizhanRecord(result);
          await db.addBaizhanRecord(record);
          baizhanCount++;
        }

        setResults(prev =>
          prev.map(r =>
            r.uid === result.uid ? { ...r, isFilled: true, filledAt: Date.now() } : r
          )
        );

        successCount++;
      } catch (error) {
        analyzerLogger.error(MODULE, `填充失败: ${result.dungeonName}`, {
          error: error instanceof Error ? error.message : String(error),
        });
      }
    }

    if (successCount > 0) {
      onRecordAdded();
      onBaizhanRecordAdded();
      toast.success(`已填充 ${raidCount} 条团队副本，${baizhanCount} 条百战记录`);
    }

    setSelectedResultIds(new Set());
  }, [results, selectedResultIds, onRecordAdded, onBaizhanRecordAdded]);

  // 按角色分组结果
  const groupedResults = useMemo((): GroupedAnalysisResults[] => {
    const groups = new Map<string, GroupedAnalysisResults>();

    // 先按筛选条件过滤
    const filtered = results.filter(r => {
      if (typeFilter !== 'all' && r.dungeonType !== typeFilter) return false;
      if (statusFilter === 'pending' && r.isFilled) return false;
      if (statusFilter === 'filled' && !r.isFilled) return false;
      return true;
    });

    for (const result of filtered) {
      const key = result.workerRoleId;
      if (!groups.has(key)) {
        groups.set(key, {
          roleId: result.workerRoleId,
          roleName: result.workerName,
          results: [],
          selectedCount: 0,
          raidCount: 0,
          baizhanCount: 0,
        });
      }

      const group = groups.get(key)!;
      group.results.push(result);

      if (result.dungeonType === 'raid') {
        group.raidCount++;
      } else {
        group.baizhanCount++;
      }

      if (selectedResultIds.has(result.uid)) {
        group.selectedCount++;
      }
    }

    return Array.from(groups.values());
  }, [results, typeFilter, statusFilter, selectedResultIds]);

  // 计算总选中数
  const totalSelected = selectedResultIds.size;
  const totalResults = results.length;

  // 渲染
  return (
    <div className="space-y-5">
      {/* 数据源卡片 */}
      <div className="flex items-center justify-between p-4 bg-surface rounded-xl border border-base">
        <div className="flex items-center gap-4 flex-wrap">
          <div className="flex items-center gap-2">
            <FolderOpen className="w-4 h-4 text-muted" />
            <span className="text-sm text-muted">数据源</span>
          </div>

          <div className="flex items-center gap-1.5 px-3 py-1.5 bg-base/50 rounded-lg">
            <Folder className="w-4 h-4 text-amber-500" />
            <span className="text-sm text-main font-mono">
              {gameDirectory || '未配置'}
            </span>
          </div>
        </div>

        <button
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-primary hover:text-primary-hover transition-colors"
          onClick={handleStartAnalysis}
          disabled={!gameDirectory}
        >
          <RefreshCw className="w-4 h-4" />
          刷新
        </button>
      </div>

      {/* 未配置提示 */}
      {!gameDirectory && (
        <div className="flex items-center gap-2 p-3 bg-amber-50 dark:bg-amber-900/10 rounded-lg border border-amber-200 dark:border-amber-800">
          <AlertCircle className="w-4 h-4 text-amber-500" />
          <span className="text-sm text-amber-600 dark:text-amber-400">
            请先在「配置」页面设置游戏安装目录
          </span>
        </div>
      )}

      {/* 筛选条件卡片 */}
      <div className="bg-surface p-5 rounded-xl border border-base space-y-5">
        <div className="flex items-center gap-2">
          <Filter className="w-4 h-4 text-muted" />
          <span className="font-medium text-main">筛选条件</span>
        </div>

        {/* 时间范围 */}
        <div>
          <label className="block text-xs text-muted mb-2">时间范围</label>
          <div className="flex gap-2">
            {[
              { key: 'week' as TimeRangeType, label: '最近一周' },
              { key: 'month' as TimeRangeType, label: '最近一个月' },
              { key: 'all' as TimeRangeType, label: '全部' },
            ].map(option => (
              <button
                key={option.key}
                onClick={() => setTimeRange(option.key)}
                className={cn(
                  'px-4 py-2 text-sm rounded-lg transition-all',
                  timeRange === option.key
                    ? 'bg-primary text-white shadow-sm'
                    : 'bg-base/50 text-muted hover:text-main hover:bg-base'
                )}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>

        {/* 打工角色多选 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-xs text-muted">
              打工角色
              {selectedRoles.size > 0 && (
                <span className="ml-1 text-primary">
                  (已选 {selectedRoles.size}/{activeRoles.length})
                </span>
              )}
            </label>
            <div className="flex gap-2">
              <button
                className="text-xs text-primary hover:text-primary-hover"
                onClick={handleSelectAllRoles}
              >
                全选
              </button>
              <button
                className="text-xs text-muted hover:text-main"
                onClick={handleClearRoles}
              >
                清空
              </button>
            </div>
          </div>

          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2 max-h-40 overflow-y-auto p-3 bg-base/30 rounded-lg border border-base">
            {activeRoles.length === 0 ? (
              <div className="col-span-full text-center text-sm text-muted py-4">
                暂无启用的角色
              </div>
            ) : (
              activeRoles.map(role => (
                <label
                  key={role.id}
                  className={cn(
                    'flex items-center gap-2 p-2 rounded-lg cursor-pointer transition-all border',
                    selectedRoles.has(role.id)
                      ? 'bg-primary/10 border-primary/30'
                      : 'bg-base/50 border-transparent hover:border-base'
                  )}
                >
                  <input
                    type="checkbox"
                    checked={selectedRoles.has(role.id)}
                    onChange={() => toggleRole(role.id)}
                    className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                  />
                  <div className="flex-1 min-w-0">
                    <div className="text-sm text-main truncate">{role.name}</div>
                    <div className="text-xs text-muted truncate">{role.server}</div>
                  </div>
                </label>
              ))
            )}
          </div>
        </div>

        {/* 开始分析按钮 */}
        <button
          className={cn(
            'w-full py-3 rounded-lg font-medium transition-all flex items-center justify-center gap-2',
            isAnalyzing || !gameDirectory || selectedRoles.size === 0
              ? 'bg-base text-muted cursor-not-allowed'
              : 'bg-primary text-white hover:bg-primary-hover shadow-md shadow-primary/20'
          )}
          disabled={isAnalyzing || !gameDirectory || selectedRoles.size === 0}
          onClick={handleStartAnalysis}
        >
          {isAnalyzing ? (
            <>
              <Loader2 className="w-4 h-4 animate-spin" />
              分析中...
            </>
          ) : (
            <>
              <Play className="w-4 h-4" />
              开始分析
            </>
          )}
        </button>

        {/* 进度条 */}
        {isAnalyzing && (
          <div className="space-y-2">
            <div className="flex justify-between text-xs text-muted">
              <span>{progress.currentFile}</span>
              <span>{progress.percent}%</span>
            </div>
            <div className="h-2 bg-base rounded-full overflow-hidden">
              <div
                className="h-full bg-primary transition-all duration-300"
                style={{ width: `${progress.percent}%` }}
              />
            </div>
          </div>
        )}

        {/* 分析完成提示 */}
        {analysisComplete && stats && (
          <div className="p-3 bg-emerald-50 dark:bg-emerald-900/10 border border-emerald-200 dark:border-emerald-800 rounded-lg">
            <div className="flex items-center gap-2 mb-2">
              <CheckCircle2 className="w-4 h-4 text-emerald-500" />
              <span className="text-sm text-emerald-700 dark:text-emerald-300 font-medium">
                分析完成！
              </span>
            </div>
            <div className="ml-6 text-xs text-emerald-600 dark:text-emerald-400 space-y-0.5">
              <div>
                团队副本: {stats.raidCount} 条 · 百战: {stats.baizhanCount} 条
              </div>
              <div className="text-muted">
                耗时: {stats.duration}ms
              </div>
            </div>
          </div>
        )}
      </div>

      {/* 分析结果列表 */}
      {results.length > 0 && (
        <div className="bg-surface rounded-xl border border-base">
          {/* 头部工具栏 */}
          <div className="p-4 border-b border-base">
            <div className="flex items-center justify-between flex-wrap gap-3">
              <div className="flex items-center gap-3">
                <List className="w-4 h-4 text-muted" />
                <span className="font-medium text-main">分析结果</span>
                <span className="text-sm text-muted">
                  共 {totalResults} 条 · 已选 {totalSelected} 条
                </span>
              </div>

              <div className="flex items-center gap-3 flex-wrap">
                {/* 类型筛选 */}
                <select
                  className="px-2 py-1.5 bg-base/50 border border-base rounded-lg text-xs text-main"
                  value={typeFilter}
                  onChange={e => setTypeFilter(e.target.value as 'all' | 'raid' | 'baizhan')}
                >
                  <option value="all">全部类型</option>
                  <option value="raid">团队副本</option>
                  <option value="baizhan">百战</option>
                </select>

                {/* 状态筛选 */}
                <select
                  className="px-2 py-1.5 bg-base/50 border border-base rounded-lg text-xs text-main"
                  value={statusFilter}
                  onChange={e => setStatusFilter(e.target.value as 'all' | 'pending' | 'filled')}
                >
                  <option value="all">全部状态</option>
                  <option value="pending">待填充</option>
                  <option value="filled">已填充</option>
                </select>

                {/* 查看日志 */}
                <button
                  onClick={() => setShowLogModal(true)}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-muted hover:text-main bg-base/50 rounded-lg hover:bg-base transition-colors"
                >
                  <FileText className="w-3.5 h-3.5" />
                  查看日志
                </button>

                {/* 批量填充 */}
                <button
                  className={cn(
                    'px-3 py-1.5 text-xs rounded-lg transition-colors',
                    totalSelected > 0
                      ? 'bg-primary text-white hover:bg-primary-hover'
                      : 'bg-base text-muted cursor-not-allowed'
                  )}
                  disabled={totalSelected === 0}
                  onClick={handleBatchFill}
                >
                  批量填充 ({totalSelected})
                </button>
              </div>
            </div>
          </div>

          {/* 按角色分组的结果列表 */}
          <div className="p-4 space-y-5 max-h-[600px] overflow-y-auto">
            {groupedResults.map(group => (
              <div key={group.roleId} className="space-y-2">
                {/* 角色分组标题 */}
                <div className="flex items-center justify-between py-1.5 border-b border-base/50">
                  <div className="flex items-center gap-2">
                    <User className="w-4 h-4 text-muted" />
                    <span className="font-medium text-main text-sm">{group.roleName}</span>
                    <span className="text-xs text-muted">{group.results.length} 条</span>
                    <span className="text-xs">
                      <span className="text-blue-600">团队 {group.raidCount}</span>
                      <span className="text-muted mx-1">·</span>
                      <span className="text-purple-600">百战 {group.baizhanCount}</span>
                    </span>
                  </div>
                </div>

                {/* 该角色的结果卡片 */}
                <div className="space-y-2">
                  {group.results.map(result => (
                    <ResultCard
                      key={result.uid}
                      result={result}
                      isSelected={selectedResultIds.has(result.uid)}
                      isExpanded={expandedResults.has(result.uid)}
                      onToggleSelect={() => toggleResultSelection(result.uid)}
                      onToggleExpand={() => toggleResultExpanded(result.uid)}
                      onFill={() => handleFillResult(result)}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 空状态 */}
      {results.length === 0 && !isAnalyzing && analysisComplete && (
        <div className="text-center py-12 text-muted bg-surface rounded-xl border border-base">
          <List className="w-12 h-12 mx-auto mb-3 opacity-20" />
          <p>暂无分析结果</p>
          <p className="text-xs mt-1">选择角色后点击「开始分析」</p>
        </div>
      )}

      {/* 日志弹窗 */}
      {showLogModal && (
        <LogModal onClose={() => setShowLogModal(false)} />
      )}
    </div>
  );
};

// ============================================================================
// 子组件
// ============================================================================

/** 结果卡片 */
const ResultCard: React.FC<{
  result: AnalysisResult;
  isSelected: boolean;
  isExpanded: boolean;
  onToggleSelect: () => void;
  onToggleExpand: () => void;
  onFill: () => void;
}> = ({ result, isSelected, isExpanded, onToggleSelect, onToggleExpand, onFill }) => {
  const dropTags = getDropTypeTags(result.specialItems);

  return (
    <div
      className={cn(
        'rounded-lg border transition-all',
        isSelected
          ? 'bg-primary/5 border-primary/30'
          : result.isFilled
            ? 'bg-base/20 border-base opacity-60'
            : 'bg-base/30 border-base hover:border-primary/20'
      )}
    >
      {/* 头部行 */}
      <div className="flex items-center gap-3 p-3">
        <input
          type="checkbox"
          checked={isSelected}
          onChange={onToggleSelect}
          disabled={result.isFilled}
          className="w-4 h-4 text-primary rounded border-base focus:ring-primary disabled:opacity-50 flex-shrink-0"
        />

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-xs text-muted font-mono">
              {formatDateTime(result.startTime)}
            </span>
          </div>
        </div>

        <button
          onClick={onToggleExpand}
          className="p-1 text-muted hover:text-main transition-colors"
        >
          {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
        </button>
      </div>

      {/* 简略信息（收起状态） */}
      {!isExpanded && (
        <div className="px-3 pb-3 flex items-center justify-between">
          <div className="flex items-center gap-2 flex-wrap">
            <TypeTag type={result.dungeonType} />
            <span className="font-medium text-main text-sm">
              {result.playerCount ? `${result.playerCount}人` : ''}{result.difficulty || ''}{result.dungeonName}
            </span>
            {result.isFilled && (
              <span className="px-1.5 py-0.5 bg-emerald-100 dark:bg-emerald-900/30 text-emerald-600 dark:text-emerald-400 text-[10px] rounded">
                已填充
              </span>
            )}
            {dropTags.length > 0 && (
              <div className="flex items-center gap-1">
                {dropTags.slice(0, 3).map((tag, idx) => (
                  <span key={idx} className="text-amber-600 text-xs">
                    [{tag.label}]
                  </span>
                ))}
              </div>
            )}
          </div>
          <div className="flex items-center gap-3">
            <span className={cn(
              'font-medium text-sm',
              result.netIncome >= 0 ? 'text-emerald-600' : 'text-amber-600'
            )}>
              净收入: {result.netIncome >= 0 ? '+' : ''}{result.netIncome.toLocaleString()}金
            </span>
            <FillButton isFilled={result.isFilled} onFill={onFill} />
          </div>
        </div>
      )}

      {/* 详细信息（展开状态） */}
      {isExpanded && (
        <div className="px-3 pb-3 space-y-3">
          {/* 副本信息行 */}
          <div className="flex items-center justify-between p-2.5 bg-base/50 rounded-lg">
            <div className="flex items-center gap-2">
              <TypeTag type={result.dungeonType} />
              <span className="font-medium text-main">
                {result.playerCount ? `${result.playerCount}人` : ''}{result.difficulty || ''}{result.dungeonName}
              </span>
            </div>
            {result.dungeonType === 'raid' && (
              <span className="text-xs text-muted">团长: {result.leaderName}</span>
            )}
          </div>

          {/* 收支明细 */}
          <div className="p-3 bg-base/30 rounded-lg space-y-1.5">
            <div className="text-xs font-medium text-muted mb-2 flex items-center gap-1">
              <TrendingUp className="w-3.5 h-3.5" />
              收支明细
            </div>

            <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
              <div className="flex justify-between">
                <span className="text-muted">工资收入</span>
                <span className="text-emerald-600 font-medium">
                  +{result.personalIncome.toLocaleString()} 金
                </span>
              </div>

              {result.scatteredConsumption > 0 && (
                <div className="flex justify-between">
                  <span className="text-muted">敷件消费</span>
                  <span className="text-amber-600">
                    -{result.scatteredConsumption.toLocaleString()} 金
                  </span>
                </div>
              )}

              {result.ironConsumption > 0 && (
                <div className="flex justify-between">
                  <span className="text-muted">小铁消费</span>
                  <span className="text-amber-600">
                    -{result.ironConsumption.toLocaleString()} 金
                  </span>
                </div>
              )}

              {result.specialConsumption > 0 && (
                <div className="flex justify-between">
                  <span className="text-muted">特殊消费</span>
                  <span className="text-amber-600">
                    -{result.specialConsumption.toLocaleString()} 金
                  </span>
                </div>
              )}

              {result.otherConsumption > 0 && (
                <div className="flex justify-between">
                  <span className="text-muted">其他消费</span>
                  <span className="text-amber-600">
                    -{result.otherConsumption.toLocaleString()} 金
                  </span>
                </div>
              )}

              {result.fine > 0 && (
                <div className="flex justify-between">
                  <span className="text-muted">罚款</span>
                  <span className="text-amber-600">
                    -{result.fine.toLocaleString()} 金
                  </span>
                </div>
              )}
            </div>

            <div className="border-t border-base/50 pt-2 mt-2 flex justify-between font-medium">
              <span className="text-main">净收入</span>
              <span className={cn(
                result.netIncome >= 0 ? 'text-emerald-600' : 'text-amber-600'
              )}>
                {result.netIncome >= 0 ? '+' : ''}{result.netIncome.toLocaleString()} 金
              </span>
            </div>
          </div>

          {/* 特殊掉落 */}
          {result.specialItems.length > 0 && (
            <div className="p-3 bg-amber-50/50 dark:bg-amber-900/10 rounded-lg border border-amber-100 dark:border-amber-800/50">
              <div className="text-xs font-medium text-amber-700 dark:text-amber-400 mb-2">
                特殊掉落
              </div>

              {/* 识别出的掉落类型标签 */}
              {dropTags.length > 0 && (
                <div className="flex flex-wrap gap-1.5 mb-2">
                  {dropTags.map(tag => (
                    <span
                      key={tag.type}
                      className="px-2 py-0.5 text-xs rounded-full bg-amber-100 dark:bg-amber-800/30 text-amber-700 dark:text-amber-400 border border-amber-200 dark:border-amber-700/50"
                    >
                      {tag.label}
                    </span>
                  ))}
                </div>
              )}

              {/* 具体物品列表 */}
              <div className="text-xs text-amber-600 dark:text-amber-400 space-y-0.5">
                {result.specialItems.map((item, idx) => (
                  <div key={idx} className="flex items-center gap-1">
                    <span>• {item.name}</span>
                    <span className="text-muted">({item.price.toLocaleString()}金)</span>
                    {item.isWorkerBought && (
                      <span className="text-amber-500">(自购)</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* 备注 */}
          {result.notes && (
            <div className="text-xs text-muted italic px-2">
              📝 {result.notes}
            </div>
          )}

          {/* 操作按钮 */}
          <div className="flex justify-end pt-2">
            <FillButton isFilled={result.isFilled} onFill={onFill} />
          </div>
        </div>
      )}
    </div>
  );
};

/** 类型标签 */
const TypeTag: React.FC<{ type: 'raid' | 'baizhan' | 'trial' }> = ({ type }) => (
  <span
    className={cn(
      'px-2 py-0.5 text-[10px] font-medium rounded flex-shrink-0',
      type === 'raid' || type === 'trial'
        ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400'
        : 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400'
    )}
  >
    {type === 'raid' || type === 'trial' ? '团队' : '百战'}
  </span>
);

/** 填充按钮 */
const FillButton: React.FC<{ isFilled: boolean; onFill: () => void }> = ({ isFilled, onFill }) => (
  <button
    className={cn(
      'px-3 py-1.5 text-xs rounded-lg transition-colors flex items-center gap-1.5',
      isFilled
        ? 'bg-base text-muted cursor-not-allowed'
        : 'bg-primary text-white hover:bg-primary-hover'
    )}
    disabled={isFilled}
    onClick={onFill}
  >
    {isFilled ? (
      <>
        <Check className="w-3.5 h-3.5" />
        已填充
      </>
    ) : (
      <>
        <ArrowRightToLine className="w-3.5 h-3.5" />
        填充
      </>
    )}
  </button>
);

/** 日志弹窗 */
const LogModal: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const [filter, setFilter] = useState<'all' | 'error' | 'warn' | 'info' | 'debug'>('all');
  const logs = analyzerLogger.getLogs();

  const filteredLogs = logs.filter(log => {
    if (filter === 'all') return true;
    return log.level === filter;
  });

  const handleExport = () => {
    const content = analyzerLogger.exportLogs();
    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `analyzer-log-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="fixed inset-0 bg-slate-900/60 z-50 flex items-center justify-center p-4">
      <div className="bg-surface rounded-xl shadow-2xl w-full max-w-4xl max-h-[80vh] flex flex-col">
        {/* 头部 */}
        <div className="flex items-center justify-between p-4 border-b border-base">
          <h2 className="text-lg font-semibold text-main">分析日志</h2>
          <div className="flex items-center gap-3">
            <select
              className="px-2 py-1 bg-base/50 border border-base rounded text-sm"
              value={filter}
              onChange={e => setFilter(e.target.value as typeof filter)}
            >
              <option value="all">全部级别</option>
              <option value="error">ERROR</option>
              <option value="warn">WARN</option>
              <option value="info">INFO</option>
              <option value="debug">DEBUG</option>
            </select>
            <button
              onClick={handleExport}
              className="px-3 py-1 text-sm bg-primary text-white rounded hover:bg-primary-hover"
            >
              导出日志
            </button>
            <button onClick={onClose} className="p-1 text-muted hover:text-main">
              ✕
            </button>
          </div>
        </div>

        {/* 日志列表 */}
        <div className="flex-1 overflow-auto p-4 font-mono text-xs">
          {filteredLogs.length === 0 ? (
            <div className="text-center text-muted py-8">暂无日志</div>
          ) : (
            <div className="space-y-1">
              {filteredLogs.map((log, idx) => (
                <div
                  key={idx}
                  className={cn(
                    'p-2 rounded',
                    log.level === 'error' && 'bg-red-50 dark:bg-red-900/10 text-red-700 dark:text-red-400',
                    log.level === 'warn' && 'bg-amber-50 dark:bg-amber-900/10 text-amber-700 dark:text-amber-400',
                    log.level === 'info' && 'bg-blue-50 dark:bg-blue-900/10 text-blue-700 dark:text-blue-400',
                    log.level === 'debug' && 'bg-base/50 text-muted'
                  )}
                >
                  <span className="text-muted mr-2">{log.timestamp.slice(11, 23)}</span>
                  <span className={cn(
                    'px-1 rounded text-[10px] font-bold mr-2',
                    log.level === 'error' && 'bg-red-200 text-red-800',
                    log.level === 'warn' && 'bg-amber-200 text-amber-800',
                    log.level === 'info' && 'bg-blue-200 text-blue-800',
                    log.level === 'debug' && 'bg-gray-200 text-gray-600'
                  )}>
                    {log.level.toUpperCase().padEnd(5)}
                  </span>
                  <span className="text-primary mr-2">[{log.module}]</span>
                  <span>{log.message}</span>
                  {log.data !== undefined && log.data !== null && (
                    <pre className="mt-1 ml-4 text-muted text-[10px] overflow-x-auto">
                      {JSON.stringify(log.data, null, 2)}
                    </pre>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default AutoAnalyzer;
