import React, { useMemo, useState } from 'react';
import { ArrowLeft, Coins, TrendingUp, TrendingDown, Search, Calendar, Trash2, Pencil, Sparkles, Ghost, Package, Flag, Shirt, Crown, Anchor, ChevronDown } from 'lucide-react';
import { RaidRecord, Account, AccountType, Role } from '../types';
import { toast } from '../utils/toastManager';

interface IncomeDetailProps {
  records: RaidRecord[];
  accounts: Account[];
  onBack: () => void;
  onDeleteRecord: (recordId: string) => void;
  onEditRecord: (record: RaidRecord) => void;
}

interface EnhancedRecord extends RaidRecord {
  displayRoleName: string;
  displayServer: string;
}

export const IncomeDetail: React.FC<IncomeDetailProps> = ({ records, accounts, onBack, onDeleteRecord, onEditRecord }) => {
  const [period, setPeriod] = useState<'week' | 'month' | 'all'>('week');
  const [searchTerm, setSearchTerm] = useState('');
  const [activeTab, setActiveTab] = useState<'all' | 'income' | 'expense'>('all');
  const [deleteConfirmRecordId, setDeleteConfirmRecordId] = useState<string | null>(null);
  const [expandedRecordId, setExpandedRecordId] = useState<string | null>(null);

  const safeRecords = Array.isArray(records) ? records : [];
  const safeAccounts = Array.isArray(accounts) ? accounts : [];

  const findRoleInfo = (accountId: string, roleId: string): { roleName: string; server: string } => {
    for (const account of safeAccounts) {
      if (account.id === accountId) {
        const role = account.roles?.find((r: Role) => r.id === roleId);
        if (role) {
          return {
            roleName: role.name,
            server: `${role.region} ${role.server}`
          };
        }
      }
    }
    return { roleName: '', server: '' };
  };

  const enhancedRecords = useMemo<EnhancedRecord[]>(() => {
    return safeRecords.map(record => {
      const roleInfo = findRoleInfo(record.accountId, record.roleId);
      const roleName = record.roleName || roleInfo.roleName || '未知角色';

      // 优先使用最新的 roleInfo.server，如果不存在则使用 record.server
      let rawServer = roleInfo.server || record.server || '未知服务器';

      // 清洗 server 字段，移除可能包含的角色名
      // 例如：如果 server 是 "梦江南 风闪"，而 roleName 是 "风闪"，则替换为 "梦江南"
      if (roleName && roleName !== '未知角色') {
        rawServer = rawServer.replace(new RegExp(`\\s*${roleName}\\s*`, 'g'), ' ').trim();
      }

      return {
        ...record,
        displayRoleName: roleName,
        displayServer: rawServer
      };
    });
  }, [safeRecords, safeAccounts]);

  const filteredRecords = useMemo(() => {
    const now = new Date();
    const startOfPeriod = new Date();

    if (period === 'week') {
      const dayOfWeek = now.getDay();
      const diff = dayOfWeek === 0 ? 6 : dayOfWeek - 1;
      startOfPeriod.setDate(now.getDate() - diff);
    } else if (period === 'month') {
      startOfPeriod.setDate(1);
    } else {
      startOfPeriod.setFullYear(now.getFullYear() - 10);
    }

    startOfPeriod.setHours(0, 0, 0, 0);

    return enhancedRecords.filter(r => new Date(r.date) >= startOfPeriod);
  }, [enhancedRecords, period]);

  const searchedRecords = useMemo(() => {
    return filteredRecords.filter(r =>
      r.raidName.toLowerCase().includes(searchTerm.toLowerCase()) ||
      r.displayRoleName.toLowerCase().includes(searchTerm.toLowerCase())
    );
  }, [filteredRecords, searchTerm]);

  const tabFilteredRecords = useMemo(() => {
    if (activeTab === 'income') {
      return searchedRecords.filter(r => r.goldIncome > 0);
    } else if (activeTab === 'expense') {
      return searchedRecords.filter(r => (r.goldExpense || 0) > 0);
    }
    return searchedRecords;
  }, [searchedRecords, activeTab]);

  const stats = useMemo(() => {
    const totalIncome = filteredRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    const totalExpense = filteredRecords.reduce((acc, r) => acc + (r.goldExpense || 0), 0);
    const netIncome = totalIncome - totalExpense;
    const xuanjingCount = filteredRecords.filter(r => r.hasXuanjing).length;

    const clientAccountIds = safeAccounts
      .filter(a => a.type === AccountType.CLIENT && !a.disabled)
      .map(a => a.id);
    const clientIncome = filteredRecords
      .filter(r => clientAccountIds.includes(r.accountId))
      .reduce((acc, r) => acc + r.goldIncome, 0);

    const clientExpense = filteredRecords
      .filter(r => clientAccountIds.includes(r.accountId))
      .reduce((acc, r) => acc + (r.goldExpense || 0), 0);

    return {
      totalIncome,
      totalExpense,
      netIncome,
      xuanjingCount,
      clientIncome,
      clientExpense,
      clientNetIncome: clientIncome - clientExpense
    };
  }, [filteredRecords, safeAccounts]);

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    const now = new Date();
    const isToday = date.toDateString() === now.toDateString();

    if (isToday) {
      return `今天 ${date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`;
    }

    return date.toLocaleDateString('zh-CN', {
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  };

  const formatGold = (amount: number) => {
    if (amount >= 10000) {
      return `${(amount / 10000).toFixed(1)}w`;
    }
    return amount.toLocaleString();
  };

  const handleDeleteClick = (recordId: string) => {
    setDeleteConfirmRecordId(recordId);
  };

  const handleConfirmDelete = () => {
    if (deleteConfirmRecordId) {
      onDeleteRecord(deleteConfirmRecordId);
      toast.success('删除成功');
      setDeleteConfirmRecordId(null);
    }
  };

  const handleCancelDelete = () => {
    setDeleteConfirmRecordId(null);
  };

  return (
    <div className="space-y-5">
      <div className="flex items-center gap-4">
        <button
          onClick={onBack}
          className="p-2 hover:bg-base rounded-lg transition-colors"
        >
          <ArrowLeft className="w-5 h-5 text-muted" />
        </button>
        <h2 className="text-2xl font-bold text-main">收支明细</h2>
      </div>

      <div className="flex items-center gap-2 bg-surface rounded-lg p-1 shadow-sm border border-base w-fit">
        <button
          onClick={() => setPeriod('week')}
          className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${period === 'week'
            ? 'bg-primary text-white shadow-sm'
            : 'text-muted hover:text-main hover:bg-base'
            }`}
        >
          本周
        </button>
        <button
          onClick={() => setPeriod('month')}
          className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${period === 'month'
            ? 'bg-primary text-white shadow-sm'
            : 'text-muted hover:text-main hover:bg-base'
            }`}
        >
          本月
        </button>
        <button
          onClick={() => setPeriod('all')}
          className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${period === 'all'
            ? 'bg-primary text-white shadow-sm'
            : 'text-muted hover:text-main hover:bg-base'
            }`}
        >
          全部
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-surface rounded-xl p-5 shadow-sm border border-base">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-base rounded-lg">
              <TrendingUp className="w-6 h-6 text-muted" />
            </div>
            <span className="text-muted font-medium">总收入</span>
          </div>
          <p className="text-3xl font-bold text-main">{formatGold(stats.totalIncome)}</p>
          <p className="text-muted text-sm mt-2">代清收入: {formatGold(stats.clientIncome)} 金</p>
        </div>

        <div className="bg-surface rounded-xl p-5 shadow-sm border border-base">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-base rounded-lg">
              <TrendingDown className="w-6 h-6 text-muted" />
            </div>
            <span className="text-muted font-medium">总支出</span>
          </div>
          <p className="text-3xl font-bold text-main">{formatGold(stats.totalExpense)}</p>
          <p className="text-muted text-sm mt-2">代清支出: {formatGold(stats.clientExpense)} 金</p>
        </div>

        <div className="bg-surface rounded-xl p-5 shadow-sm border border-base">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-base rounded-lg">
              <TrendingUp className="w-6 h-6 text-muted" />
            </div>
            <span className="text-muted font-medium">净收入</span>
          </div>
          <p className={`text-3xl font-bold ${stats.netIncome >= 0 ? 'text-emerald-600' : 'text-amber-600'}`}>
            {stats.netIncome >= 0 ? '' : '-'}{formatGold(Math.abs(stats.netIncome))}
          </p>
          <p className="text-muted text-sm mt-2">代清净入: {formatGold(stats.clientNetIncome)} 金</p>
        </div>
      </div>

      <div className="bg-surface rounded-xl shadow-sm border border-base overflow-hidden">
        <div className="p-4 border-b border-base">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-base rounded-lg">
                <Coins className="w-5 h-5 text-muted" />
              </div>
              <div>
                <h3 className="text-lg font-semibold text-main">收支记录</h3>
                <p className="text-sm text-muted">{tabFilteredRecords.length} 条记录</p>
              </div>
            </div>
            <div className="relative">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
              <input
                type="text"
                placeholder="搜索副本或角色"
                value={searchTerm}
                onChange={e => setSearchTerm(e.target.value)}
                className="pl-9 pr-3 py-2 bg-surface border border-base rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary w-48 text-main placeholder:text-muted/50"
              />
            </div>
          </div>

          <div className="flex items-center gap-2 bg-base p-1 rounded-lg">
            <button
              onClick={() => setActiveTab('all')}
              className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === 'all'
                ? 'bg-surface text-main shadow-sm'
                : 'text-muted hover:text-main'
                }`}
            >
              全部
            </button>
            <button
              onClick={() => setActiveTab('income')}
              className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === 'income'
                ? 'bg-surface text-emerald-700 dark:text-emerald-400 shadow-sm'
                : 'text-muted hover:text-main'
                }`}
            >
              收入
            </button>
            <button
              onClick={() => setActiveTab('expense')}
              className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === 'expense'
                ? 'bg-surface text-rose-700 dark:text-rose-400 shadow-sm'
                : 'text-muted hover:text-main'
                }`}
            >
              支出
            </button>
          </div>
        </div>

        <div className="max-h-[500px] overflow-y-auto">
          {tabFilteredRecords.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-muted">
              <Coins className="w-12 h-12 text-muted/30 mb-3" />
              <p className="text-[1rem]">暂无记录</p>
            </div>
          ) : (
            <div className="divide-y divide-base">
              {tabFilteredRecords.map((record) => {
                const netIncome = record.goldIncome - (record.goldExpense || 0);
                const isExpanded = expandedRecordId === record.id;

                return (
                  <div
                    key={record.id}
                    className={`transition-colors border-l-4 ${isExpanded
                      ? 'bg-base/50 border-primary'
                      : 'hover:bg-base border-transparent'
                      }`}
                  >
                    {/* Summary Row - Click to Expand */}
                    <div
                      className="p-4 cursor-pointer flex items-center gap-4"
                      onClick={() => setExpandedRecordId(isExpanded ? null : record.id)}
                    >
                      {/* Left: Raid Info & Role */}
                      <div className="flex items-center gap-3 min-w-0 w-[30%] flex-shrink-0">
                        <div className={`w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0 transition-colors ${isExpanded ? 'bg-primary/10 text-primary' : 'bg-base text-muted'
                          }`}>
                          <Coins className="w-5 h-5" />
                        </div>

                        <div className="min-w-0">
                          <div className="font-semibold text-main truncate text-[1rem] leading-tight mb-0.5">
                            {record.raidName}
                          </div>
                          <div className="flex items-center gap-1.5 text-xs text-muted truncate">
                            <span className="truncate max-w-[100px]">{record.displayRoleName}</span>
                            <span className="text-muted/40">•</span>
                            <span className="truncate max-w-[120px]">{record.displayServer}</span>
                          </div>
                        </div>
                      </div>

                      {/* Center: Special Badges */}
                      <div className="flex-1 flex flex-wrap gap-1.5 items-center justify-start content-center min-h-[24px]">
                        {(record.hasXuanjing || record.hasMaJu || record.hasPet || record.hasPendant || record.hasMount || record.hasAppearance || record.hasTitle) && (
                          <>
                            {record.hasXuanjing && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-400 text-xs font-medium rounded border border-amber-100 dark:border-amber-800 flex-shrink-0">
                                <Sparkles className="w-3 h-3" /> 玄晶
                              </span>
                            )}
                            {record.hasMaJu && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400 text-xs font-medium rounded border border-blue-100 dark:border-blue-800 flex-shrink-0">
                                <Anchor className="w-3 h-3" /> 马具
                              </span>
                            )}
                            {record.hasPet && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-purple-50 dark:bg-purple-900/20 text-purple-700 dark:text-purple-400 text-xs font-medium rounded border border-purple-100 dark:border-purple-800 flex-shrink-0">
                                <Ghost className="w-3 h-3" /> 宠物
                              </span>
                            )}
                            {record.hasPendant && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-orange-50 dark:bg-orange-900/20 text-orange-700 dark:text-orange-400 text-xs font-medium rounded border border-orange-100 dark:border-orange-800 flex-shrink-0">
                                <Package className="w-3 h-3" /> 挂件
                              </span>
                            )}
                            {record.hasMount && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-emerald-50 dark:bg-emerald-900/20 text-emerald-700 dark:text-emerald-400 text-xs font-medium rounded border border-emerald-100 dark:border-emerald-800 flex-shrink-0">
                                <Flag className="w-3 h-3" /> 坐骑
                              </span>
                            )}
                            {record.hasAppearance && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-pink-50 dark:bg-pink-900/20 text-pink-700 dark:text-pink-400 text-xs font-medium rounded border border-pink-100 dark:border-pink-800 flex-shrink-0">
                                <Shirt className="w-3 h-3" /> 外观
                              </span>
                            )}
                            {record.hasTitle && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-yellow-50 dark:bg-yellow-900/20 text-yellow-700 dark:text-yellow-400 text-xs font-medium rounded border border-yellow-100 dark:border-yellow-800 flex-shrink-0">
                                <Crown className="w-3 h-3" /> 称号
                              </span>
                            )}
                          </>
                        )}
                      </div>

                      {/* Right: Net Income, Time, Actions */}
                      <div className="flex items-center gap-4 flex-shrink-0">
                        {/* Net Income */}
                        <div className="text-right flex flex-col items-end w-24">
                          <span className={`font-bold text-[1rem] tabular-nums ${netIncome > 0 ? 'text-emerald-600 dark:text-emerald-400' :
                            netIncome < 0 ? 'text-amber-600 dark:text-amber-400' : 'text-muted'
                            }`}>
                            {netIncome > 0 ? '+' : ''}{formatGold(netIncome)}
                          </span>
                          <span className="text-xs text-muted flex items-center gap-1">
                            {formatDate(record.date).split(' ')[0]}
                          </span>
                        </div>

                        {/* Actions */}
                        <div className="flex items-center gap-1">
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              onEditRecord(record);
                            }}
                            className="p-2 rounded-lg text-muted hover:text-primary hover:bg-surface active:scale-95 transition-all"
                            title="修改"
                          >
                            <Pencil size={18} />
                          </button>
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDeleteClick(record.id);
                            }}
                            className="p-2 rounded-lg text-muted hover:text-red-600 hover:bg-surface active:scale-95 transition-all"
                            title="删除"
                          >
                            <Trash2 size={18} />
                          </button>
                          <div className={`p-1 text-muted transition-transform duration-200 ${isExpanded ? 'rotate-180' : ''}`}>
                            <ChevronDown size={20} />
                          </div>
                        </div>
                      </div>
                    </div>

                    {/* Detail Panel - Expanded Only */}
                    {isExpanded && (
                      <div className="px-4 pb-4 pt-0 pl-[4.5rem] animate-in slide-in-from-top-2 duration-200">
                        <div className="space-y-3 pt-2 border-t border-base/50">

                          {/* Income & Expense Breakdown */}
                          <div className="flex gap-6 text-sm">
                            <div className="flex items-center gap-2 text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-900/10 px-2 py-1 rounded">
                              <TrendingUp className="w-3.5 h-3.5" />
                              <span className="font-medium">收入: {formatGold(record.goldIncome)}</span>
                            </div>
                            {(record.goldExpense || 0) > 0 && (
                              <div className="flex items-center gap-2 text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-900/10 px-2 py-1 rounded">
                                <TrendingDown className="w-3.5 h-3.5" />
                                <span className="font-medium">支出: {formatGold(record.goldExpense || 0)}</span>
                              </div>
                            )}
                          </div>

                          {/* Notes */}
                          {record.notes && (
                            <div className="bg-base/50 p-2.5 rounded-md text-sm text-muted border border-base">
                              <div className="break-all">{record.notes}</div>
                            </div>
                          )}

                          {/* Full Date */}
                          <div className="flex items-center gap-1.5 text-xs text-muted/60 mt-2">
                            <Calendar className="w-3 h-3" />
                            {formatDate(record.date)}
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* Delete Confirmation Dialog */}
      {deleteConfirmRecordId && (
        <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50">
          <div className="bg-surface p-6 rounded-lg shadow-xl border border-base max-w-sm w-full mx-4">
            <h3 className="text-lg font-semibold text-main mb-4">确认删除</h3>
            <p className="text-slate-600 mb-6">确认删除这条收支记录吗？此操作不可撤销。</p>
            <div className="flex justify-end gap-3">
              <button
                onClick={handleCancelDelete}
                className="px-4 py-2 bg-surface hover:bg-base border border-base text-main hover:border-primary hover:text-primary active:scale-[0.98] rounded-lg transition-all duration-200 font-medium"
              >
                取消
              </button>
              <button
                onClick={handleConfirmDelete}
                className="px-4 py-2 bg-red-600 hover:bg-red-700 text-white shadow-sm hover:shadow active:scale-[0.98] rounded-lg transition-all duration-200 font-medium"
              >
                确定
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
