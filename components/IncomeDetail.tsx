import React, { useMemo, useState } from 'react';
import { ArrowLeft, Coins, ArrowUpCircle, ArrowDownCircle, TrendingUp, Search, Calendar, Trophy } from 'lucide-react';
import { RaidRecord, Account, AccountType, Role } from '../types';

interface IncomeDetailProps {
  records: RaidRecord[];
  accounts: Account[];
  onBack: () => void;
}

interface EnhancedRecord extends RaidRecord {
  displayRoleName: string;
  displayServer: string;
}

export const IncomeDetail: React.FC<IncomeDetailProps> = ({ records, accounts, onBack }) => {
  const [period, setPeriod] = useState<'week' | 'month' | 'all'>('week');
  const [searchTerm, setSearchTerm] = useState('');
  const [activeTab, setActiveTab] = useState<'all' | 'income' | 'expense'>('all');

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
      return {
        ...record,
        displayRoleName: record.roleName || roleInfo.roleName || '未知角色',
        displayServer: record.server || roleInfo.server || '未知服务器'
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

  return (
    <div className="space-y-5">
      <div className="flex items-center gap-4">
        <button
          onClick={onBack}
          className="p-2 hover:bg-slate-100 rounded-lg transition-colors"
        >
          <ArrowLeft className="w-5 h-5 text-slate-600" />
        </button>
        <h2 className="text-2xl font-bold text-slate-800">收支明细</h2>
      </div>

      <div className="flex items-center gap-2 bg-white rounded-lg p-1 shadow-sm border border-slate-200 w-fit">
        <button
          onClick={() => setPeriod('week')}
          className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${
            period === 'week'
              ? 'bg-slate-800 text-white shadow-sm'
              : 'text-slate-600 hover:text-slate-900 hover:bg-slate-50'
          }`}
        >
          本周
        </button>
        <button
          onClick={() => setPeriod('month')}
          className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${
            period === 'month'
              ? 'bg-slate-800 text-white shadow-sm'
              : 'text-slate-600 hover:text-slate-900 hover:bg-slate-50'
          }`}
        >
          本月
        </button>
        <button
          onClick={() => setPeriod('all')}
          className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${
            period === 'all'
              ? 'bg-slate-800 text-white shadow-sm'
              : 'text-slate-600 hover:text-slate-900 hover:bg-slate-50'
          }`}
        >
          全部
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-white rounded-xl p-5 shadow-sm border border-slate-200">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-slate-100 rounded-lg">
              <ArrowUpCircle className="w-6 h-6 text-slate-600" />
            </div>
            <span className="text-slate-600 font-medium">总收入</span>
          </div>
          <p className="text-3xl font-bold text-slate-800">{formatGold(stats.totalIncome)}</p>
          <p className="text-slate-500 text-sm mt-2">代清收入: {formatGold(stats.clientIncome)} 金</p>
        </div>

        <div className="bg-white rounded-xl p-5 shadow-sm border border-slate-200">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-slate-100 rounded-lg">
              <ArrowDownCircle className="w-6 h-6 text-slate-600" />
            </div>
            <span className="text-slate-600 font-medium">总支出</span>
          </div>
          <p className="text-3xl font-bold text-slate-800">{formatGold(stats.totalExpense)}</p>
          <p className="text-slate-500 text-sm mt-2">代清支出: {formatGold(stats.clientExpense)} 金</p>
        </div>

        <div className="bg-white rounded-xl p-5 shadow-sm border border-slate-200">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-slate-100 rounded-lg">
              <TrendingUp className="w-6 h-6 text-slate-600" />
            </div>
            <span className="text-slate-600 font-medium">净收入</span>
          </div>
          <p className={`text-3xl font-bold ${stats.netIncome >= 0 ? 'text-emerald-600' : 'text-rose-600'}`}>
            {stats.netIncome >= 0 ? '' : '-'}{formatGold(Math.abs(stats.netIncome))}
          </p>
          <p className="text-slate-500 text-sm mt-2">代清净入: {formatGold(stats.clientNetIncome)} 金</p>
        </div>
      </div>

      <div className="bg-white rounded-xl shadow-sm border border-slate-200 overflow-hidden">
        <div className="p-4 border-b border-slate-200">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-slate-100 rounded-lg">
                <Coins className="w-5 h-5 text-slate-600" />
              </div>
              <div>
                <h3 className="text-lg font-semibold text-slate-800">收支记录</h3>
                <p className="text-sm text-slate-500">{tabFilteredRecords.length} 条记录</p>
              </div>
            </div>
            <div className="relative">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
              <input
                type="text"
                placeholder="搜索副本或角色"
                value={searchTerm}
                onChange={e => setSearchTerm(e.target.value)}
                className="pl-9 pr-3 py-2 border border-slate-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-slate-400 w-48"
              />
            </div>
          </div>

          <div className="flex items-center gap-2 bg-slate-100 p-1 rounded-lg">
            <button
              onClick={() => setActiveTab('all')}
              className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${
                activeTab === 'all'
                  ? 'bg-white text-slate-900 shadow-sm'
                  : 'text-slate-600 hover:text-slate-900'
              }`}
            >
              全部
            </button>
            <button
              onClick={() => setActiveTab('income')}
              className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${
                activeTab === 'income'
                  ? 'bg-white text-emerald-700 shadow-sm'
                  : 'text-slate-600 hover:text-slate-900'
              }`}
            >
              收入
            </button>
            <button
              onClick={() => setActiveTab('expense')}
              className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${
                activeTab === 'expense'
                  ? 'bg-white text-rose-700 shadow-sm'
                  : 'text-slate-600 hover:text-slate-900'
              }`}
            >
              支出
            </button>
          </div>
        </div>

        <div className="max-h-[500px] overflow-y-auto">
          {tabFilteredRecords.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-slate-400">
              <Coins className="w-12 h-12 text-slate-200 mb-3" />
              <p className="text-base">暂无记录</p>
            </div>
          ) : (
            <div className="divide-y divide-slate-100">
              {tabFilteredRecords.map((record) => {
                const netIncome = record.goldIncome - (record.goldExpense || 0);
                const isPositive = netIncome >= 0;
                
                return (
                  <div key={record.id} className="p-4 hover:bg-slate-50 transition-colors">
                    <div className="flex items-start gap-4">
                      <div className={`w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0 ${
                        isPositive ? 'bg-emerald-50' : 'bg-rose-50'
                      }`}>
                        <Coins className={`w-5 h-5 ${
                          isPositive ? 'text-emerald-500' : 'text-rose-500'
                        }`} />
                      </div>
                      
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-1">
                          <span className="font-medium text-slate-800 truncate">{record.raidName}</span>
                          {record.hasXuanjing && (
                            <span className="flex-shrink-0 inline-flex items-center gap-1 px-1.5 py-0.5 bg-amber-50 text-amber-600 text-xs rounded">
                              <Trophy className="w-3 h-3" />
                              玄晶
                            </span>
                          )}
                        </div>
                        <div className="flex items-center gap-2 text-sm text-slate-500">
                          <span className="truncate">{record.displayRoleName}</span>
                          <span className="text-slate-300">•</span>
                          <span className="truncate">{record.displayServer}</span>
                        </div>
                      </div>

                      <div className="flex-shrink-0 flex items-center gap-6">
                        <div className="flex items-center gap-4">
                          {record.goldIncome > 0 && (
                            <div className="text-right w-20">
                              <p className="text-xs text-slate-400 mb-0.5">收入</p>
                              <p className="text-sm font-semibold text-emerald-600">
                                +{formatGold(record.goldIncome)}
                              </p>
                            </div>
                          )}
                          {record.goldExpense && record.goldExpense > 0 && (
                            <div className="text-right w-20">
                              <p className="text-xs text-slate-400 mb-0.5">支出</p>
                              <p className="text-sm font-semibold text-rose-600">
                                -{formatGold(record.goldExpense)}
                              </p>
                            </div>
                          )}
                        </div>
                        <div className="flex-shrink-0 flex items-center gap-1.5 text-xs text-slate-400 w-20">
                          <Calendar className="w-3 h-3 flex-shrink-0" />
                          <span className="truncate">{formatDate(record.date)}</span>
                        </div>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
