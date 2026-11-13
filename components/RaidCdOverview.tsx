import React, { useEffect, useMemo, useState } from 'react';
import { Copy, KeyRound, Star, X } from 'lucide-react';
import { Account, BaizhanRecord, Raid, RaidRecord, TrialPlaceRecord } from '../types';
import { CdActivityCell, CdCell, CdCellStatus, CdOverviewRow, buildRaidCdOverview, resolveCdCellClickTarget } from '../utils/raidCdOverview';
import { SeasonContext } from '../utils/weeklyRaids';
import { getBaseServerName } from '../utils/serverUtils';
import { TrialProgressPeriod } from '../utils/trialProgress';
import { toast } from '../utils/toastManager';
import { SectIcon } from './SectIcon';
import { FilterSelect, FilterSelectOption } from './FilterSelect';

interface RaidCdOverviewProps {
  raids: Raid[];
  accounts: Account[];
  records: RaidRecord[];
  trialRecords: TrialPlaceRecord[];
  baizhanRecords: BaizhanRecord[];
  /** undefined 表示试炼周期仍在加载，此时试炼列不参与统计 */
  trialProgressPeriod?: TrialProgressPeriod | null;
  weeklyRaidNames: string[];
  favoriteRaids: ReadonlySet<string>;
  seasonContext: SeasonContext;
  /** 大版本名 → 新旧排名（0 最新），用于把周常副本列按离现在最近的赛季排序 */
  versionOrder?: Record<string, number>;
  /** 是否展示周常副本列（默认 true）。关闭时周常列仅保留当前赛季副本，收藏列不受影响 */
  showWeeklyRaids?: boolean;
  /** 当前版本等级上限（null/undefined 表示未配置，不做等级过滤） */
  levelCap?: number | null;
  /** 是否隐藏非满级角色（默认 false）。开启时等级已知且未达上限的角色不进入概览 */
  hideNonMaxLevelRoles?: boolean;
  /** 点击副本列头时进入对应副本详情 */
  onOpenRaid: (raid: Raid) => void;
  /** 点击「试炼之地」列头时切换到试炼之地页面 */
  onOpenTrial: () => void;
  /** 点击「百战」列头时切换到百战页面 */
  onOpenBaizhan: () => void;
  /** 点击团队副本的「待确认」单元格时打开该角色该副本的待确认记录确认框 */
  onRequestConfirmRaid: (scope: { roleId: string; raid: Raid }) => void;
  /** 点击副本单元格：未清 / 部分清 → 添加记录，已清 → 查看该角色该副本的记录 */
  onOpenRaidCell: (raid: Raid, row: CdOverviewRow, status: CdCellStatus) => void;
  /** 点击试炼单元格：待确认 / 已清 → 查看该角色的试炼记录，其余 → 添加记录 */
  onOpenTrialCell: (row: CdOverviewRow, status: CdCellStatus) => void;
  /** 点击百战单元格：待确认 / 已清 → 查看该角色的百战记录，其余 → 添加记录 */
  onOpenBaizhanCell: (row: CdOverviewRow, status: CdCellStatus) => void;
}

/** 单元格四态配色：与详情页卡片保持同一套语义（未清 success / 部分清 info / 待确认 warning / 完全清 neutral） */
const CELL_CLASS: Record<CdCellStatus, string> = {
  none: 'bg-ds-success-soft text-ds-success-strong border-ds-success-soft',
  partial: 'bg-ds-info-soft text-ds-info-strong border-ds-info-soft',
  pending: 'bg-ds-warning-soft text-ds-warning-strong border-ds-warning-soft',
  complete: 'bg-base text-ds-neutral border-ds-neutral-soft',
  na: 'bg-transparent text-muted border-transparent',
};

const CELL_HOVER_CLASS: Record<CdCellStatus, string> = {
  none: 'hover:bg-ds-success-soft hover:border-ds-success',
  partial: 'hover:bg-ds-info-soft hover:border-ds-info',
  pending: 'hover:bg-ds-warning-soft hover:border-ds-warning',
  complete: 'hover:bg-ds-neutral-soft hover:border-ds-neutral',
  na: '',
};

const CELL_TEXT: Record<CdCellStatus, string> = {
  none: '未清',
  partial: '部分',
  pending: '待确认',
  complete: '已清',
  na: '—',
};

/** 角色名清洗：与详情页一致，去掉「名字 名字」这类重复 */
const cleanRoleName = (name: string): string => {
  const parts = name.trim().split(/\s+/);
  if (parts.length > 1 && parts[0] === parts[1]) return parts[0];
  return name;
};

const renderCellText = (cell: CdCell): string => {
  if (cell.status === 'partial' && cell.totalCount > 0) {
    return `${cell.clearedCount}/${cell.totalCount}`;
  }
  return CELL_TEXT[cell.status];
};

/** 试炼 / 百战这类次数型单元格：一律显示「已完成/目标」，避免只有颜色没有数字 */
const renderActivityText = (cell: CdActivityCell): string => {
  if (cell.status === 'na') return CELL_TEXT.na;
  if (cell.status === 'pending') return CELL_TEXT.pending;
  return `${cell.clearedCount}/${cell.totalCount}`;
};

const formatEquipmentScore = (score?: number): string => {
  // 0 与未填都视为没有装分，与详情页「装分大于 0 才展示」保持一致
  if (typeof score !== 'number' || !Number.isFinite(score) || score <= 0) return '—';
  return score.toLocaleString('zh-CN');
};

/** 单元格的点击语义：未清/部分清去补记录，已清去看记录，待确认去处理 */
const cellActionHint = (status: CdCellStatus): string => {
  switch (status) {
    case 'none':
    case 'partial':
      return '点击添加记录';
    case 'complete':
      return '点击查看记录';
    case 'pending':
      return '点击处理待确认记录';
    default:
      return '';
  }
};

/** 操作列的小图标按钮：只复制内容，不展示原文 */
const COPY_ICON_BUTTON_CLASS =
  'inline-flex items-center justify-center p-1 rounded-tag text-muted hover:text-main hover:bg-ds-surface-2 transition-colors duration-ds-fast';

/** 列头里可点击的标题按钮：文字靠左、整块可点 */
const HEADER_BUTTON_CLASS = 'flex flex-col items-center gap-0.5 w-full group';

export const RaidCdOverview: React.FC<RaidCdOverviewProps> = ({
  raids,
  accounts,
  records,
  trialRecords,
  baizhanRecords,
  trialProgressPeriod,
  weeklyRaidNames,
  favoriteRaids,
  seasonContext,
  versionOrder,
  showWeeklyRaids,
  levelCap,
  hideNonMaxLevelRoles,
  onOpenRaid,
  onOpenTrial,
  onOpenBaizhan,
  onRequestConfirmRaid,
  onOpenRaidCell,
  onOpenTrialCell,
  onOpenBaizhanCell,
}) => {
  const [onlyUncleared, setOnlyUncleared] = useState(false);
  // 心法 / 账号过滤：空字符串 = 不过滤（全部）。选项由当前行数据去重推导，不展示已无角色的心法或账号
  const [martialFilter, setMartialFilter] = useState('');
  const [accountFilter, setAccountFilter] = useState('');

  const overview = useMemo(
    () => buildRaidCdOverview({
      raids,
      accounts,
      records,
      trialRecords,
      baizhanRecords,
      trialProgressPeriod,
      weeklyRaidNames,
      favoriteRaids,
      seasonContext,
      versionOrder,
      showWeeklyRaids,
      levelCap,
      hideNonMaxLevelRoles,
    }),
    [
      raids,
      accounts,
      records,
      trialRecords,
      baizhanRecords,
      trialProgressPeriod,
      weeklyRaidNames,
      favoriteRaids,
      seasonContext,
      versionOrder,
      showWeeklyRaids,
      levelCap,
      hideNonMaxLevelRoles,
    ],
  );

  const { columns, hasTrialParticipants, hasBaizhanParticipants } = overview;

  // 整行都是「不适用」（例如 10 人本里的代清号角色）时不占位置
  const rows = useMemo(
    () => overview.rows.filter(row => (
      (hasTrialParticipants && row.trial.status !== 'na')
      || (hasBaizhanParticipants && row.baizhan.status !== 'na')
      || columns.some(column => (row.cells[column.key]?.status ?? 'na') !== 'na')
    )),
    [overview.rows, columns, hasTrialParticipants, hasBaizhanParticipants],
  );

  // 心法 / 账号过滤选项：从当前行数据推导去重，保持行顺序，确保只展示实际存在的心法与账号
  const martialOptions = useMemo(() => {
    const seen = new Set<string>();
    rows.forEach(row => {
      if (row.martial) seen.add(row.martial);
    });
    return Array.from(seen);
  }, [rows]);

  const accountOptions = useMemo(() => {
    const seen = new Set<string>();
    rows.forEach(row => {
      if (row.accountName) seen.add(row.accountName);
    });
    return Array.from(seen);
  }, [rows]);

  // 过滤条件变化时，若所选心法 / 账号已不存在于数据中则自动重置，避免筛选永远为空
  const martialFilterValid = martialFilter === '' || martialOptions.includes(martialFilter);
  const accountFilterValid = accountFilter === '' || accountOptions.includes(accountFilter);
  useEffect(() => {
    if (!martialFilterValid) setMartialFilter('');
  }, [martialFilterValid]);
  useEffect(() => {
    if (!accountFilterValid) setAccountFilter('');
  }, [accountFilterValid]);

  const visibleRows = useMemo(() => {
    let filtered = rows;
    if (martialFilter) {
      filtered = filtered.filter(row => row.martial === martialFilter);
    }
    if (accountFilter) {
      filtered = filtered.filter(row => row.accountName === accountFilter);
    }
    if (onlyUncleared) {
      filtered = filtered.filter(
        row => row.noneClearedCount > 0 || row.partialClearedCount > 0 || row.pendingClearedCount > 0,
      );
    }
    return filtered;
  }, [rows, martialFilter, accountFilter, onlyUncleared]);

  const hasActiveFilters = martialFilter !== '' || accountFilter !== '';

  const totals = useMemo(() => {
    let none = 0;
    let partial = 0;
    let pending = 0;
    let complete = 0;
    rows.forEach(row => {
      none += row.noneClearedCount;
      partial += row.partialClearedCount;
      pending += row.pendingClearedCount;
      complete += row.completeClearedCount;
    });
    return { none, partial, pending, complete };
  }, [rows]);

  const copyText = async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`已复制${label}`);
    } catch (error) {
      console.error(`复制${label}失败:`, error);
      toast.error(`复制${label}失败，请稍后重试`);
    }
  };

  // 三类单元格统一按「所在模块 + 状态」分发，避免把试炼/百战的待确认错送到团队副本待确认面板
  const handleCellClick = (
    module: 'raid' | 'trial' | 'baizhan',
    status: CdCellStatus,
    row: CdOverviewRow,
    raid?: Raid,
  ) => {
    const target = resolveCdCellClickTarget(module, status);
    switch (target.kind) {
      case 'raid-confirm':
        if (raid) onRequestConfirmRaid({ roleId: row.roleId, raid });
        break;
      case 'raid-cell':
        if (raid) onOpenRaidCell(raid, row, status);
        break;
      case 'trial-cell':
        onOpenTrialCell(row, status);
        break;
      case 'baizhan-cell':
        onOpenBaizhanCell(row, status);
        break;
      case 'none':
        break;
    }
  };

  // 没有任何可展示的副本难度时不占用页面空间；行数为 0 仍需渲染（可能是筛选结果为空，需要给出提示）
  if (columns.length === 0 || rows.length === 0) return null;

  const buildFilterOptions = (values: string[], allLabel: string, withIcon: boolean): FilterSelectOption[] => [
    { value: '', label: allLabel },
    ...values.map(value => ({
      value,
      label: value,
      ...(withIcon ? { leading: <SectIcon sectName={value} variant="image" size="xs" /> } : {}),
    })),
  ];

  return (
    <section className="rounded-card border border-base bg-surface">
      <div className="flex items-center gap-3 px-4 py-3">
        <span className="text-sm font-bold text-main shrink-0">CD 概览</span>
        <span className="text-ds-xs text-muted shrink-0">
          {rows.length} 个角色 · {columns.length} 个副本
        </span>
        <span className="ml-auto flex items-center gap-3 text-ds-xs font-mono shrink-0">
          <span className="text-ds-success-strong">未清 {totals.none}</span>
          <span className="text-ds-info-strong">部分 {totals.partial}</span>
          {totals.pending > 0 && <span className="text-ds-warning-strong">待确认 {totals.pending}</span>}
          <span className="text-ds-neutral">已清 {totals.complete}</span>
        </span>
      </div>

      <div className="px-4 pb-4 space-y-3">
        <div className="flex items-center justify-between gap-3">
          <p className="text-ds-xs text-muted">
            绿=本周未清，雾蓝=部分清，棕=待确认，灰=已清；列头 0/7 为该副本本周已清角色数。点击列头进入详情，点击「待确认」按所在模块分别处理对应记录。
          </p>
          <div className="flex items-center gap-2 shrink-0">
            {martialOptions.length > 0 && (
              <FilterSelect
                value={martialFilter}
                onChange={setMartialFilter}
                options={buildFilterOptions(martialOptions, '全部心法', true)}
                ariaLabel="按心法过滤角色"
              />
            )}
            {accountOptions.length > 0 && (
              <FilterSelect
                value={accountFilter}
                onChange={setAccountFilter}
                options={buildFilterOptions(accountOptions, '全部账号', false)}
                ariaLabel="按账号过滤角色"
              />
            )}
            <label className="flex items-center gap-1.5 text-ds-xs text-muted cursor-pointer select-none">
              <input
                type="checkbox"
                checked={onlyUncleared}
                onChange={event => setOnlyUncleared(event.target.checked)}
                className="accent-current"
              />
              只看未清
            </label>
          </div>
        </div>

        {hasActiveFilters && visibleRows.length > 0 && (
          <div className="flex items-center gap-2 text-ds-xs text-muted">
            <span>当前筛选：{[martialFilter, accountFilter].filter(Boolean).join(' · ')}</span>
            <button
              type="button"
              onClick={() => {
                setMartialFilter('');
                setAccountFilter('');
              }}
              className="inline-flex items-center gap-0.5 text-muted hover:text-main transition-colors"
              title="清除全部筛选"
            >
              <X size={12} />
              清除
            </button>
          </div>
        )}

        <div className="overflow-x-auto border border-base rounded-card">
          <table className="w-full border-collapse text-ds-sm">
            <thead>
              <tr className="bg-base">
                <th
                  scope="col"
                  className="sticky left-0 z-10 bg-base text-left font-normal text-ds-xs text-muted px-3 py-2 border-b border-base w-[9.5rem] min-w-[8rem]"
                >
                  角色
                </th>
                <th
                  scope="col"
                  className="text-right font-normal text-ds-xs text-muted px-3 py-2 border-b border-l border-base w-[5.5rem] min-w-[5rem]"
                >
                  装分
                </th>
                {columns.map(column => (
                  <th
                    key={column.key}
                    scope="col"
                    className="border-b border-l border-base px-1 py-2 align-bottom"
                  >
                    <button
                      type="button"
                      onClick={() => onOpenRaid(column.raid)}
                      className={HEADER_BUTTON_CLASS}
                      title={`${column.raidName} · ${column.raid.playerCount}人${column.raid.difficulty}（本周已清 ${column.clearedCount}/${column.totalCount}）`}
                    >
                      <span className="flex items-center gap-1 text-ds-xs font-bold text-main">
                        {column.source === 'favorite' && <Star size={10} className="text-ds-warning shrink-0" />}
                        <span className="max-w-[6rem] truncate group-hover:text-ds-accent-strong">
                          {column.raidName}
                        </span>
                      </span>
                      <span className="text-[10px] font-mono text-muted whitespace-nowrap">
                        {column.raid.playerCount}人{column.raid.difficulty} · {column.clearedCount}/{column.totalCount}
                      </span>
                    </button>
                  </th>
                ))}
                {hasTrialParticipants && (
                  <th
                    scope="col"
                    className="border-b border-l border-base px-1 py-2 align-bottom bg-ds-surface-2/40"
                  >
                    <button
                      type="button"
                      onClick={onOpenTrial}
                      className={HEADER_BUTTON_CLASS}
                      title="试炼之地：本周翻牌进度，点击进入试炼之地"
                    >
                      <span className="text-ds-xs font-bold text-main group-hover:text-ds-accent-strong whitespace-nowrap">
                        试炼之地
                      </span>
                      <span className="text-[10px] text-muted whitespace-nowrap">本周翻牌</span>
                    </button>
                  </th>
                )}
                {hasBaizhanParticipants && (
                  <th
                    scope="col"
                    className="border-b border-l border-base px-1 py-2 align-bottom bg-ds-surface-2/40"
                  >
                    <button
                      type="button"
                      onClick={onOpenBaizhan}
                      className={HEADER_BUTTON_CLASS}
                      title="百战：本周完成次数，点击进入百战"
                    >
                      <span className="text-ds-xs font-bold text-main group-hover:text-ds-accent-strong whitespace-nowrap">
                        百战
                      </span>
                      <span className="text-[10px] text-muted whitespace-nowrap">本周次数</span>
                    </button>
                  </th>
                )}
                <th
                  scope="col"
                  className="text-center font-normal text-ds-xs text-muted px-2 py-2 border-b border-l border-base w-[4.5rem] min-w-[4rem]"
                >
                  操作
                </th>
              </tr>
            </thead>
            <tbody>
              {visibleRows.map(row => (
                <tr key={row.roleId} className="hover:bg-ds-surface-2">
                  <th
                    scope="row"
                    className="sticky left-0 z-10 bg-surface hover:bg-ds-surface-2 text-left font-normal px-3 py-2 border-b border-base min-w-[8rem]"
                  >
                    <div className="flex items-center gap-1.5 text-ds-sm text-main">
                      {row.martial && (
                        <SectIcon
                          sectName={row.martial}
                          variant="image"
                          size="xs"
                          className="shrink-0"
                        />
                      )}
                      <span className="truncate">
                        {cleanRoleName(row.roleName)}·{getBaseServerName(row.server)}
                      </span>
                    </div>
                  </th>
                  <td className="text-right px-3 py-2 border-b border-l border-base text-ds-xs font-mono text-muted whitespace-nowrap">
                    {formatEquipmentScore(row.equipmentScore)}
                  </td>
                  {columns.map(column => {
                    const cell = row.cells[column.key];
                    const status = cell?.status ?? 'na';
                    const isPending = status === 'pending';
                    return (
                      <td key={column.key} className="border-b border-l border-base p-0.5 text-center">
                        <button
                          type="button"
                          onClick={() => handleCellClick('raid', status, row, column.raid)}
                          disabled={status === 'na'}
                          className={`w-full min-w-[3.2rem] px-1 py-1 rounded-tag border text-ds-xs font-mono transition-colors duration-ds-fast ${CELL_CLASS[status]} ${CELL_HOVER_CLASS[status]} ${status === 'na' ? 'cursor-default' : ''}`}
                          title={isPending
                            ? `${cleanRoleName(row.roleName)} · ${column.raidName} ${column.raid.playerCount}人${column.raid.difficulty}（点击处理该角色该副本的待确认记录）`
                            : status === 'na'
                              ? `${cleanRoleName(row.roleName)} · ${column.raidName} ${column.raid.playerCount}人${column.raid.difficulty}（不适用）`
                              : `${cleanRoleName(row.roleName)} · ${column.raidName} ${column.raid.playerCount}人${column.raid.difficulty}（${cellActionHint(status)}）`}
                        >
                          {cell ? renderCellText(cell) : '—'}
                        </button>
                      </td>
                    );
                  })}
                  {hasTrialParticipants && (
                    <td className="border-b border-l border-base p-0.5 text-center bg-ds-surface-2/20">
                      <button
                        type="button"
                        onClick={() => handleCellClick('trial', row.trial.status, row)}
                        disabled={row.trial.status === 'na'}
                        className={`w-full min-w-[3.2rem] px-1 py-1 rounded-tag border text-ds-xs font-mono transition-colors duration-ds-fast ${CELL_CLASS[row.trial.status]} ${CELL_HOVER_CLASS[row.trial.status]} ${row.trial.status === 'na' ? 'cursor-default' : ''}`}
                        title={row.trial.status === 'na'
                          ? `${cleanRoleName(row.roleName)} 未参与试炼之地`
                          : row.trial.status === 'pending'
                            ? `${cleanRoleName(row.roleName)} · 试炼之地有记录待确认（点击查看并确认）`
                            : `${cleanRoleName(row.roleName)} · 试炼之地本周翻牌 ${row.trial.clearedCount}/${row.trial.totalCount}（${cellActionHint(row.trial.status)}）`}
                      >
                        {renderActivityText(row.trial)}
                      </button>
                    </td>
                  )}
                  {hasBaizhanParticipants && (
                    <td className="border-b border-l border-base p-0.5 text-center bg-ds-surface-2/20">
                      <button
                        type="button"
                        onClick={() => handleCellClick('baizhan', row.baizhan.status, row)}
                        disabled={row.baizhan.status === 'na'}
                        className={`w-full min-w-[3.2rem] px-1 py-1 rounded-tag border text-ds-xs font-mono transition-colors duration-ds-fast ${CELL_CLASS[row.baizhan.status]} ${CELL_HOVER_CLASS[row.baizhan.status]} ${row.baizhan.status === 'na' ? 'cursor-default' : ''}`}
                        title={row.baizhan.status === 'na'
                          ? `${cleanRoleName(row.roleName)} 未参与百战`
                          : row.baizhan.status === 'pending'
                            ? `${cleanRoleName(row.roleName)} · 百战有记录待确认（点击查看记录）`
                            : `${cleanRoleName(row.roleName)} · 百战本周 ${row.baizhan.clearedCount}/${row.baizhan.totalCount}（${cellActionHint(row.baizhan.status)}）`}
                      >
                        {renderActivityText(row.baizhan)}
                      </button>
                    </td>
                  )}
                  <td className="border-b border-l border-base px-2 py-1 text-center whitespace-nowrap">
                    <span className="inline-flex items-center gap-0.5">
                      <button
                        type="button"
                        onClick={() => copyText(row.accountName, '账号')}
                        className={COPY_ICON_BUTTON_CLASS}
                        title={`复制账号（${row.accountName}）`}
                        aria-label="复制账号"
                      >
                        <Copy size={13} />
                      </button>
                      {row.password && (
                        <button
                          type="button"
                          onClick={() => copyText(row.password ?? '', '密码')}
                          className={COPY_ICON_BUTTON_CLASS}
                          title="复制密码"
                          aria-label="复制密码"
                        >
                          <KeyRound size={13} />
                        </button>
                      )}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {visibleRows.length === 0 && (
          hasActiveFilters ? (
            <div className="flex items-center justify-center gap-2 text-ds-xs text-muted py-2">
              <span>当前筛选条件下没有角色。</span>
              <button
                type="button"
                onClick={() => {
                  setMartialFilter('');
                  setAccountFilter('');
                }}
                className="inline-flex items-center gap-0.5 text-muted hover:text-main transition-colors"
              >
                <X size={12} />
                清除筛选
              </button>
            </div>
          ) : (
            <p className="text-ds-xs text-muted text-center py-2">所有角色本周都已清完。</p>
          )
        )}
        {overview.disabledColumnCount > 0 && (
          <p className="text-ds-xs text-muted">
            已跳过 {overview.disabledColumnCount} 个被禁用的副本难度。
          </p>
        )}
      </div>
    </section>
  );
};
