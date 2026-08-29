import React, { useState, useRef, useMemo } from 'react';
import { Coins, X } from 'lucide-react';
import { ScrapsItem } from '../types';
import { SCRAPS_MATERIAL_WHITELIST, computeScrapsExpense, computeScrapsValue, normalizeScrapsItems } from '../utils/scrapsUtils';
import { formatGold } from '../utils/goldFormat';

interface ScrapsItemsEditorProps {
  items: ScrapsItem[];
  onChange: (items: ScrapsItem[]) => void;
  /** 允许编辑数量与删除（默认 true；扫描确认弹窗传 false） */
  editable?: boolean;
  /** 显示「类型·来源」标签（默认 false；扫描确认弹窗传 true） */
  showSourceLabel?: boolean;
  /** 允许添加自定义散件（默认 false；手动新增弹窗传 true） */
  allowAddCustom?: boolean;
  /** 是否为散件老板（影响合计标签文案） */
  isScrapsBoss?: boolean;
  /** 清单最大高度（Tailwind class，默认 max-h-52；传 none 则跟随外层滚动，避免嵌套滚动条） */
  listMaxHeightClass?: string;
}

/** 判断是否为白名单材料项 */
const isWhitelistMaterial = (item: ScrapsItem): boolean =>
  item.category === 'material' && SCRAPS_MATERIAL_WHITELIST.includes(item.name);

/** 分组渲染结构：rows 携带 displayItems 原始索引，保证编辑/删除定位正确 */
interface ScrapsItemGroup {
  key: 'material' | 'equipment';
  label: string;
  rows: Array<{ item: ScrapsItem; idx: number }>;
}

/**
 * 散件清单编辑器（AddRecordModal 与 PendingRecordsPanel 共享）
 *
 * allowAddCustom 模式：白名单材料默认全部展示（count=0），不可删除；
 * 仅支持添加自定义装备。npc 来源单价只读；手填空值用 amber 提示。
 * 清单按「材料 / 装备」两组显示，组标题右侧展示组内估价小计。
 * 入口对 items 做归一化，兼容 v2.1.53 旧数据的 snake_case 键与 npc 单价铜→金问题。
 */
export const ScrapsItemsEditor: React.FC<ScrapsItemsEditorProps> = ({
  items,
  onChange,
  editable = true,
  showSourceLabel = false,
  allowAddCustom = false,
  isScrapsBoss = false,
  listMaxHeightClass = 'max-h-52',
}) => {
  const [customName, setCustomName] = useState('');
  const [customCount, setCustomCount] = useState(1);
  const customNameRef = useRef<HTMLInputElement>(null);

  // 入口归一化：兼容旧版 snake_case 键（unit_price）并修正 npc 单价铜→金
  const normalizedItems = useMemo(() => normalizeScrapsItems(items), [items]);

  // 用 ref 跟踪最新 items，解决连续操作时闭包旧值问题
  const itemsRef = useRef(normalizedItems);
  itemsRef.current = normalizedItems;

  // allowAddCustom 模式：将白名单材料合并到显示列表中（count=0）
  // 纯计算，不触发副作用，避免与父组件 useEffect 竞争
  const displayItems = useMemo(() => {
    if (!allowAddCustom) return normalizedItems;
    const existing = new Set(
      normalizedItems.filter(it => it.category === 'material').map(it => it.name)
    );
    const missing = SCRAPS_MATERIAL_WHITELIST
      .filter(name => !existing.has(name))
      .map(name => ({
        name,
        count: 0,
        unitPrice: null,
        category: 'material' as const,
        priceSource: 'manual' as const,
      }));
    return [...normalizedItems, ...missing];
  }, [normalizedItems, allowAddCustom]);

  // displayItems 也需要 ref 跟踪
  const displayItemsRef = useRef(displayItems);
  displayItemsRef.current = displayItems;

  // 材料 / 装备 分组（保留原始索引，空组不显示）
  const groups = useMemo<ScrapsItemGroup[]>(() => {
    const materialRows: Array<{ item: ScrapsItem; idx: number }> = [];
    const equipmentRows: Array<{ item: ScrapsItem; idx: number }> = [];
    displayItems.forEach((item, idx) => {
      if (item.category === 'equipment') {
        equipmentRows.push({ item, idx });
      } else {
        materialRows.push({ item, idx });
      }
    });
    const result: ScrapsItemGroup[] = [];
    if (materialRows.length > 0) result.push({ key: 'material', label: '材料', rows: materialRows });
    if (equipmentRows.length > 0) result.push({ key: 'equipment', label: '装备', rows: equipmentRows });
    return result;
  }, [displayItems]);

  const commit = (next: ScrapsItem[]) => {
    itemsRef.current = next;
    onChange(next);
  };

  const updateItem = (idx: number, patch: Partial<ScrapsItem>) => {
    const next = displayItemsRef.current.slice();
    next[idx] = { ...next[idx], ...patch };
    commit(next);
  };

  const removeItem = (idx: number) => {
    commit(displayItemsRef.current.filter((_, i) => i !== idx));
  };

  const addCustomEquipment = () => {
    const name = customName.trim();
    if (!name) return;
    commit([
      ...displayItemsRef.current,
      { name, count: Math.max(1, customCount), unitPrice: null, category: 'equipment', priceSource: 'manual' },
    ]);
    setCustomName('');
    setCustomCount(1);
    customNameRef.current?.focus();
  };

  // 只统计 count>0 且未填单价的项
  const pendingCount = displayItems.filter(i => i.count > 0 && i.unitPrice == null).length;
  const totalValue = computeScrapsValue(displayItems);
  // 散件支出：白名单材料的实际购买花费合计（装备/小铁等其他购买不计入）
  const totalExpense = computeScrapsExpense(displayItems);
  const hasItems = displayItems.length > 0;

  const renderRow = (item: ScrapsItem, idx: number) => {
    const isReadOnly = item.priceSource === 'npc';
    const isManual = item.unitPrice == null;
    const isWhitelist = isWhitelistMaterial(item);
    const categoryLabel = item.category === 'material' ? '材料' : '装备';
    const sourceLabel = item.priceSource === 'npc'
      ? 'NPC'
      : item.priceSource === 'jx3box'
        ? 'JX3Box'
        : '手填';
    // 用左侧色条标识单价状态：待填=amber，已填=emerald，只读=neutral
    const indicatorColor = isReadOnly
      ? 'bg-base'
      : isManual
        ? 'bg-ds-warning'
        : 'bg-ds-success';
    // 白名单材料：数量最小 0；自定义装备：数量最小 1
    const minCount = isWhitelist ? 0 : 1;
    return (
      <div
        key={`${item.name}-${idx}`}
        className={`flex items-center gap-2 pl-0 pr-2 py-1.5 bg-surface rounded-lg border ${
          isManual && !isReadOnly
            ? 'border-ds-warning-soft dark:border-ds-warning-soft/50'
            : 'border-base'
        } ${item.count === 0 ? 'opacity-50' : ''}`}
      >
        {/* 左侧状态色条 */}
        <div className={`w-0.5 self-stretch rounded-full flex-shrink-0 ${indicatorColor}`} />

        {/* 名称 + 数量 */}
        <div className="flex items-center gap-1.5 flex-1 min-w-0">
          <span className="text-sm text-main truncate" title={item.name}>
            {item.name || '未命名'}
          </span>
          {editable ? (
            <input
              type="number"
              min={minCount}
              value={item.count}
              onChange={e => {
                const count = Number(e.target.value);
                updateItem(idx, { count: Number.isFinite(count) ? Math.max(minCount, count) : minCount });
              }}
              title="数量"
              className="w-12 px-1 py-0.5 rounded text-xs font-mono text-center border border-base bg-base focus:outline-none focus:ring-1 focus:ring-ds-success text-main"
            />
          ) : (
            <span className="text-xs text-muted font-mono whitespace-nowrap">×{item.count}</span>
          )}
        </div>

        {/* 来源标签 */}
        {showSourceLabel && (
          <span className="text-[10px] text-muted whitespace-nowrap px-1.5 py-0.5 rounded bg-base/60 flex-shrink-0">
            {categoryLabel}·{sourceLabel}
          </span>
        )}

        {/* 实际购买价（来自扫描的"花费[..]购买了"消息；装备/小铁等其他购买不在此显示） */}
        {item.totalPrice != null && item.totalPrice > 0 && (
          <span
            className="text-[10px] font-mono text-muted whitespace-nowrap flex-shrink-0"
            title="实际购买花费合计"
          >
            购{formatGold(item.totalPrice)}
          </span>
        )}

        {/* 单价输入 */}
        <div className="relative w-24 flex-shrink-0">
          <Coins className={`absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 ${isReadOnly ? 'text-slate-400' : 'text-ds-success-strong'}`} />
          <input
            type="number"
            min="0"
            readOnly={isReadOnly}
            value={item.unitPrice ?? ''}
            placeholder={isManual ? '单价' : ''}
            onChange={e => {
              const raw = e.target.value;
              const next = raw === '' ? null : Math.max(0, Number(raw));
              updateItem(idx, { unitPrice: next });
            }}
            className={`w-full pl-6 pr-7 py-1 rounded text-xs font-mono border focus:outline-none focus:ring-1 ${
              isReadOnly
                ? 'bg-base text-muted border-base cursor-not-allowed'
                : isManual
                  ? 'bg-surface border-ds-warning dark:border-ds-warning text-main placeholder:text-ds-warning focus:ring-ds-warning'
                  : 'bg-surface border-ds-success dark:border-ds-success text-main focus:ring-ds-success'
            }`}
          />
          <span className="absolute right-1.5 top-1/2 -translate-y-1/2 text-[10px] text-muted pointer-events-none">金</span>
        </div>

        {/* 删除（仅自定义装备项可删除） */}
        {editable && !isWhitelist && (
          <button
            type="button"
            onClick={() => removeItem(idx)}
            title="移除"
            className="text-muted hover:text-red-500 transition-colors flex-shrink-0 p-0.5"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </div>
    );
  };

  return (
    <div className="rounded-lg border border-base bg-base/40 p-3 space-y-2.5">
      {/* 添加自定义装备（仅手动新增弹窗） */}
      {allowAddCustom && (
        <div className="p-2.5 bg-surface rounded-lg border border-base">
          <div className="text-[11px] text-muted mb-1.5">添加装备</div>
          <div className="flex items-center gap-2 flex-wrap">
            <input
              ref={customNameRef}
              type="text"
              value={customName}
              onChange={e => setCustomName(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); addCustomEquipment(); } }}
              placeholder="装备名称"
              className="flex-1 min-w-[100px] px-2.5 py-1.5 bg-base border border-base rounded-md text-sm text-main placeholder:text-muted focus:outline-none focus:ring-1 focus:ring-ds-success"
            />
            <input
              type="number"
              min="1"
              value={customCount}
              onChange={e => setCustomCount(Math.max(1, Number(e.target.value) || 1))}
              title="数量"
              className="w-16 px-1.5 py-1.5 bg-base border border-base rounded-md text-sm font-mono text-center text-main focus:outline-none focus:ring-1 focus:ring-ds-success"
            />
            <button
              type="button"
              onClick={addCustomEquipment}
              disabled={!customName.trim()}
              className="px-2.5 py-1.5 rounded-md text-sm font-medium bg-ds-success text-white hover:bg-ds-success transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              添加
            </button>
          </div>
        </div>
      )}

      {/* 清单（按 材料 / 装备 分组显示） */}
      {hasItems ? (
        <>
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-muted">散件清单（{displayItems.length} 项）</span>
            {pendingCount > 0 && (
              <span className="flex items-center gap-1 text-[11px] font-medium text-ds-warning-strong dark:text-ds-warning-strong">
                <span className="inline-flex items-center justify-center min-w-[16px] h-4 px-1 rounded-full bg-ds-warning-soft dark:bg-ds-warning-soft/30 text-ds-warning-strong dark:text-ds-warning-strong text-[10px] font-bold">
                  {pendingCount}
                </span>
                项待填单价
              </span>
            )}
          </div>
          <div className={`space-y-1.5 pr-1 ${listMaxHeightClass === 'none' ? '' : `${listMaxHeightClass} overflow-y-auto`}`}>
            {groups.map(group => {
              const groupValue = computeScrapsValue(group.rows.map(r => r.item));
              return (
                <div key={group.key} className="space-y-1.5">
                  {/* 组标题：名称 + 数量 + 组内估价小计 */}
                  <div className="flex items-center justify-between px-0.5 pt-0.5">
                    <span className="text-[11px] font-medium text-muted">
                      {group.label}
                      <span className="ml-1 text-muted/70">({group.rows.length})</span>
                    </span>
                    <span className="text-[11px] font-mono text-muted">
                      小计 {formatGold(groupValue)}
                    </span>
                  </div>
                  {group.rows.map(({ item, idx }) => renderRow(item, idx))}
                </div>
              );
            })}
          </div>
        </>
      ) : (
        <div className="text-xs text-muted py-1">
          {allowAddCustom ? '暂无散件物品' : '本次未识别散件物品'}
        </div>
      )}

      {/* 估价合计 */}
      <div className="flex items-center justify-between pt-2 border-t border-base">
        <span className="text-xs text-muted">
          散件估价合计
          <span className="ml-1 text-muted/70">（未填单价按 0 计入）</span>
        </span>
        <div className="flex items-center gap-1.5">
          <span className={`font-mono font-semibold ${
            pendingCount > 0 ? 'text-ds-warning dark:text-ds-warning' : 'text-muted'
          }`}>
            {formatGold(totalValue)}
          </span>
          <span className={`text-[10px] font-medium px-1.5 py-0.5 rounded ${
            isScrapsBoss
              ? 'bg-ds-success-soft dark:bg-ds-success-soft/20 text-ds-success-strong dark:text-ds-success-strong'
              : 'bg-base/60 text-muted'
          }`}>
            {isScrapsBoss ? '已计入' : '仅展示'}
          </span>
        </div>
      </div>

      {/* 散件支出合计（仅存在购买数据时显示） */}
      {totalExpense > 0 && (
        <div
          className="flex items-center justify-between pt-1.5 border-t border-base/60"
          title="白名单材料的实际购买花费合计；装备/小铁等其他购买不属于散件支出"
        >
          <span className="text-xs text-muted">
            散件支出合计
            <span className="ml-1 text-muted/70">（白名单材料实际购买价）</span>
          </span>
          <span className="font-mono font-semibold text-ds-warning dark:text-ds-warning">
            {formatGold(totalExpense)}
          </span>
        </div>
      )}
    </div>
  );
};
