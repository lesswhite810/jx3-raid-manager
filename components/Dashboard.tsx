import React, { useMemo, useState } from 'react';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Cell } from 'recharts';
import { RaidRecord, Account, AccountType, DashboardStats, BaizhanRecord, TrialPlaceRecord } from '../types';
import { ArrowRight, Star, Zap } from 'lucide-react';
import { db } from '../services/db';

interface DashboardProps {
  records: RaidRecord[];
  accounts: Account[];
  baizhanRecords: BaizhanRecord[];
  trialRecords: TrialPlaceRecord[];
  onShowIncomeDetail: () => void;
  onShowCrystalDetail: () => void;
}

export const Dashboard: React.FC<DashboardProps> = ({ records, accounts, baizhanRecords, trialRecords, onShowIncomeDetail, onShowCrystalDetail }) => {
  const [statsPeriod, setStatsPeriod] = useState<'week' | 'month'>('week');
  const [equipments, setEquipments] = useState<any[]>([]);

  React.useEffect(() => {
    db.getEquipments().then((data: any[]) => {
      setEquipments(data.map(d => typeof d === 'string' ? JSON.parse(d) : d));
    }).catch(console.error);
  }, []);

  const findEquipmentById = React.useCallback((id: string | undefined) => {
    if (!id || !id.trim()) return null;
    return equipments.find(e => e.ID?.toString() === id) || null;
  }, [equipments]);

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

  const safeBaizhanRecords = Array.isArray(baizhanRecords) ? baizhanRecords : [];

  const filteredBaizhanRecords = useMemo(() => {
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

    return safeBaizhanRecords.filter(r => new Date(r.date) >= startOfPeriod);
  }, [safeBaizhanRecords, statsPeriod]);

  const safeTrialRecords = Array.isArray(trialRecords) ? trialRecords : [];

  const filteredTrialRecords = useMemo(() => {
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

    return safeTrialRecords.filter(r => new Date(r.date) >= startOfPeriod);
  }, [safeTrialRecords, statsPeriod]);

  const stats: DashboardStats = useMemo(() => {
    const totalRaidGold = filteredRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    const totalBaizhanGold = filteredBaizhanRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    const totalGold = totalRaidGold + totalBaizhanGold;

    const xuanjingCount = filteredRecords.filter(r => r.hasXuanjing).length;
    const dropRate = filteredRecords.length > 0 ? (xuanjingCount / filteredRecords.length) * 100 : 0;

    const clientAccountIds = safeAccounts
      .filter(a => a.type === AccountType.CLIENT && !a.disabled)
      .map(a => a.id);
    const clientRaidIncome = filteredRecords
      .filter(r => clientAccountIds.includes(r.accountId))
      .reduce((acc, r) => acc + r.goldIncome, 0);
    const clientBaizhanIncome = filteredBaizhanRecords
      .filter(r => clientAccountIds.includes(r.accountId))
      .reduce((acc, r) => acc + r.goldIncome, 0);
    const clientIncome = clientRaidIncome + clientBaizhanIncome;

    let equipCount = 0;
    filteredTrialRecords.forEach((r) => {
      const equipId = (r as any)[`card${r.flippedIndex}`];
      if (equipId) {
        const equip = findEquipmentById(equipId);
        if (equip && (equip.BindType === 1 || equip.BindType === 2)) {
          equipCount++;
        }
      }
    });

    return {
      totalGold,
      totalRaids: filteredRecords.length + filteredBaizhanRecords.length,
      xuanjingCount,
      equipCount,
      dropRate,
      clientIncome,
    };
  }, [filteredRecords, filteredBaizhanRecords, filteredTrialRecords, safeAccounts, findEquipmentById]);

  const chartData = useMemo(() => {
    const grouped: Record<string, number> = {};
    filteredRecords.forEach(r => {
      grouped[r.raidName] = (grouped[r.raidName] || 0) + r.goldIncome;
    });
    filteredBaizhanRecords.forEach(r => {
      grouped['百战异闻录'] = (grouped['百战异闻录'] || 0) + r.goldIncome;
    });
    return Object.keys(grouped).map(k => ({ name: k, value: grouped[k] }));
  }, [filteredRecords, filteredBaizhanRecords, filteredTrialRecords]);

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

    filteredBaizhanRecords.forEach(r => {
      const roleId = r.roleId || r.accountId;
      const roleName = r.roleName || '未知角色';
      const server = r.server || '未知服务器';

      if (!roleMap.has(roleId)) {
        roleMap.set(roleId, { roleName, server, totalGold: 0, xuanjingCount: 0 });
      }

      const role = roleMap.get(roleId)!;
      role.totalGold += r.goldIncome;
    });

    let maxRole: typeof roleMap extends Map<string, infer V> ? V : never = { roleName: '暂无数据', server: '', totalGold: 0, xuanjingCount: 0 };
    roleMap.forEach(role => {
      if (role.totalGold > maxRole.totalGold) {
        maxRole = role;
      }
    });

    return maxRole;
  }, [filteredRecords, filteredBaizhanRecords]);

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

    filteredBaizhanRecords
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
  }, [filteredRecords, filteredBaizhanRecords, safeAccounts]);

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-main">数据概览</h2>
        <div className="flex items-center gap-1 bg-base rounded-lg p-1 border border-base">
          <button
            onClick={() => setStatsPeriod('week')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${statsPeriod === 'week'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            本周
          </button>
          <button
            onClick={() => setStatsPeriod('month')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${statsPeriod === 'month'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            本月
          </button>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        {/* Lucky Role Card */}
        <div className="bg-surface rounded-xl p-5 border border-base shadow-sm">
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2.5 bg-slate-100 dark:bg-slate-800/50 rounded-xl border border-slate-200/50 dark:border-slate-700/50">
                <Star className="w-5 h-5 text-primary/80" />
              </div>
              <div>
                <p className="text-muted text-sm">本期欧皇</p>
                <h3 className="text-xl font-bold mt-1 text-main">{luckyRole.roleName}</h3>
                <p className="text-muted text-xs mt-0.5">{luckyRole.server}</p>
              </div>
            </div>
            <div className="flex items-center gap-6">
              <div className="text-right">
                <p className="text-muted text-xs">金币收益</p>
                <p className="text-2xl font-bold text-emerald-600 mt-0.5">{luckyRole.totalGold.toLocaleString()}</p>
              </div>
              {luckyRole.xuanjingCount > 0 && (
                <div className="text-right">
                  <p className="text-muted text-xs">玄晶</p>
                  <p className="text-2xl font-bold text-violet-600 dark:text-violet-400 mt-0.5">{luckyRole.xuanjingCount}</p>
                </div>
              )}
            </div>
          </div>
          <div className="flex items-center gap-1.5 mt-3 pt-2.5 border-t border-base">
            <Zap className="w-3.5 h-3.5 text-primary/70" />
            <span className="text-xs text-muted">收入最高的角色</span>
          </div>
        </div>

        {/* Big Spender Card */}
        <div className="bg-surface rounded-xl p-5 border border-base shadow-sm">
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2.5 bg-slate-100 dark:bg-slate-800/50 rounded-xl border border-slate-200/50 dark:border-slate-700/50">
                <svg className="w-5 h-5 text-rose-500/80" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <div>
                <p className="text-muted text-sm">本期败家子</p>
                <h3 className="text-xl font-bold mt-1 text-main">{bigSpender.roleName}</h3>
                <p className="text-muted text-xs mt-0.5">{bigSpender.server}</p>
              </div>
            </div>
            <div className="text-right">
              <p className="text-muted text-xs">总支出</p>
              <p className="text-2xl font-bold text-amber-600 mt-0.5">{bigSpender.totalExpense.toLocaleString()}</p>
            </div>
          </div>
          <div className="flex items-center gap-1.5 mt-3 pt-2.5 border-t border-base">
            <svg className="w-3.5 h-3.5 text-rose-500/70" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2zm7-5a2 2 0 11-4 0 2 2 0 014 0z" />
            </svg>
            <span className="text-xs text-muted">支出最高的角色（仅本人账号）</span>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div
          onClick={onShowIncomeDetail}
          className="bg-surface rounded-xl shadow-sm border border-base p-4 cursor-pointer transition-all hover:shadow-md"
        >
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-bold text-main">收益概览</span>
            <div className="flex items-center gap-1 px-2.5 py-1 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 rounded-full border border-slate-200 dark:border-slate-700/50">
              <span className="text-xs">{statsPeriod === 'week' ? '本周' : '本月'}</span>
              <ArrowRight className="w-3 h-3 text-muted" />
            </div>
          </div>

          <div className="flex items-center gap-4">
            {/* 总收益区块 - 柔和琥珀色 */}
            <div className="flex-1 bg-gradient-to-br from-amber-50/50 to-orange-50/50 dark:from-amber-900/10 dark:to-orange-900/10 rounded-lg p-3 border border-amber-100/50 dark:border-amber-800/20">
              <div className="flex items-center gap-1.5 mb-2">
                <span className="text-xs font-medium text-amber-700 dark:text-amber-300">本期总收入</span>
              </div>
              <p className="text-2xl font-bold text-amber-900 dark:text-amber-100">
                {stats.totalGold.toLocaleString()}
                <span className="text-xs font-normal text-amber-600/70 dark:text-amber-400/70 ml-1">金</span>
              </p>
              <div className="mt-1 flex items-center justify-between">
                <span className="text-[10px] text-amber-600/60 dark:text-amber-400/60">代清所得</span>
                <span className="text-[10px] font-medium text-amber-700/80 dark:text-amber-300/80">{stats.clientIncome.toLocaleString()}</span>
              </div>
            </div>

            {/* 通关统计区块 - 柔和靛蓝色 */}
            <div className="flex-1 bg-gradient-to-br from-indigo-50/50 to-blue-50/50 dark:from-indigo-900/10 dark:to-blue-900/10 rounded-lg p-3 border border-indigo-100/50 dark:border-indigo-800/20">
              <div className="flex items-center gap-1.5 mb-2">
                <span className="text-xs font-medium text-indigo-700 dark:text-indigo-300">通关统计</span>
              </div>
              <p className="text-2xl font-bold text-indigo-900 dark:text-indigo-100">
                {stats.totalRaids}
                <span className="text-xs font-normal text-indigo-600/70 dark:text-indigo-400/70 ml-1">次</span>
              </p>
              <div className="mt-1 flex items-center justify-between">
                <span className="text-[10px] text-indigo-600/60 dark:text-indigo-400/60">活跃度</span>
                <span className="text-[10px] font-medium text-indigo-700/80 dark:text-indigo-300/80">
                  {stats.totalRaids >= 10 ? '活跃' : '普通'}
                </span>
              </div>
            </div>
          </div>
        </div>

        <div
          onClick={onShowCrystalDetail}
          className="bg-surface rounded-xl shadow-sm border border-base p-4 cursor-pointer transition-all hover:shadow-md"
        >
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-bold text-main">稀有掉落统计</span>
            <div className="flex items-center gap-1 px-2.5 py-1 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 rounded-full border border-slate-200 dark:border-slate-700/50">
              <span className="text-xs">详情</span>
              <ArrowRight className="w-3 h-3 text-muted" />
            </div>
          </div>

          <div className="flex items-center gap-4">
            {/* 玄晶掉落区块 - 柔和紫金色 */}
            <div className="flex-1 bg-gradient-to-br from-violet-50/50 to-amber-50/50 dark:from-violet-900/10 dark:to-amber-900/10 rounded-lg p-3 border border-violet-100/50 dark:border-violet-800/20">
              <div className="flex items-center gap-1.5 mb-2">
                <span className="text-xs font-medium text-violet-700 dark:text-violet-300">玄晶获取</span>
              </div>
              <p className="text-2xl font-bold text-violet-900 dark:text-violet-100">
                {stats.xuanjingCount}
                <span className="text-xs font-normal text-violet-600/70 dark:text-violet-400/70 ml-1">次</span>
              </p>
              <div className="mt-1 flex items-center justify-between">
                <span className="text-[10px] text-violet-600/60 dark:text-violet-400/60">掉率</span>
                <span className="text-[10px] font-medium text-violet-700/80 dark:text-violet-300/80">{stats.dropRate.toFixed(2)}%</span>
              </div>
            </div>

            {/* 装备掉落区块 - 柔和翠绿色 */}
            <div className="flex-1 bg-gradient-to-br from-emerald-50/50 to-teal-50/50 dark:from-emerald-900/10 dark:to-teal-900/10 rounded-lg p-3 border border-emerald-100/50 dark:border-emerald-800/20">
              <div className="flex items-center gap-1.5 mb-2">
                <span className="text-xs font-medium text-emerald-700 dark:text-emerald-300">可交易装备</span>
              </div>
              <p className="text-2xl font-bold text-emerald-900 dark:text-emerald-100">
                {stats.equipCount}
                <span className="text-xs font-normal text-emerald-600/70 dark:text-emerald-400/70 ml-1">件</span>
              </p>
              <div className="mt-1 flex items-center justify-between">
                <span className="text-[10px] text-emerald-600/60 dark:text-emerald-400/60">总计</span>
                <span className="text-[10px] font-medium text-emerald-700/80 dark:text-emerald-300/80">试炼之地</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="bg-surface rounded-xl shadow-sm border border-base p-5">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-main">副本收益分布</h3>
          <span className="text-sm text-muted">{statsPeriod === 'week' ? '本周' : '本月'}数据</span>
        </div>
        <div className="h-56">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={chartData} margin={{ top: 10, right: 20, left: -10, bottom: 10 }}>
              <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="rgb(var(--border-base))" />
              <XAxis
                dataKey="name"
                fontSize={11}
                tickLine={false}
                axisLine={false}
                tick={{ fill: 'rgb(var(--text-muted))' }}
                interval={0}
                angle={-20}
                textAnchor="end"
                height={50}
              />
              <YAxis
                fontSize={11}
                tickLine={false}
                axisLine={false}
                tick={{ fill: 'rgb(var(--text-muted))' }}
                tickFormatter={(val) => val >= 10000 ? `${(val / 10000).toFixed(1)}w` : val}
              />
              <Tooltip
                cursor={{ fill: 'rgb(var(--text-muted))', opacity: 0.1 }}
                contentStyle={{
                  backgroundColor: 'rgb(var(--bg-surface))',
                  borderColor: 'rgb(var(--border-base))',
                  borderRadius: '0.5rem',
                  boxShadow: '0 4px 6px -1px rgb(0 0 0 / 0.1)',
                  color: 'rgb(var(--text-main))'
                }}
                itemStyle={{ color: 'rgb(var(--text-main))' }}
                formatter={(value: number) => [`${value.toLocaleString()} 金`, '']}
              />
              <Bar dataKey="value" radius={[4, 4, 0, 0]} maxBarSize={60}>
                {chartData.map((_entry, index) => (
                  <Cell key={`cell-${index}`} fill={`rgb(var(--primary-base) / ${0.7 - index * 0.05})`} />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div>
  );
};