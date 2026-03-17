import React, { useMemo, useState } from 'react';

import { RaidRecord, Account, AccountType, DashboardStats, BaizhanRecord, TrialPlaceRecord } from '../types';
import { ArrowRight, Star, Zap } from 'lucide-react';
import { db } from '../services/db';
import { getLastMonday } from '../utils/cooldownManager';
import { getBaseServerName } from '../utils/serverUtils';
import { calculateTrialFlipStats } from '../utils/trialFlipStats';

interface DashboardProps {
  records: RaidRecord[];
  accounts: Account[];
  baizhanRecords: BaizhanRecord[];
  trialRecords: TrialPlaceRecord[];
  statsPeriod: 'week' | 'month' | 'all';
  onStatsPeriodChange: (period: 'week' | 'month' | 'all') => void;
  onShowIncomeDetail: () => void;
  onShowCrystalDetail: () => void;
  onShowTrialFlipDetail: () => void;
}

export const Dashboard: React.FC<DashboardProps> = ({
  records,
  accounts,
  baizhanRecords,
  trialRecords,
  statsPeriod,
  onStatsPeriodChange,
  onShowIncomeDetail,
  onShowCrystalDetail,
  onShowTrialFlipDetail
}) => {
  const [equipments, setEquipments] = useState<any[]>([]);

  React.useEffect(() => {
    db.getEquipments().then((data: any[]) => {
      setEquipments(data.map(d => typeof d === 'string' ? JSON.parse(d) : d));
    }).catch(console.error);
  }, []);

  const findEquipmentById = React.useCallback((id: string | undefined) => {
    if (!id || !id.trim()) return null;
    return equipments.find((e: any) => e.ID?.toString() === id) || null;
  }, [equipments]);

  const safeRecords = Array.isArray(records) ? records : [];
  const safeAccounts = Array.isArray(accounts) ? accounts : [];

  const getPeriodStartTime = React.useCallback((period: 'week' | 'month' | 'all') => {
    if (period === 'all') return null;

    const now = new Date();
    if (period === 'week') {
      return getLastMonday(now).getTime();
    }

    return new Date(now.getFullYear(), now.getMonth(), 1).getTime();
  }, []);

  const periodStartTime = getPeriodStartTime(statsPeriod);
  const periodLabel = statsPeriod === 'week' ? '本周' : statsPeriod === 'month' ? '本月' : '全部';

  const filteredRecords = useMemo(() => {
    return safeRecords.filter(r => {
      const recordTime = typeof r.date === 'number' ? r.date : new Date(r.date).getTime();
      return periodStartTime === null ? true : recordTime >= periodStartTime;
    });
  }, [safeRecords, periodStartTime]);

  const safeBaizhanRecords = Array.isArray(baizhanRecords) ? baizhanRecords : [];

  const filteredBaizhanRecords = useMemo(() => {
    return safeBaizhanRecords.filter(r => {
      const recordTime = typeof r.date === 'number' ? r.date : new Date(r.date).getTime();
      return periodStartTime === null ? true : recordTime >= periodStartTime;
    });
  }, [safeBaizhanRecords, periodStartTime]);

  const safeTrialRecords = Array.isArray(trialRecords) ? trialRecords : [];
  const totalTrialFlipStats = useMemo(() => calculateTrialFlipStats(safeTrialRecords), [safeTrialRecords]);
  const totalTradableEquipCount = useMemo(() => {
    let count = 0;
    safeTrialRecords.forEach(record => {
      const equipId = (record as any)[`card${record.flippedIndex}`];
      if (!equipId) {
        return;
      }
      const equip = findEquipmentById(equipId);
      if (equip && (equip.BindType === 1 || equip.BindType === 2)) {
        count += 1;
      }
    });
    return count;
  }, [safeTrialRecords, findEquipmentById]);

  const filteredTrialRecords = useMemo(() => {
    return safeTrialRecords.filter(r => {
      const recordTime = typeof r.date === 'number' ? r.date : new Date(r.date).getTime();
      return periodStartTime === null ? true : recordTime >= periodStartTime;
    });
  }, [safeTrialRecords, periodStartTime]);

  // 稀有掉落分类统计（基于筛选后的记录）
  const rareDropStats = useMemo(() => {
    const drops = {
      xuanjing: 0,   // 玄晶
      maju: 0,       // 马具
      pet: 0,        // 宠物
      pendant: 0,    // 挂件
      mount: 0,      // 坐骑
      appearance: 0, // 外观
      title: 0,      // 称号
      secretBook: 0, // 秘籍
    };
    filteredRecords.forEach(r => {
      if (r.hasXuanjing) drops.xuanjing++;
      if (r.hasMaJu) drops.maju++;
      if (r.hasPet) drops.pet++;
      if (r.hasPendant) drops.pendant++;
      if (r.hasMount) drops.mount++;
      if (r.hasAppearance) drops.appearance++;
      if (r.hasTitle) drops.title++;
      if (r.hasSecretBook) drops.secretBook++;
    });
    const total = drops.xuanjing + drops.maju + drops.pet + drops.pendant + drops.mount + drops.appearance + drops.title + drops.secretBook;
    return { ...drops, total };
  }, [filteredRecords]);

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

  const incomeBreakdown = useMemo(() => {
    const raidGold = filteredRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    const baizhanGold = filteredBaizhanRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    return {
      raidGold,
      baizhanGold,
      raidCount: filteredRecords.length,
      baizhanCount: filteredBaizhanRecords.length
    };
  }, [filteredRecords, filteredBaizhanRecords]);



  const luckyRole = useMemo(() => {
    const roleMap = new Map<string, { roleName: string; server: string; totalGold: number; xuanjingCount: number }>();

    filteredRecords.forEach(r => {
      const roleId = r.roleId || r.accountId;
      const roleName = r.roleName || '未知角色';
      const server = getBaseServerName(r.server || '未知服务器');

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
      const server = getBaseServerName(r.server || '未知服务器');

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
          const server = getBaseServerName(r.server || '未知服务器');

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
          const server = getBaseServerName(r.server || '未知服务器');

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
            onClick={() => onStatsPeriodChange('week')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${statsPeriod === 'week'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            本周
          </button>
          <button
            onClick={() => onStatsPeriodChange('month')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${statsPeriod === 'month'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            本月
          </button>
          <button
            onClick={() => onStatsPeriodChange('all')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${statsPeriod === 'all'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            全部
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
                <h3 className="text-xl font-bold mt-1 text-main">
                  {luckyRole.roleName}
                  {luckyRole.server && (
                    <span className="text-sm font-normal text-muted ml-1.5">· {luckyRole.server}</span>
                  )}
                </h3>
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
                <h3 className="text-xl font-bold mt-1 text-main">
                  {bigSpender.roleName}
                  {bigSpender.server && (
                    <span className="text-sm font-normal text-muted ml-1.5">· {bigSpender.server}</span>
                  )}
                </h3>
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

      {/* 第一行：收益概览（全宽） */}
      <div
        onClick={onShowIncomeDetail}
        className="cursor-pointer rounded-xl border border-base bg-surface p-4 shadow-sm transition-colors hover:border-emerald-300 dark:hover:border-emerald-700"
      >
        <div className="flex items-center justify-between mb-3">
          <span className="text-sm font-bold text-main">收益概览</span>
          <div className="flex items-center gap-1 rounded-full border border-slate-200 bg-slate-100 px-2 py-1 text-slate-600 dark:border-slate-700/50 dark:bg-slate-800 dark:text-slate-300">
            <span className="text-xs">{periodLabel}</span>
            <ArrowRight className="w-3 h-3 text-muted" />
          </div>
        </div>

        <div className="grid grid-cols-2 lg:grid-cols-4 gap-2">
          {/* 总收入 */}
          <div className="rounded-lg border border-emerald-200/60 bg-emerald-50/60 p-3 dark:border-emerald-800/30 dark:bg-emerald-900/10">
            <div className="text-xs font-medium text-emerald-700 dark:text-emerald-300">本期总收入</div>
            <div className="mt-1.5 text-2xl font-bold text-emerald-700 dark:text-emerald-300">
              {stats.totalGold.toLocaleString()}
              <span className="ml-1 text-xs font-normal text-emerald-600/70 dark:text-emerald-400/70">金</span>
            </div>
          </div>

          {/* 通关次数 */}
          <div className="rounded-lg border border-base bg-slate-50/80 p-3 dark:bg-slate-800/30">
            <div className="text-xs font-medium text-muted">通关次数</div>
            <div className="mt-1.5 text-2xl font-bold text-main">
              {stats.totalRaids}
              <span className="ml-1 text-xs font-normal text-muted">次</span>
            </div>
          </div>

          {/* 团队副本 / 百战收益明细 */}
          <div className="rounded-lg border border-base bg-slate-50/80 p-3 dark:bg-slate-800/30">
            <div className="text-xs font-medium text-muted">团队副本</div>
            <div className="mt-1.5 text-lg font-bold text-main">{incomeBreakdown.raidGold.toLocaleString()}<span className="ml-1 text-xs font-normal text-muted">金</span></div>
            <div className="mt-1 text-xs text-muted">{incomeBreakdown.raidCount} 次通关</div>
          </div>

          <div className="rounded-lg border border-base bg-slate-50/80 p-3 dark:bg-slate-800/30">
            <div className="text-xs font-medium text-muted">百战异闻录</div>
            <div className="mt-1.5 text-lg font-bold text-main">{incomeBreakdown.baizhanGold.toLocaleString()}<span className="ml-1 text-xs font-normal text-muted">金</span></div>
            <div className="mt-1 text-xs text-muted">{incomeBreakdown.baizhanCount} 次通关</div>
          </div>
        </div>
      </div>

      {/* 第二行：稀有掉落 + 试炼翻牌（并排等宽） */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {/* 稀有掉落统计 */}
        <div
          onClick={onShowCrystalDetail}
          className="flex flex-col justify-between cursor-pointer rounded-xl border border-base bg-surface p-4 shadow-sm transition-colors hover:border-violet-300 dark:hover:border-violet-700"
        >
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-bold text-main">稀有掉落统计</span>
            <div className="flex items-center gap-1 rounded-full border border-slate-200 bg-slate-100 px-2 py-1 text-slate-600 dark:border-slate-700/50 dark:bg-slate-800 dark:text-slate-300">
              <span className="text-xs">{periodLabel}</span>
              <ArrowRight className="w-3 h-3 text-muted" />
            </div>
          </div>

          {/* 主指标行 */}
          <div className="grid grid-cols-3 gap-2">
            <div className="rounded-lg border border-violet-200/60 bg-violet-50/60 px-2.5 py-2.5 dark:border-violet-800/30 dark:bg-violet-900/10">
              <div className="text-[10px] text-violet-700 dark:text-violet-300">掉落总数</div>
              <div className="mt-1.5 text-lg font-bold text-violet-700 dark:text-violet-300">{rareDropStats.total}</div>
            </div>
            <div className="rounded-lg border border-base bg-slate-50/80 px-2.5 py-2.5 dark:bg-slate-800/30">
              <div className="text-[10px] text-muted">玄晶</div>
              <div className="mt-1.5 text-lg font-bold text-main">{rareDropStats.xuanjing}</div>
            </div>
            <div className="rounded-lg border border-base bg-slate-50/80 px-2.5 py-2.5 dark:bg-slate-800/30">
              <div className="text-[10px] text-muted">玄晶掉率</div>
              <div className="mt-1.5 text-lg font-bold text-main">{stats.dropRate.toFixed(1)}%</div>
            </div>
          </div>

          {/* 其他稀有掉落标签 */}
          <div className="mt-3 flex flex-wrap gap-1.5">
            {[
              { label: '马具', count: rareDropStats.maju },
              { label: '宠物', count: rareDropStats.pet },
              { label: '挂件', count: rareDropStats.pendant },
              { label: '坐骑', count: rareDropStats.mount },
              { label: '外观', count: rareDropStats.appearance },
              { label: '称号', count: rareDropStats.title },
              { label: '秘籍', count: rareDropStats.secretBook },
            ].map(item => (
              <span
                key={item.label}
                className={`inline-flex items-center gap-1 rounded-md px-2 py-1 text-[11px] border ${
                  item.count > 0
                    ? 'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-900/20 dark:text-emerald-300 dark:border-emerald-800/30'
                    : 'bg-slate-50 text-muted border-base dark:bg-slate-800/30'
                }`}
              >
                {item.label}
                <span className="font-semibold">{item.count}</span>
              </span>
            ))}
          </div>
        </div>

        {/* 试炼之地翻牌统计 */}
        <div
          onClick={onShowTrialFlipDetail}
          className="flex flex-col justify-between cursor-pointer rounded-xl border border-base bg-surface p-4 shadow-sm transition-colors hover:border-emerald-300 dark:hover:border-emerald-700"
        >
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-bold text-main">试炼之地翻牌</span>
            <div className="flex items-center gap-1 rounded-full border border-slate-200 bg-slate-100 px-2 py-1 text-slate-600 dark:border-slate-700/50 dark:bg-slate-800 dark:text-slate-300">
              <span className="text-xs">全部历史</span>
              <ArrowRight className="w-3 h-3 text-muted" />
            </div>
          </div>

          {/* 主指标行 */}
          <div className="grid grid-cols-2 gap-2">
            <div className="rounded-lg border border-base bg-slate-50/80 px-2.5 py-2.5 dark:bg-slate-800/30">
              <div className="text-[10px] text-muted">总记录数</div>
              <div className="mt-1.5 text-lg font-bold text-main">{totalTrialFlipStats.totalRecords}</div>
            </div>
            <div className="rounded-lg border border-emerald-200/60 bg-emerald-50/60 px-2.5 py-2.5 dark:border-emerald-800/30 dark:bg-emerald-900/10">
              <div className="text-[10px] text-emerald-700 dark:text-emerald-300">可交易装备</div>
              <div className="mt-1.5 text-lg font-bold text-emerald-700 dark:text-emerald-300">{totalTradableEquipCount}</div>
            </div>
          </div>

          {/* 翻牌分析 */}
          <div className="mt-3 grid grid-cols-2 gap-2">
            <div className="rounded-lg border border-base bg-slate-50/80 px-2.5 py-2.5 dark:bg-slate-800/30">
              <div className="text-[10px] text-muted">装备率最高位</div>
              <div className="mt-1 text-sm font-semibold text-main">
                {totalTrialFlipStats.bestFlipPosition
                  ? `${totalTrialFlipStats.bestFlipPosition.position}号位`
                  : '-'}
              </div>
              <div className="mt-0.5 text-[10px] text-emerald-600 dark:text-emerald-400">
                {totalTrialFlipStats.bestFlipPosition
                  ? `${(totalTrialFlipStats.bestFlipPosition.flipEquipmentRate * 100).toFixed(1)}%`
                  : '0%'}
              </div>
            </div>
            <div className="rounded-lg border border-base bg-slate-50/80 px-2.5 py-2.5 dark:bg-slate-800/30">
              <div className="text-[10px] text-muted">出现率最高位</div>
              <div className="mt-1 text-sm font-semibold text-main">
                {totalTrialFlipStats.bestAppearancePosition
                  ? `${totalTrialFlipStats.bestAppearancePosition.position}号位`
                  : '-'}
              </div>
              <div className="mt-0.5 text-[10px] text-muted">
                {totalTrialFlipStats.bestAppearancePosition
                  ? `${(totalTrialFlipStats.bestAppearancePosition.appearanceRate * 100).toFixed(1)}%`
                  : '0%'}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

