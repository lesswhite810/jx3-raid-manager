import React, { useMemo, useState } from 'react';
import { ArrowLeft } from 'lucide-react';
import { RaidRecord, Account, Season } from '../types';
import { getLastMonday } from '../utils/cooldownManager';
import { buildSpecialDropRecords, SpecialDropRecord, SpecialDropType, SPECIAL_DROP_DEFINITIONS } from '../utils/rareDropUtils';
import { db } from '../services/db';

interface CrystalDetailProps {
  records: RaidRecord[];
  accounts: Account[];
  initialPeriod: 'week' | 'season' | 'all';
  onPeriodChange: (period: 'week' | 'season' | 'all') => void;
  onBack: () => void;
}

interface CrystalRoleStats {
  roleId: string;
  roleName: string;
  server: string;
  totalCount: number;
  records: SpecialDropRecord[];
  dropTypeCounts: Map<SpecialDropType, number>;
}

const ALL_DROP_TYPES: SpecialDropType[] = SPECIAL_DROP_DEFINITIONS.map(d => d.label);

export const CrystalDetail: React.FC<CrystalDetailProps> = ({ records, accounts, initialPeriod, onPeriodChange, onBack }) => {
  const [expandedRoleId, setExpandedRoleId] = useState<string | null>(null);
  const [statsPeriod, setStatsPeriod] = useState<'week' | 'season' | 'all'>(initialPeriod);
  const [currentSeason, setCurrentSeason] = useState<Season | null>(null);
  const [seasonLoaded, setSeasonLoaded] = useState(false);
  // 掉落类型筛选：默认全部选中，点击切换
  const [selectedTypes, setSelectedTypes] = useState<Set<SpecialDropType>>(() => new Set(ALL_DROP_TYPES));

  React.useEffect(() => {
    setStatsPeriod(initialPeriod);
  }, [initialPeriod]);

  React.useEffect(() => {
    db.getCurrentSeason().then((s) => {
      setCurrentSeason(s);
      setSeasonLoaded(true);
    }).catch(() => {
      setSeasonLoaded(true);
    });
  }, []);

  React.useEffect(() => {
    if (seasonLoaded && !currentSeason && statsPeriod === 'season') {
      handleStatsPeriodChange('week');
    }
  }, [seasonLoaded, currentSeason, statsPeriod]);

  const handleStatsPeriodChange = (nextPeriod: 'week' | 'season' | 'all') => {
    setStatsPeriod(nextPeriod);
    onPeriodChange(nextPeriod);
  };

  const safeRecords = Array.isArray(records) ? records : [];
  const safeAccounts = Array.isArray(accounts) ? accounts : [];

  const findRoleInfo = (accountId: string, roleId: string): { roleName: string; server: string } => {
    for (const account of safeAccounts) {
      if (account.id === accountId) {
        const roles = account.roles || [];
        const role = roles.find(item => item.id === roleId);
        if (role) {
          return {
            roleName: role.name,
            server: role.server
          };
        }
      }
    }
    return { roleName: '', server: '' };
  };

  // 获取时间段筛选的开始时间
  const getPeriodStartTime = (): number | null => {
    if (statsPeriod === 'all') return null;
    const now = new Date();
    if (statsPeriod === 'week') {
      return getLastMonday(now).getTime();
    } else {
      if (currentSeason?.startDate) {
        const sd = currentSeason.startDate;
        return sd > 1e12 ? sd : sd * 1000;
      }
      return 0;
    }
  };

  // 获取当前版本（赛季）的时间范围
  const getSeasonRange = (): { start: number; end: number } | null => {
    if (!currentSeason?.startDate) return null;
    const sd = currentSeason.startDate;
    const start = sd > 1e12 ? sd : sd * 1000;
    const ed = currentSeason.endDate;
    const end = ed ? (ed > 1e12 ? ed : ed * 1000) : Date.now();
    return { start, end };
  };

  const periodStartTime = getPeriodStartTime();

  // 统计所有团队副本中的稀有特殊掉落
  const allDropRecords = useMemo(() => {
    return buildSpecialDropRecords(safeRecords, periodStartTime);
  }, [safeRecords, periodStartTime]);

  // 按类型统计全量计数（不受类型筛选影响，用于 chip 上展示真实数量）
  const typeCounts = useMemo(() => {
    const map = new Map<SpecialDropType, number>();
    allDropRecords.forEach(r => {
      map.set(r.type, (map.get(r.type) || 0) + 1);
    });
    return map;
  }, [allDropRecords]);

  // 按类型筛选后的掉落记录
  const filteredDropRecords = useMemo(() => {
    if (selectedTypes.size === ALL_DROP_TYPES.length) {
      return allDropRecords;
    }
    return allDropRecords.filter(r => selectedTypes.has(r.type));
  }, [allDropRecords, selectedTypes]);

  const toggleType = (type: SpecialDropType) => {
    setSelectedTypes(prev => {
      const next = new Set(prev);
      if (next.has(type)) {
        next.delete(type);
      } else {
        next.add(type);
      }
      return next;
    });
  };

  const toggleAllTypes = () => {
    setSelectedTypes(prev => {
      if (prev.size === ALL_DROP_TYPES.length) {
        return new Set<SpecialDropType>();
      }
      return new Set(ALL_DROP_TYPES);
    });
  };

  const roleStats = useMemo<CrystalRoleStats[]>(() => {
    const roleMap = new Map<string, CrystalRoleStats>();

    filteredDropRecords.forEach(record => {
      const roleId = record.roleId;

      if (!roleMap.has(roleId)) {
        const roleInfo = findRoleInfo(record.accountId, roleId);
        roleMap.set(roleId, {
          roleId,
          roleName: record.roleName || roleInfo.roleName || '未知角色',
          server: record.server || roleInfo.server || '未知服务器',
          totalCount: 0,
          records: [],
          dropTypeCounts: new Map<SpecialDropType, number>()
        });
      }

      const stats = roleMap.get(roleId)!;
      stats.totalCount++;
      stats.records.push(record);
      stats.dropTypeCounts.set(record.type, (stats.dropTypeCounts.get(record.type) || 0) + 1);
    });

    return Array.from(roleMap.values()).sort((a, b) => b.totalCount - a.totalCount);
  }, [filteredDropRecords, safeAccounts]);

  const getBadgeClassName = (type: SpecialDropType): string => {
    if (type === '玄晶') {
      return 'bg-violet-50 text-violet-600 border-violet-200 dark:bg-violet-900/20 dark:text-violet-300 dark:border-violet-800/30';
    }

    if (type === '马具' || type === '宠物' || type === '挂件' || type === '坐骑') {
      return 'bg-amber-50 text-amber-600 border-amber-200 dark:bg-amber-900/20 dark:text-amber-300 dark:border-amber-800/30';
    }

    return 'bg-slate-100 text-slate-600 border-slate-200 dark:bg-slate-800 dark:text-slate-300 dark:border-slate-700';
  };

  const DROP_TYPE_ORDER: SpecialDropType[] = ['玄晶', '外观', '坐骑', '称号', '宠物', '马具', '挂件', '秘籍'];

  const getOrderedDropTypes = (dropTypeCounts: Map<SpecialDropType, number>): { type: SpecialDropType; count: number }[] => {
    return DROP_TYPE_ORDER
      .filter(type => (dropTypeCounts.get(type) || 0) > 0)
      .map(type => ({ type, count: dropTypeCounts.get(type)! }));
  };

  const toggleExpand = (roleId: string) => {
    setExpandedRoleId(expandedRoleId === roleId ? null : roleId);
  };

  const totalDrops = filteredDropRecords.length;
  const xuanjingTotal = filteredDropRecords.filter(r => r.type === '玄晶').length;
  const totalRoles = roleStats.length;
  const allDropsTotal = allDropRecords.length;
  const hasTypeFilter = selectedTypes.size !== ALL_DROP_TYPES.length;
  const noTypeSelected = selectedTypes.size === 0;

  // 玄晶统计卡：始终显示当前版本（赛季）所有副本出的玄晶数量
  // 不受时间范围切换（week/season/all）和类型筛选影响
  const seasonRange = useMemo(() => getSeasonRange(), [currentSeason]);
  const xuanjingTotalInSeason = useMemo(() => {
    if (!seasonRange) {
      // 无赛季配置时回退到当前筛选范围的玄晶数
      return xuanjingTotal;
    }
    return safeRecords.filter(r => {
      if (!r.hasXuanjing) return false;
      const t = typeof r.date === 'number' ? r.date : new Date(r.date).getTime();
      return t >= seasonRange.start && t <= seasonRange.end;
    }).length;
  }, [safeRecords, seasonRange, xuanjingTotal]);

  return (
    <div className="space-y-5">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-4">
          <button
            onClick={onBack}
            className="p-2 hover:bg-base rounded-lg transition-colors"
          >
            <ArrowLeft className="w-5 h-5 text-muted" />
          </button>
          <div>
            <h2 className="text-2xl font-bold text-main">稀有掉落统计</h2>
            <p className="text-sm text-muted mt-1">
              {statsPeriod === 'week' ? '本周' : statsPeriod === 'season' ? '本赛季' : '全部'}
              {hasTypeFilter
                ? `已筛选 ${totalDrops} / ${allDropsTotal} 次特殊掉落，来自 ${totalRoles} 个角色`
                : `共获取 ${totalDrops} 次特殊掉落，来自 ${totalRoles} 个角色`}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1 bg-base rounded-lg p-1 border border-base">
          <button
            onClick={() => handleStatsPeriodChange('week')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${statsPeriod === 'week'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            本周
          </button>
          {currentSeason && (
            <button
              onClick={() => handleStatsPeriodChange('season')}
              className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${statsPeriod === 'season'
                ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
                : 'text-muted hover:text-main'
                }`}
            >
              本赛季
            </button>
          )}
          <button
            onClick={() => handleStatsPeriodChange('all')}
            className={`px-4 py-1.5 rounded-md text-sm font-medium transition-all ${statsPeriod === 'all'
              ? 'bg-surface text-primary shadow-sm ring-1 ring-base'
              : 'text-muted hover:text-main'
              }`}
          >
            全部
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-surface rounded-xl p-5 shadow-sm border border-base">
          <div className="flex items-center gap-3 mb-3">
            <span className="text-muted font-medium text-sm">特殊掉落总数</span>
          </div>
          <p className="text-3xl font-bold text-main">{totalDrops}</p>
        </div>
        <div className="bg-surface rounded-xl p-5 shadow-sm border border-base">
          <div className="flex items-center gap-3 mb-3">
            <span className="text-muted font-medium text-sm">当前版本玄晶</span>
          </div>
          <p className="text-3xl font-bold text-violet-600 dark:text-violet-400">{xuanjingTotalInSeason}</p>
        </div>
        <div className="bg-surface rounded-xl p-5 shadow-sm border border-base">
          <div className="flex items-center gap-3 mb-3">
            <span className="text-muted font-medium text-sm">涉及角色</span>
          </div>
          <p className="text-3xl font-bold text-main">{totalRoles}</p>
        </div>
      </div>

      {/* 掉落类型筛选：默认全选，点击切换。chip 上展示该类型全量数量。 */}
      <div className="bg-surface rounded-xl shadow-sm border border-base p-4">
        <div className="flex items-center justify-between gap-3 mb-3 flex-wrap">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-main">按类型筛选</span>
            <span className="text-xs text-muted">
              已选 {selectedTypes.size} / {ALL_DROP_TYPES.length} 类
            </span>
          </div>
          <button
            onClick={toggleAllTypes}
            className="text-xs font-medium text-primary hover:text-primary-hover px-2 py-1 rounded-md hover:bg-primary/5 transition-colors"
          >
            {selectedTypes.size === ALL_DROP_TYPES.length ? '全部取消' : '全部选中'}
          </button>
        </div>
        <div className="flex flex-wrap gap-2">
          {DROP_TYPE_ORDER.map(type => {
            const active = selectedTypes.has(type);
            const count = typeCounts.get(type) || 0;
            const disabled = count === 0;
            return (
              <button
                key={type}
                onClick={() => toggleType(type)}
                disabled={disabled}
                className={`px-2.5 py-1 rounded-full text-xs font-medium border transition-all ${
                  active
                    ? 'bg-emerald-50 text-emerald-700 border-emerald-200 hover:border-emerald-300 dark:bg-emerald-900/20 dark:text-emerald-300 dark:border-emerald-800/30'
                    : 'bg-slate-50 text-slate-500 border-slate-200 hover:border-slate-300 dark:bg-slate-800/50 dark:text-slate-400 dark:border-slate-700/50'
                } ${disabled ? 'opacity-40 cursor-not-allowed' : 'cursor-pointer'}`}
                title={disabled ? `${type}：暂无记录` : `${active ? '隐藏' : '显示'}${type}（共 ${count} 条）`}
              >
                {type}×{count}
              </button>
            );
          })}
        </div>
        {noTypeSelected && (
          <p className="mt-3 text-xs text-amber-600 dark:text-amber-400">
            未选择任何类型，列表已隐藏所有记录。点击上方标签或「全部选中」即可恢复。
          </p>
        )}
      </div>

      <div className="bg-surface rounded-xl shadow-sm border border-base">
        {roleStats.length === 0 ? (
          <div className="text-center py-12">
            <div className="w-12 h-12 rounded-full bg-slate-100 dark:bg-slate-800 flex items-center justify-center mx-auto mb-3">
              <span className="text-slate-400 dark:text-slate-500 text-xl font-bold">0</span>
            </div>
            <p className="text-muted">
              {allDropsTotal === 0 ? '暂无稀有掉落记录' : '当前筛选条件下无匹配记录'}
            </p>
            <p className="text-sm text-muted/70 mt-1">
              {allDropsTotal === 0
                ? '团队副本掉落特殊物品后会自动记录在这里'
                : '调整类型筛选或点击「全部选中」即可查看完整列表'}
            </p>
            {allDropsTotal === 0 && safeRecords.length > 0 && (
              <p className="text-xs text-muted/50 mt-2">共有 {safeRecords.length} 条副本记录，暂无特殊掉落</p>
            )}
            {allDropsTotal > 0 && hasTypeFilter && (
              <p className="text-xs text-muted/50 mt-2">已隐藏 {allDropsTotal - totalDrops} 条其他类型记录</p>
            )}
          </div>
        ) : (
          <div className="space-y-1">
            {roleStats.map((stat, index) => {
              return (
                <div key={stat.roleId} className="border-b border-base last:border-b-0">
                  <button
                    onClick={() => toggleExpand(stat.roleId)}
                    className="w-full flex items-center gap-4 p-4 hover:bg-base/50 transition-colors text-left"
                  >
                    {/* 序号 - 固定宽度 */}
                    <div className="flex items-center justify-center w-8 h-8 bg-base rounded-lg font-bold text-sm text-main flex-shrink-0">
                      {index + 1}
                    </div>
                    {/* 角色信息 - 固定宽度，确保后续 tag 起始位置对齐 */}
                    <div className="w-40 md:w-52 flex-shrink-0 min-w-0">
                      <div className="flex items-center gap-2">
                        <h4 className="font-semibold text-main truncate">{stat.roleName}</h4>
                        {index === 0 && stat.totalCount > 0 && (
                          <span className="px-2 py-0.5 bg-primary/10 text-primary text-xs rounded-full flex-shrink-0">榜首</span>
                        )}
                      </div>
                      <p className="text-sm text-muted truncate">{stat.server}</p>
                    </div>
                    {/* 掉落 tag - flex-1，左对齐，wrap；起始位置固定，多 tag 自动换行 */}
                    <div className="flex-1 flex flex-wrap items-center gap-1.5 justify-start min-w-0">
                      {getOrderedDropTypes(stat.dropTypeCounts).map(({ type, count }) => (
                        <span
                          key={type}
                          className={`px-1.5 py-0.5 rounded text-[11px] font-medium border whitespace-nowrap ${getBadgeClassName(type)}`}
                        >
                          {type}×{count}
                        </span>
                      ))}
                    </div>
                    {/* 总计 + 展开箭头 - 固定宽度 */}
                    <div className="flex items-center gap-2 flex-shrink-0">
                      <span className="text-sm text-muted mr-1">掉落</span>
                      <span className="text-lg font-bold text-main w-8 text-right">{stat.totalCount}</span>
                      <span className="text-xs text-muted">次</span>
                      <svg
                        className={`w-4 h-4 text-muted transition-transform duration-200 ${expandedRoleId === stat.roleId ? 'rotate-180' : ''}`}
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                      </svg>
                    </div>
                  </button>

                  {expandedRoleId === stat.roleId && (
                    <div className="bg-surface p-4 border-t border-base">
                      <div className="space-y-3">
                        {stat.records.map((record, recordIndex) => (
                          <div key={`${record.id || record.date}-${recordIndex}`} className="flex items-center gap-4 p-3 border border-base rounded-lg">
                            <div className="flex-shrink-0 text-sm text-muted">
                              {new Date(record.date).toLocaleString('zh-CN', {
                                year: 'numeric',
                                month: '2-digit',
                                day: '2-digit',
                                hour: '2-digit',
                                minute: '2-digit'
                              })}
                            </div>
                            <div className="flex-1">
                              <p className="font-medium text-main">{record.raidName}</p>
                              <p className="text-xs text-muted">{record.notes || '稀有特殊掉落'}</p>
                            </div>
                            <div className="flex-shrink-0">
                              <span className={`px-2 py-0.5 rounded text-[11px] font-medium border ${getBadgeClassName(record.type)}`}>
                                {record.type}
                              </span>
                            </div>
                          </div>
                        ))}
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
  );
};
