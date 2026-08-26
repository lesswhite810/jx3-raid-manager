import React, { useEffect, useMemo, useState } from 'react';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Cell, Legend, LabelList } from 'recharts';
import { ArrowLeft, Coins, TrendingUp, TrendingDown, Search, Calendar, Trash2, Pencil, Sparkles, Ghost, Package, Flag, Shirt, Crown, Anchor, ChevronDown, BookOpen, Boxes } from 'lucide-react';
import { RaidRecord, Account, BaizhanRecord, Season } from '../types';
import { toast } from '../utils/toastManager';
import { getLastMonday, getSeasonStartTimeMs } from '../utils/cooldownManager';
import { buildClientAccountIdSet, buildRoleInfoLookup, getRoleInfoKey } from '../utils/recordLookupUtils';
import { getRecordScrapsExpense, getRecordScrapsValue, summarizeScrapsByName } from '../utils/scrapsUtils';
import { db } from '../services/db';

/** 收益记录单次渲染条数上限，超出部分通过底部按钮分页加载，避免超长列表一次性渲染造成卡顿 */
const PAGE_SIZE = 100;

interface IncomeDetailProps {
  records: RaidRecord[];
  baizhanRecords: BaizhanRecord[];
  accounts: Account[];
  initialPeriod: 'week' | 'season' | 'all';
  onPeriodChange: (period: 'week' | 'season' | 'all') => void;
  onBack: () => void;
  onDeleteRecord: (recordId: string, isBaizhan?: boolean, isTrial?: boolean) => void;
  onEditRecord: (record: RaidRecord) => void;
  onEditBaizhanRecord: (record: BaizhanRecord) => void;
}

interface EnhancedRecord {
  id: string;
  accountId: string;
  roleId: string;
  roleName?: string;
  server?: string;
  date: string | number;
  raidName: string;
  goldIncome: number;
  goldExpense?: number;
  notes?: string;
  hasXuanjing?: boolean;
  hasMaJu?: boolean;
  hasPet?: boolean;
  hasPendant?: boolean;
  hasMount?: boolean;
  hasAppearance?: boolean;
  hasTitle?: boolean;
  hasSecretBook?: boolean;
  displayRoleName: string;
  displayServer: string;
  isBaizhan?: boolean;
  isTrial?: boolean;
  source?: 'auto' | 'manual';
  status?: 'pending' | 'confirmed' | 'rejected' | 'scanning';
  // 散件老板字段
  scrapsItems?: RaidRecord['scrapsItems'];
  scrapsValue?: number;
  isScrapsBoss?: boolean;
}

export const IncomeDetail: React.FC<IncomeDetailProps> = ({ records, baizhanRecords, accounts, initialPeriod, onPeriodChange, onBack, onDeleteRecord, onEditRecord, onEditBaizhanRecord }) => {
  const [period, setPeriod] = useState<'week' | 'season' | 'all'>(initialPeriod);
  const [searchTerm, setSearchTerm] = useState('');
  const [activeTab, setActiveTab] = useState<'all' | 'income' | 'expense'>('all');
  const [deleteConfirmRecordId, setDeleteConfirmRecordId] = useState<string | null>(null);
  const [expandedRecordId, setExpandedRecordId] = useState<string | null>(null);
  const [currentSeason, setCurrentSeason] = useState<Season | null>(null);
  const [chartView, setChartView] = useState<'raid' | 'role'>('role');
  // 内容分区：收益汇总图表 / 散件白名单汇总 / 收益记录列表
  const [contentTab, setContentTab] = useState<'summary' | 'scraps' | 'records'>('summary');
  const [renderLimit, setRenderLimit] = useState(PAGE_SIZE);


  useEffect(() => {
    setPeriod(initialPeriod);
  }, [initialPeriod]);

  // 切换周期/搜索词/tab 时重置分页，从第一页重新展示
  useEffect(() => {
    setRenderLimit(PAGE_SIZE);
  }, [period, searchTerm, activeTab]);

  useEffect(() => {
    db.getCurrentSeason().then(setCurrentSeason).catch(() => {
      setCurrentSeason(null);
    });
  }, []);

  const handlePeriodChange = (nextPeriod: 'week' | 'season' | 'all') => {
    setPeriod(nextPeriod);
    onPeriodChange(nextPeriod);
  };

  const safeRecords = Array.isArray(records) ? records : [];
  const safeBaizhanRecords = Array.isArray(baizhanRecords) ? baizhanRecords : [];
  const safeAccounts = Array.isArray(accounts) ? accounts : [];
  const roleInfoLookup = useMemo(() => buildRoleInfoLookup(safeAccounts), [safeAccounts]);

  const enhancedRecords = useMemo<EnhancedRecord[]>(() => {
    const raidList: EnhancedRecord[] = safeRecords.map(record => {
      const roleInfo = roleInfoLookup.get(getRoleInfoKey(record.accountId, record.roleId));
      const roleName = record.roleName || roleInfo?.roleName || '未知角色';
      let rawServer = roleInfo?.server || record.server || '未知服务器';
      if (roleName && roleName !== '未知角色') {
        rawServer = rawServer.replace(new RegExp(`\\s*${roleName}\\s*`, 'g'), ' ').trim();
      }
      return {
        ...record,
        displayRoleName: roleName,
        displayServer: rawServer,
        isBaizhan: false
      };
    });

    const baizhanList: EnhancedRecord[] = safeBaizhanRecords.map(record => {
      const roleInfo = roleInfoLookup.get(getRoleInfoKey(record.accountId, record.roleId));
      const roleName = record.roleName || roleInfo?.roleName || '未知角色';
      let rawServer = roleInfo?.server || record.server || '未知服务器';
      if (roleName && roleName !== '未知角色') {
        rawServer = rawServer.replace(new RegExp(`\\s*${roleName}\\s*`, 'g'), ' ').trim();
      }
      return {
        ...record,
        raidName: '百战异闻录',
        displayRoleName: roleName,
        displayServer: rawServer,
        isBaizhan: true
      };
    });

    return [...raidList, ...baizhanList].sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
  }, [safeRecords, safeBaizhanRecords, roleInfoLookup]);

  const filteredRecords = useMemo(() => {
    const now = new Date();
    let startOfPeriod: Date;

    if (period === 'week') {
      startOfPeriod = getLastMonday(now);
    } else if (period === 'season') {
      // 赛季未配置时回退为不过滤（展示全部），避免出现"本赛季与本周相同"的假象
      const seasonStartMs = currentSeason ? getSeasonStartTimeMs(currentSeason.startDate) : null;
      startOfPeriod = new Date(seasonStartMs ?? 0);
    } else {
      startOfPeriod = new Date(now.getFullYear() - 10, 0, 1);
    }

    const startTime = startOfPeriod.getTime();
    return enhancedRecords.filter(r => {
      // 排除待确认的自动扫描记录，不计入收益统计
      // rejected 记录保留显示在"全部"页签中，但不参与统计
      if (r.source === 'auto' && r.status === 'pending') return false;
      const recordTime = typeof r.date === 'number' ? r.date : new Date(r.date).getTime();
      return recordTime >= startTime;
    });
  }, [enhancedRecords, period, currentSeason]);

  // 用于统计和图表的记录：排除 rejected 记录
  const confirmedRecords = useMemo(() => {
    return filteredRecords.filter(r => !(r.source === 'auto' && r.status === 'rejected'));
  }, [filteredRecords]);

  const searchedRecords = useMemo(() => {
    return filteredRecords.filter(r =>
      r.raidName.toLowerCase().includes(searchTerm.toLowerCase()) ||
      r.displayRoleName.toLowerCase().includes(searchTerm.toLowerCase())
    );
  }, [filteredRecords, searchTerm]);

  const tabFilteredRecords = useMemo(() => {
    if (activeTab === 'income') {
      return searchedRecords.filter(r => r.goldIncome > 0 && !(r.source === 'auto' && r.status === 'rejected'));
    } else if (activeTab === 'expense') {
      return searchedRecords.filter(r => (r.goldExpense || 0) > 0 && !(r.source === 'auto' && r.status === 'rejected'));
    }
    return searchedRecords;
  }, [searchedRecords, activeTab]);

  /** 当前实际渲染的记录（分页截断） */
  const shownRecords = useMemo(() => {
    return tabFilteredRecords.slice(0, renderLimit);
  }, [tabFilteredRecords, renderLimit]);

  const stats = useMemo(() => {
    const clientAccountIds = buildClientAccountIdSet(safeAccounts);
    const totalIncome = confirmedRecords.reduce((acc, r) => acc + r.goldIncome, 0);
    // 总支出仅统计本人账号,代清账号的支出由老板承担,不计入本人支出
    const totalExpense = confirmedRecords
      .filter(r => !clientAccountIds.has(r.accountId))
      .reduce((acc, r) => acc + (r.goldExpense || 0), 0);
    const netIncome = totalIncome - totalExpense;
    const xuanjingCount = confirmedRecords.filter(r => r.hasXuanjing).length;
    const clientIncome = confirmedRecords
      .filter(r => clientAccountIds.has(r.accountId))
      .reduce((acc, r) => acc + r.goldIncome, 0);

    const clientExpense = confirmedRecords
      .filter(r => clientAccountIds.has(r.accountId))
      .reduce((acc, r) => acc + (r.goldExpense || 0), 0);

    // 散件口径：仅统计 isScrapsBoss=true 的记录
    // 支出 = 白名单物品的实际购买花费合计（旧版记录回退为副本总支出，见 getRecordScrapsExpense）
    // 预估收益 = getRecordScrapsValue 之和
    // 净收入 = 预估收益 − 支出
    const scrapsRecords = confirmedRecords.filter(r => r.isScrapsBoss);
    const scrapsExpense = scrapsRecords.reduce((acc, r) => acc + getRecordScrapsExpense(r), 0);
    const scrapsEstimatedIncome = scrapsRecords.reduce(
      (acc, r) => acc + getRecordScrapsValue(r),
      0,
    );
    const scrapsNetIncome = scrapsEstimatedIncome - scrapsExpense;

    return {
      totalIncome,
      totalExpense,
      netIncome,
      xuanjingCount,
      clientIncome,
      clientExpense,
      clientNetIncome: clientIncome - clientExpense,
      scrapsExpense,
      scrapsEstimatedIncome,
      scrapsNetIncome,
      scrapsRecordCount: scrapsRecords.length,
    };
  }, [confirmedRecords, safeAccounts]);

  // 散件白名单汇总（仅 isScrapsBoss=true 的记录）
  const scrapsSummary = useMemo(
    () => summarizeScrapsByName(confirmedRecords),
    [confirmedRecords],
  );

  // 副本收益分布图表数据
  const chartData = useMemo(() => {
    const grouped: Record<string, number> = {};
    confirmedRecords.forEach(r => {
      grouped[r.raidName] = (grouped[r.raidName] || 0) + r.goldIncome;
    });
    return Object.keys(grouped)
      .map(k => ({ name: k, value: grouped[k] }))
      .sort((a, b) => b.value - a.value);
  }, [confirmedRecords]);

  // 角色收支图表数据
  const roleChartData = useMemo(() => {
    const grouped: Record<string, { income: number; expense: number; name: string }> = {};
    confirmedRecords.forEach(r => {
      const key = r.roleId;
      if (!grouped[key]) {
        grouped[key] = { income: 0, expense: 0, name: r.displayRoleName };
      }
      grouped[key].income += r.goldIncome;
      grouped[key].expense += (r.goldExpense || 0);
    });
    return Object.values(grouped)
      .map(d => ({ name: d.name, 收入: d.income, 支出: d.expense, netIncome: d.income - d.expense }))
      .filter(d => d.收入 > 0 || d.支出 > 0)
      .sort((a, b) => b.收入 - a.收入);
  }, [confirmedRecords]);



  const formatDate = (dateString: string | number) => {
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
    return amount.toLocaleString();
  };

  const handleDeleteClick = (recordId: string) => {
    setDeleteConfirmRecordId(recordId);
  };

  const handleConfirmDelete = () => {
    if (deleteConfirmRecordId) {
      const recordToDelete = tabFilteredRecords.find(r => r.id === deleteConfirmRecordId);
      onDeleteRecord(deleteConfirmRecordId, !!recordToDelete?.isBaizhan, !!recordToDelete?.isTrial);
      toast.success('删除成功');
      setDeleteConfirmRecordId(null);
    }
  };

  const handleCancelDelete = () => {
    setDeleteConfirmRecordId(null);
  };

  return (
    <div className="flex flex-col h-full gap-5">
      <div className="flex items-start justify-between gap-4 flex-shrink-0">
        <div className="flex items-center gap-4">
          <button
            onClick={onBack}
            className="p-2 hover:bg-base rounded-lg transition-colors"
          >
            <ArrowLeft className="w-5 h-5 text-muted" />
          </button>
          <div>
            <h2 className="text-2xl font-bold text-main">收益概览</h2>
            <p className="text-sm text-muted mt-1">
              {period === 'week' ? '本周' : period === 'season' ? '本赛季' : '全部'}共记录 {filteredRecords.length} 条收益与支出数据
            </p>
          </div>
        </div>

        <div className="flex items-center gap-1 bg-base rounded-lg p-1 border border-base">
          <button
            onClick={() => handlePeriodChange('week')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${period === 'week'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            本周
          </button>
          {currentSeason && (
            <button
              onClick={() => handlePeriodChange('season')}
              className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${period === 'season'
                ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
                : 'text-muted hover:text-main'
                }`}
            >
              本赛季
            </button>
          )}
          <button
            onClick={() => handlePeriodChange('all')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${period === 'all'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            全部
          </button>
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto pr-1 space-y-5">
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
          {stats.scrapsRecordCount > 0 && (
            <p className="text-muted text-sm mt-1">散件预估收益: {formatGold(stats.scrapsEstimatedIncome)} 金</p>
          )}
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
          {stats.scrapsRecordCount > 0 && (
            <p className="text-muted text-sm mt-1" title="白名单物品的实际购买花费合计；旧版记录按副本总支出近似">
              散件支出: {formatGold(stats.scrapsExpense)} 金
            </p>
          )}
        </div>

        <div className="bg-surface rounded-xl p-5 shadow-sm border border-base">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-2 bg-base rounded-lg">
              <TrendingUp className="w-6 h-6 text-muted" />
            </div>
            <span className="text-muted font-medium">净收入</span>
          </div>
          <p className={`text-3xl font-bold ${stats.netIncome >= 0 ? 'text-ds-success' : 'text-ds-warning'}`}>
            {stats.netIncome >= 0 ? '' : '-'}{formatGold(Math.abs(stats.netIncome))}
          </p>
          <p className="text-muted text-sm mt-2">代清净入: {formatGold(stats.clientNetIncome)} 金</p>
          {stats.scrapsRecordCount > 0 && (
            <p className={`text-sm mt-1 ${stats.scrapsNetIncome >= 0 ? 'text-ds-success' : 'text-ds-warning'}`}>
              散件净收入: {stats.scrapsNetIncome >= 0 ? '' : '-'}{formatGold(Math.abs(stats.scrapsNetIncome))} 金
            </p>
          )}
        </div>
      </div>

      {/* 内容分区 Tab：汇总 / 散件白名单 / 收益记录 */}
      <div className="flex items-center gap-1 bg-base rounded-lg p-1 border border-base w-fit">
        {([
          { key: 'summary', label: '收益汇总' },
          { key: 'scraps', label: '散件白名单' },
          { key: 'records', label: '收益记录' },
        ] as const).map((tab) => (
          <button
            key={tab.key}
            onClick={() => setContentTab(tab.key)}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${contentTab === tab.key
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* 收益分布图表 */}
      {contentTab === 'summary' && (
      <div className="bg-surface rounded-xl shadow-sm border border-base p-5">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <h3 className="text-lg font-semibold text-main">收益分布</h3>
            <div className="flex items-center gap-1 bg-base rounded-lg p-0.5">
              <button
                onClick={() => setChartView('raid')}
                className={`px-3 py-1 rounded-md text-xs font-medium transition-all ${chartView === 'raid'
                  ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
                  : 'text-muted hover:text-main'
                  }`}
              >
                按副本
              </button>
              <button
                onClick={() => setChartView('role')}
                className={`px-3 py-1 rounded-md text-xs font-medium transition-all ${chartView === 'role'
                  ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
                  : 'text-muted hover:text-main'
                  }`}
              >
                按角色
              </button>
            </div>
          </div>
          <span className="text-sm text-muted">
            {period === 'week' ? '本周' : period === 'season' ? '本赛季' : '全部'}数据
          </span>
        </div>
        {chartView === 'raid' ? (
          chartData.length > 0 ? (
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
                    tickFormatter={(val) => Number(val).toLocaleString()}
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
                    {chartData.map((_entry: any, index: number) => (
                      <Cell key={`cell-${index}`} fill={`rgb(var(--primary-base) / ${0.7 - index * 0.05})`} />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center py-12 text-muted">
              <Coins className="w-10 h-10 text-muted/30 mb-2" />
              <p className="text-sm">暂无收益数据</p>
            </div>
          )
        ) : (
          roleChartData.length > 0 ? (
            <div className="h-64">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={roleChartData} margin={{ top: 20, right: 20, left: -10, bottom: 10 }}>
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
                    tickFormatter={(val) => Number(val).toLocaleString()}
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
                    formatter={(value: number, name: string) => [`${value.toLocaleString()} 金`, name]}
                  />
                  <Bar dataKey="收入" fill="rgb(var(--chart-income))" radius={[4, 4, 0, 0]} maxBarSize={40}>
                    <LabelList dataKey="收入" position="top" formatter={(val: number) => val.toLocaleString()} style={{ fill: 'rgb(var(--chart-income))', fontSize: 10 }} />
                  </Bar>
                  <Bar dataKey="支出" fill="rgb(var(--chart-expense))" radius={[4, 4, 0, 0]} maxBarSize={40}>
                    <LabelList dataKey="支出" position="top" formatter={(val: number) => val.toLocaleString()} style={{ fill: 'rgb(var(--chart-expense))', fontSize: 10 }} />
                  </Bar>
                  <Legend />
                </BarChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center py-12 text-muted">
              <Coins className="w-10 h-10 text-muted/30 mb-2" />
              <p className="text-sm">暂无角色数据</p>
            </div>
          )
        )}
      </div>
      )}

      {/* 散件白名单汇总（按物品统计） */}
      {contentTab === 'scraps' && (
      <div className="bg-surface rounded-xl shadow-sm border border-base p-5">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-base rounded-lg">
              <Boxes className="w-5 h-5 text-muted" />
            </div>
            <div>
              <h3 className="text-lg font-semibold text-main">散件白名单汇总</h3>
              <p className="text-sm text-muted">
                按物品统计获取总数、副本数与平均每个副本数量
              </p>
            </div>
          </div>
          <span className="text-sm text-muted">
            {period === 'week' ? '本周' : period === 'season' ? '本赛季' : '全部'}数据
          </span>
        </div>

        {scrapsSummary.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-[10px] text-muted border-b border-base">
                  <th className="text-left font-medium py-2 pr-3">物品</th>
                  <th className="text-left font-medium py-2 pr-3 w-16">分类</th>
                  <th className="text-right font-medium py-2 pr-3 w-20">获取总数</th>
                  <th className="text-right font-medium py-2 pr-3 w-20">副本数</th>
                  <th className="text-right font-medium py-2 pl-3 w-24">平均/副本</th>
                </tr>
              </thead>
              <tbody>
                {scrapsSummary.map((row) => (
                  <tr key={row.name} className="border-b border-base/50 last:border-b-0">
                    <td className="py-2 pr-3 text-main">{row.name}</td>
                    <td className="py-2 pr-3 text-muted">
                      {row.category === 'equipment' ? '装备' : '材料'}
                    </td>
                    <td className="py-2 pr-3 text-right font-semibold text-main">{row.totalCount.toLocaleString()}</td>
                    <td className="py-2 pr-3 text-right text-muted">{row.raidCount.toLocaleString()}</td>
                    <td className="py-2 pl-3 text-right text-muted">{row.avgPerRaid.toFixed(2)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center py-10 text-muted">
            <Boxes className="w-10 h-10 text-muted/30 mb-2" />
            <p className="text-sm">
              {stats.scrapsRecordCount === 0 ? '暂无散件老板记录' : '暂无散件数据'}
            </p>
          </div>
        )}
      </div>
      )}

      {contentTab === 'records' && (
      <div className="bg-surface rounded-xl shadow-sm border border-base overflow-hidden">
        <div className="p-4 border-b border-base">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-base rounded-lg">
                <Coins className="w-5 h-5 text-muted" />
              </div>
              <div>
                <h3 className="text-lg font-semibold text-main">收益记录</h3>
                <p className="text-sm text-muted">{tabFilteredRecords.length} 条记录</p>
              </div>
            </div>
            <div className="relative">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
              <input
                type="text"
                data-page-search-input="true"
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
                ? 'bg-surface text-ds-success dark:text-ds-success shadow-sm'
                : 'text-muted hover:text-main'
                }`}
            >
              收入
            </button>
            <button
              onClick={() => setActiveTab('expense')}
              className={`px-4 py-2 rounded-md text-sm font-medium transition-all ${activeTab === 'expense'
                ? 'bg-surface text-ds-warning dark:text-ds-warning shadow-sm'
                : 'text-muted hover:text-main'
                }`}
            >
              支出
            </button>
          </div>
        </div>

        <div>
          {tabFilteredRecords.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-muted">
              <Coins className="w-12 h-12 text-muted/30 mb-3" />
              <p className="text-[1rem]">暂无记录</p>
            </div>
          ) : (
            <div className="divide-y divide-base">
              {shownRecords.map((record) => {
                const netIncome = record.goldIncome - (record.goldExpense || 0);
                const summaryAmount = activeTab === 'income'
                  ? record.goldIncome
                  : activeTab === 'expense'
                    ? record.goldExpense || 0
                    : netIncome;
                const summaryLabel = activeTab === 'income'
                  ? '收入'
                  : activeTab === 'expense'
                    ? '支出'
                    : '净收入';
                const summaryColorClass = activeTab === 'income'
                  ? 'text-ds-success-strong dark:text-ds-success-strong'
                  : activeTab === 'expense'
                    ? 'text-ds-warning-strong dark:text-ds-warning-strong'
                    : netIncome > 0
                      ? 'text-ds-success-strong dark:text-ds-success-strong'
                      : netIncome < 0
                        ? 'text-ds-warning-strong dark:text-ds-warning-strong'
                        : 'text-muted';
                const summaryPrefix = activeTab === 'income' || (activeTab === 'all' && summaryAmount > 0)
                  ? '+'
                  : '';
                const isExpanded = expandedRecordId === record.id;
                const isRejected = record.source === 'auto' && record.status === 'rejected';

                return (
                  <div
                    key={record.id}
                    className={`transition-colors border-l-4 ${isExpanded
                      ? 'bg-base/50 border-primary'
                      : isRejected
                        ? 'bg-red-50/30 dark:bg-red-900/5 border-red-300 dark:border-red-800'
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
                          <div className="font-semibold text-main truncate text-[1rem] leading-tight mb-0.5 flex items-center gap-1.5">
                            <span className="truncate">{record.raidName}</span>
                            {isRejected && (
                              <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-[10px] font-medium rounded border border-red-200 dark:border-red-800 flex-shrink-0">
                                已拒绝
                              </span>
                            )}
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
                        {(record.hasXuanjing || record.hasMaJu || record.hasPet || record.hasPendant || record.hasMount || record.hasAppearance || record.hasTitle || record.hasSecretBook) && (
                          <>
                            {record.hasXuanjing && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-ds-warning-soft dark:bg-ds-warning-soft/20 text-ds-warning-strong dark:text-ds-warning-strong text-xs font-medium rounded border border-ds-warning-soft dark:border-ds-warning-soft flex-shrink-0">
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
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-ds-success-soft dark:bg-ds-success-soft/20 text-ds-success-strong dark:text-ds-success-strong text-xs font-medium rounded border border-ds-success-soft dark:border-ds-success-soft flex-shrink-0">
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
                            {record.hasSecretBook && (
                              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-cyan-50 dark:bg-cyan-900/20 text-cyan-700 dark:text-cyan-400 text-xs font-medium rounded border border-cyan-100 dark:border-cyan-800 flex-shrink-0">
                                <BookOpen className="w-3 h-3" /> 秘籍
                              </span>
                            )}
                          </>
                        )}
                      </div>

                      {/* Right: Net Income, Time, Actions */}
                      <div className="flex items-center gap-4 flex-shrink-0">
                        {/* Net Income */}
                        <div className="text-right flex flex-col items-end w-24">
                          <span className={`font-bold text-[1rem] tabular-nums ${summaryColorClass}`}>
                            {summaryPrefix}{formatGold(summaryAmount)}
                          </span>
                          <span className="text-xs text-muted flex items-center gap-1">
                            <span>{summaryLabel}</span>
                            <span className="text-muted/40">·</span>
                            {formatDate(record.date).split(' ')[0]}
                          </span>
                        </div>

                        {/* Actions */}
                        <div className="flex items-center gap-1">
                          {record.isTrial ? (
                            /* 试炼记录：不提供编辑按钮（通过试炼详情弹窗 inline-edit 修改） */
                            null
                          ) : !record.isBaizhan ? (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                onEditRecord(record as RaidRecord);
                              }}
                              className="p-2 rounded-lg text-muted hover:text-primary hover:bg-surface active:scale-95 transition-all"
                              title="修改"
                            >
                              <Pencil size={18} />
                            </button>
                          ) : (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                onEditBaizhanRecord(record as unknown as BaizhanRecord);
                              }}
                              className="p-2 rounded-lg text-muted hover:text-primary hover:bg-surface active:scale-95 transition-all"
                              title="修改百战记录"
                            >
                              <Pencil size={18} />
                            </button>
                          )}
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
                          <div className="flex gap-6 text-sm flex-wrap">
                            <div className="flex items-center gap-2 text-ds-success-strong dark:text-ds-success-strong bg-ds-success-soft dark:bg-ds-success-soft/10 px-2 py-1 rounded">
                              <TrendingUp className="w-3.5 h-3.5" />
                              <span className="font-medium">收入: {formatGold(record.goldIncome)}</span>
                            </div>
                            {(record.goldExpense || 0) > 0 && (
                              <div className="flex items-center gap-2 text-ds-warning-strong dark:text-ds-warning-strong bg-ds-warning-soft dark:bg-ds-warning-soft/10 px-2 py-1 rounded">
                                <TrendingDown className="w-3.5 h-3.5" />
                                <span className="font-medium">支出: {formatGold(record.goldExpense || 0)}</span>
                              </div>
                            )}
                            {((record.scrapsItems && record.scrapsItems.length > 0) || (record.scrapsValue ?? 0) > 0) && (
                              <div
                                className="flex items-center gap-2 text-muted bg-base px-2 py-1 rounded"
                                title={record.isScrapsBoss ? '散件估价（已计入统计）' : '散件估价（仅展示，未计入）'}
                              >
                                <Boxes className="w-3.5 h-3.5" />
                                <span className="font-medium">
                                  散件估价: {formatGold(getRecordScrapsValue(record))}
                                  {record.isScrapsBoss && <span className="text-ds-success ml-1">*</span>}
                                </span>
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

          {tabFilteredRecords.length > shownRecords.length && (
            <div className="flex justify-center py-4 border-t border-base">
              <button
                onClick={() => setRenderLimit(prev => prev + PAGE_SIZE)}
                className="px-6 py-2 bg-base hover:bg-border rounded-lg text-sm text-muted hover:text-main transition-colors"
              >
                加载更多（已显示 {shownRecords.length} / {tabFilteredRecords.length} 条）
              </button>
            </div>
          )}
        </div>
      </div>
      )}

      {/* Delete Confirmation Dialog */}
      {deleteConfirmRecordId && (
        <div className="fixed inset-0 bg-black/40 backdrop-blur-sm flex items-center justify-center z-50">
          <div className="bg-surface p-6 rounded-lg shadow-ds-modal border border-base max-w-sm w-full mx-4">
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
    </div>
  );
};
