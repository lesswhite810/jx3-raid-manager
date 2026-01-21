import React, { useMemo, useState } from 'react';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Cell } from 'recharts';
import { RaidRecord, Account, AccountType, DashboardStats } from '../types';
import { Coins, Trophy, Shield, TrendingUp, Star, Zap, FileText, ArrowRight } from 'lucide-react';

interface DashboardProps {
  records: RaidRecord[];
  accounts: Account[];
  onShowIncomeDetail: () => void;
  onShowCrystalDetail: () => void;
}

export const Dashboard: React.FC<DashboardProps> = ({ records, accounts, onShowIncomeDetail, onShowCrystalDetail }) => {
  const [statsPeriod, setStatsPeriod] = useState<'week' | 'month'>('week');

  const safeRecords = Array.isArray(records) ? records : [];
  const safeAccounts = Array.isArray(accounts) ? accounts : [];

  const filteredRecords = useMemo(() => {
    const now = new Date();
    const startOfPeriod = new Date();

    if (statsPeriod === 'week') {
      const dayOfWeek = now.getDay();
      const diff = dayOfWeek === 0 ? 6 : dayOfWeek - 1;
      startOfPeriod.setDate(now.getDate() - diff);
    } else {
      startOfPeriod.setDate(1);
    }

    startOfPeriod.setHours(0, 0, 0, 0);

    return safeRecords.filter(r => new Date(r.date) >= startOfPeriod);
  }, [safeRecords, statsPeriod]);

  const stats: DashboardStats = useMemo(() => {
    const totalGold = filteredRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    const xuanjingCount = filteredRecords.filter(r => r.hasXuanjing).length;
    const dropRate = filteredRecords.length > 0 ? (xuanjingCount / filteredRecords.length) * 100 : 0;

    const clientAccountIds = safeAccounts
      .filter(a => a.type === AccountType.CLIENT && !a.disabled)
      .map(a => a.id);
    const clientIncome = filteredRecords
      .filter(r => clientAccountIds.includes(r.accountId))
      .reduce((acc, r) => acc + r.goldIncome, 0);

    return {
      totalGold,
      totalRaids: filteredRecords.length,
      xuanjingCount,
      dropRate,
      clientIncome
    };
  }, [filteredRecords, safeAccounts]);

  const chartData = useMemo(() => {
    const grouped: Record<string, number> = {};
    filteredRecords.forEach(r => {
      grouped[r.raidName] = (grouped[r.raidName] || 0) + r.goldIncome;
    });
    return Object.keys(grouped).map(k => ({ name: k, value: grouped[k] }));
  }, [filteredRecords]);

  const luckyRole = useMemo(() => {
    const roleMap = new Map<string, { roleName: string; server: string; totalGold: number; xuanjingCount: number }>();

    filteredRecords.forEach(r => {
      const roleId = r.roleId || r.accountId;
      const roleName = r.roleName || '未知角色';
      const server = r.server || '未知服务器';

      if (!roleMap.has(roleId)) {
        roleMap.set(roleId, { roleName, server, totalGold: 0, xuanjingCount: 0 });
      }

      const role = roleMap.get(roleId)!;
      role.totalGold += r.goldIncome;
      if (r.hasXuanjing) role.xuanjingCount++;
    });

    let maxRole: typeof roleMap extends Map<string, infer V> ? V : never = { roleName: '暂无数据', server: '', totalGold: 0, xuanjingCount: 0 };
    roleMap.forEach(role => {
      if (role.totalGold > maxRole.totalGold) {
        maxRole = role;
      }
    });

    return maxRole;
  }, [filteredRecords]);

  const bigSpender = useMemo(() => {
    const roleMap = new Map<string, { roleName: string; server: string; totalExpense: number }>();

    const ownAccountIds = new Set(
      safeAccounts
        .filter(a => a.type === AccountType.OWN && !a.disabled)
        .map(a => a.id)
    );

    filteredRecords
      .filter(r => ownAccountIds.has(r.accountId))
      .forEach(r => {
        if (r.goldExpense && r.goldExpense > 0) {
          const roleId = r.roleId || r.accountId;
          const roleName = r.roleName || '未知角色';
          const server = r.server || '未知服务器';

          if (!roleMap.has(roleId)) {
            roleMap.set(roleId, { roleName, server, totalExpense: 0 });
          }

          const role = roleMap.get(roleId)!;
          role.totalExpense += r.goldExpense;
        }
      });

    let maxRole: typeof roleMap extends Map<string, infer V> ? V : never = { roleName: '暂无数据', server: '', totalExpense: 0 };
    roleMap.forEach(role => {
      if (role.totalExpense > maxRole.totalExpense) {
        maxRole = role;
      }
    });

    return maxRole;
  }, [filteredRecords, safeAccounts]);

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-main">数据概览</h2>
        <div className="flex items-center gap-2 bg-surface rounded-lg p-1 shadow-sm border border-base">
          <button
            onClick={() => setStatsPeriod('week')}
            className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${statsPeriod === 'week'
                ? 'bg-emerald-500 text-white shadow-sm'
                : 'text-muted hover:text-main hover:bg-base'
              }`}
          >
            本周
          </button>
          <button
            onClick={() => setStatsPeriod('month')}
            className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${statsPeriod === 'month'
                ? 'bg-amber-500 text-white shadow-sm'
                : 'text-muted hover:text-main hover:bg-base'
              }`}
          >
            本月
          </button>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div className="bg-gradient-to-br from-violet-600 via-purple-600 to-indigo-600 rounded-2xl p-5 text-white shadow-lg">
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2.5 bg-white/20 rounded-xl">
                <Star className="w-5 h-5 text-amber-300" />
              </div>
              <div>
                <p className="text-purple-100 text-sm">本期欧皇</p>
                <h3 className="text-xl font-bold mt-1">{luckyRole.roleName}</h3>
                <p className="text-purple-200 text-xs mt-0.5">{luckyRole.server}</p>
              </div>
            </div>
            <div className="flex items-center gap-6">
              <div className="text-right">
                <p className="text-purple-200 text-xs">金币收益</p>
                <p className="text-2xl font-bold text-amber-300 mt-0.5">{luckyRole.totalGold.toLocaleString()}</p>
              </div>
              {luckyRole.xuanjingCount > 0 && (
                <div className="text-right">
                  <p className="text-purple-200 text-xs">玄晶</p>
                  <p className="text-2xl font-bold text-pink-300 mt-0.5">{luckyRole.xuanjingCount}</p>
                </div>
              )}
            </div>
          </div>
          <div className="flex items-center gap-1.5 mt-3 pt-2.5 border-t border-white/20">
            <Zap className="w-3.5 h-3.5 text-amber-300" />
            <span className="text-xs text-purple-100">收入最高的角色</span>
          </div>
        </div>

        <div className="bg-gradient-to-br from-slate-700 to-slate-800 rounded-2xl p-5 text-white shadow-lg">
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2.5 bg-white/20 rounded-xl">
                <svg className="w-5 h-5 text-rose-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <div>
                <p className="text-slate-300 text-sm">本期败家子</p>
                <h3 className="text-xl font-bold mt-1">{bigSpender.roleName}</h3>
                <p className="text-slate-400 text-xs mt-0.5">{bigSpender.server}</p>
              </div>
            </div>
            <div className="text-right">
              <p className="text-slate-400 text-xs">总支出</p>
              <p className="text-2xl font-bold text-rose-300 mt-0.5">{bigSpender.totalExpense.toLocaleString()}</p>
            </div>
          </div>
          <div className="flex items-center gap-1.5 mt-3 pt-2.5 border-t border-white/20">
            <svg className="w-3.5 h-3.5 text-rose-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2zm7-5a2 2 0 11-4 0 2 2 0 014 0z" />
            </svg>
            <span className="text-xs text-slate-300">支出最高的角色（仅本人账号）</span>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div
          onClick={onShowIncomeDetail}
          className="bg-surface rounded-xl shadow-sm border border-base p-4 cursor-pointer transition-all hover:shadow-md"
        >
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-medium text-muted">收益概览</span>
            {statsPeriod === 'week' ? (
              <span className="text-xs text-emerald-600 bg-emerald-50 px-2 py-0.5 rounded-full">本周</span>
            ) : (
              <span className="text-xs text-amber-600 bg-amber-50 px-2 py-0.5 rounded-full">本月</span>
            )}
          </div>
          <div className="flex items-center gap-6">
            <div className="flex-1">
              <div className="flex items-center gap-2 mb-1.5">
                <div className="p-1.5 bg-amber-50 rounded-lg">
                  <Coins className="w-5 h-5 text-amber-500" />
                </div>
                <span className="text-xs text-slate-400">总收入</span>
              </div>
              <p className="text-2xl font-bold text-main">
                {stats.totalGold.toLocaleString()}
                <span className="text-sm font-normal text-muted ml-1">金</span>
              </p>
              <p className="text-xs text-muted mt-1">代清: {stats.clientIncome.toLocaleString()}金</p>
            </div>
            <div className="w-px h-12 bg-slate-100" />
            <div className="flex-1">
              <div className="flex items-center gap-1.5 mb-1.5">
                <div className="p-1 bg-emerald-50 rounded-lg">
                  <Shield className="w-4 h-4 text-emerald-500" />
                </div>
                <span className="text-xs text-muted">通关</span>
              </div>
              <p className="text-xl font-bold text-main">{stats.totalRaids}</p>
            </div>
          </div>
          <div className="mt-3 pt-3 border-t border-slate-100 flex items-center justify-between">
            <span className="text-xs text-slate-400">点击查看详细记录</span>
            <FileText className="w-4 h-4 text-slate-300" />
          </div>
        </div>
        <div
          onClick={onShowCrystalDetail}
          className="bg-surface rounded-xl shadow-sm border border-base p-4 cursor-pointer transition-all hover:shadow-md"
        >
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-medium text-muted">玄晶统计</span>
            <div className="flex items-center gap-1 px-2 py-0.5 bg-amber-50 text-amber-600 rounded-full">
              <Trophy className="w-3 h-3" />
              <span className="text-xs font-medium">海景房</span>
            </div>
          </div>
          <div className="flex items-center gap-6">
            <div className="flex-1">
              <div className="flex items-center gap-2 mb-1.5">
                <div className="p-1.5 bg-amber-50 rounded-lg">
                  <Trophy className="w-5 h-5 text-amber-500" />
                </div>
                <span className="text-xs text-muted">获取次数</span>
              </div>
              <p className="text-2xl font-bold text-main">
                {stats.xuanjingCount}
                <span className="text-sm font-normal text-muted ml-1">次</span>
              </p>
              <p className="text-xs text-muted mt-1">掉率 {stats.dropRate.toFixed(2)}%</p>
            </div>
            <div className="w-px h-12 bg-slate-100" />
            <div className="flex-1">
              <div className="flex items-center gap-1.5 mb-1.5">
                <div className="p-1 bg-emerald-50 rounded-lg">
                  <TrendingUp className="w-4 h-4 text-emerald-500" />
                </div>
                <span className="text-xs text-muted">掉率</span>
              </div>
              <p className="text-xl font-bold text-main">
                {stats.dropRate.toFixed(2)}
                <span className="text-sm font-normal text-muted ml-0.5">%</span>
              </p>
            </div>
          </div>
          <div className="mt-3 pt-3 border-t border-slate-100 flex items-center justify-between">
            <span className="text-xs text-slate-400">点击查看所有玄晶记录</span>
            <ArrowRight className="w-4 h-4 text-amber-400" />
          </div>
        </div>
      </div>

      <div className="bg-surface rounded-xl shadow-sm border border-base p-5">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-main">副本收益分布</h3>
          <span className="text-sm text-slate-400">{statsPeriod === 'week' ? '本周' : '本月'}数据</span>
        </div>
        <div className="h-56">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={chartData} margin={{ top: 10, right: 20, left: -10, bottom: 10 }}>
              <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="#f1f5f9" />
              <XAxis
                dataKey="name"
                fontSize={11}
                tickLine={false}
                axisLine={false}
                tick={{ fill: '#94a3b8' }}
                interval={0}
                angle={-20}
                textAnchor="end"
                height={50}
              />
              <YAxis
                fontSize={11}
                tickLine={false}
                axisLine={false}
                tick={{ fill: '#94a3b8' }}
                tickFormatter={(val) => val >= 10000 ? `${(val / 10000).toFixed(1)}w` : val}
              />
              <Tooltip
                cursor={{ fill: 'var(--bg-base)' }}
                contentStyle={{ borderRadius: '8px', border: 'none', boxShadow: '0 4px 6px -1px rgb(0 0 0 / 0.1)' }}
                formatter={(value: number) => [`${value.toLocaleString()} 金`, '']}
              />
              <Bar dataKey="value" radius={[4, 4, 0, 0]} maxBarSize={60}>
                {chartData.map((_entry, index) => (
                  <Cell key={`cell-${index}`} fill={['#8b5cf6', '#a855f7', '#c084fc', '#d8b4fe', '#e9d5ff'][index % 5]} />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div >
  );
};