import React, { useEffect, useId, useMemo, useState } from 'react';
import { Check, Loader2, Package, X } from 'lucide-react';
import { DualRangeSlider } from './DualRangeSlider';
import { JX3Equip } from '../services/jx3BoxApi';
import {
    filterTrialDrawEquipments,
    getTrialDrawEquipmentAttributes,
    getTrialDrawEquipmentBindLabel,
    getTrialDrawEquipmentLevelBounds,
    getTrialDrawEquipmentSlot,
    TRIAL_DRAW_ATTRIBUTE_FILTERS,
} from '../utils/trialDrawEquipmentFilters';

export type TrialEquipmentCatalogState = 'loading' | 'ready' | 'empty' | 'error';

interface TrialEquipmentPickerProps {
    equipments: JX3Equip[];
    catalogState: TrialEquipmentCatalogState;
    catalogMessage?: string | null;
    selectedEquipmentId?: string;
    onSelect: (equipmentId: string) => Promise<void> | void;
    onClear: () => Promise<void> | void;
    isSaving?: boolean;
    /** 选择器内最多渲染的结果数，避免历史目录过大时影响弹窗响应。 */
    maxResults?: number;
}

const clamp = (value: number, min: number, max: number): number => Math.max(min, Math.min(value, max));

const getInitialLevelRange = (bounds: { min: number; max: number } | null): { min: number; max: number } | null => (
    bounds ? { min: bounds.min, max: bounds.max } : null
);

const EquipmentClearAction: React.FC<{ disabled: boolean; onClear: () => Promise<void> | void }> = ({ disabled, onClear }) => (
    <button type="button" disabled={disabled} onClick={onClear} className="flex w-full items-center justify-center gap-1.5 rounded-tag border border-ds-warning-soft bg-ds-warning-soft px-3 py-2 text-xs font-medium text-ds-warning-strong transition-colors hover:bg-ds-warning-soft/70 disabled:cursor-not-allowed disabled:opacity-60">
        {disabled ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <X className="h-3.5 w-3.5" />}清空当前牌位装备
    </button>
);

/**
 * 试炼装备筛选与选择面板。
 *
 * 手工记录和自动逐次翻牌共用该组件；调用方只负责提供经各自来源校验后的目录，
 * 并在选择后写入自己的草稿或当前翻牌子记录。
 */
export const TrialEquipmentPicker: React.FC<TrialEquipmentPickerProps> = ({
    equipments,
    catalogState,
    catalogMessage,
    selectedEquipmentId = '',
    onSelect,
    onClear,
    isSaving = false,
    maxResults = 80,
}) => {
    const [selectedType, setSelectedType] = useState('全部');
    const [selectedBindType, setSelectedBindType] = useState<number | null>(null);
    const [attributeFilters, setAttributeFilters] = useState<Set<string>>(new Set());
    const levelBounds = useMemo(() => getTrialDrawEquipmentLevelBounds(equipments), [equipments]);
    const [selectedLevels, setSelectedLevels] = useState<{ min: number; max: number } | null>(() => getInitialLevelRange(levelBounds));
    const [minLevelInput, setMinLevelInput] = useState('');
    const [maxLevelInput, setMaxLevelInput] = useState('');
    const levelRangeDescriptionId = useId();

    useEffect(() => {
        if (!levelBounds) {
            setSelectedLevels(null);
            setMinLevelInput('');
            setMaxLevelInput('');
            return;
        }

        setSelectedLevels(current => {
            const previous = current ?? getInitialLevelRange(levelBounds);
            if (!previous) return null;
            const min = clamp(previous.min, levelBounds.min, levelBounds.max);
            const max = clamp(Math.max(previous.max, min), min, levelBounds.max);
            setMinLevelInput(String(min));
            setMaxLevelInput(String(max));
            return { min, max };
        });
    }, [levelBounds]);

    const types = useMemo(() => ['全部', ...Array.from(new Set(
        equipments.map(getTrialDrawEquipmentSlot),
    )).sort((left, right) => left.localeCompare(right, 'zh-CN'))], [equipments]);

    useEffect(() => {
        if (!types.includes(selectedType)) setSelectedType('全部');
    }, [selectedType, types]);

    const appliedLevels = selectedLevels ?? getInitialLevelRange(levelBounds);
    const matchingEquipments = useMemo(() => filterTrialDrawEquipments(equipments, {
        query: '',
        minLevel: appliedLevels?.min ?? 0,
        maxLevel: appliedLevels?.max ?? Number.MAX_SAFE_INTEGER,
        slot: selectedType,
        bindType: selectedBindType,
        attributes: attributeFilters,
    }), [appliedLevels?.max, appliedLevels?.min, attributeFilters, equipments, selectedBindType, selectedType]);
    const filteredEquipments = matchingEquipments.slice(0, maxResults);

    const updateLevelRange = (next: { min: number; max: number }) => {
        if (!levelBounds) return;
        const min = clamp(next.min, levelBounds.min, levelBounds.max);
        const max = clamp(Math.max(next.max, min), min, levelBounds.max);
        setSelectedLevels({ min, max });
        setMinLevelInput(String(min));
        setMaxLevelInput(String(max));
    };

    const commitLevelInput = (edge: 'min' | 'max') => {
        if (!levelBounds || !appliedLevels) return;
        const rawValue = edge === 'min' ? minLevelInput.trim() : maxLevelInput.trim();
        const fallback = edge === 'min' ? appliedLevels.min : appliedLevels.max;
        const parsed = Number(rawValue);
        const value = Number.isFinite(parsed) ? Math.round(parsed) : fallback;
        if (edge === 'min') {
            updateLevelRange({ min: clamp(value, levelBounds.min, appliedLevels.max), max: appliedLevels.max });
        } else {
            updateLevelRange({ min: appliedLevels.min, max: clamp(value, appliedLevels.min, levelBounds.max) });
        }
    };

    const clearFilters = () => {
        setSelectedType('全部');
        setSelectedBindType(null);
        setAttributeFilters(new Set());
        if (levelBounds) updateLevelRange({ min: levelBounds.min, max: levelBounds.max });
    };

    const clearAction = selectedEquipmentId ? <EquipmentClearAction disabled={isSaving} onClear={onClear} /> : null;

    if (catalogState !== 'ready') {
        const unavailableContent = catalogState === 'loading'
            ? <div className="flex min-h-36 flex-col items-center justify-center gap-2 px-4 py-6 text-sm text-muted"><Loader2 className="h-5 w-5 animate-spin" />正在读取装备目录…</div>
            : catalogState === 'error'
                ? <div className="rounded-card border border-red-200 bg-red-50 px-3 py-2.5 text-sm text-red-600 dark:border-red-900/40 dark:bg-red-900/20 dark:text-red-300">{catalogMessage || '装备目录读取失败，请关闭后重新打开记录窗口重试。'}</div>
                : <div className="rounded-card border border-ds-warning-soft bg-ds-warning-soft px-3 py-2.5 text-sm text-ds-warning-strong">{catalogMessage || '当前赛季装备目录为空，暂时无法选择装备。'}</div>;
        return <div className="space-y-3">{unavailableContent}{clearAction}</div>;
    }

    return <div className="space-y-3">
        <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-muted">按条件筛选装备</span>
            <span className="text-[11px] text-muted">名称相近时可结合部位、绑定方式和属性筛选</span>
            <span className="flex-1" />
            <button type="button" onClick={clearFilters} className="rounded-tag border border-base bg-surface px-2 py-1.5 text-xs text-muted transition-colors hover:bg-base hover:text-main">
                重置筛选
            </button>
        </div>

        {levelBounds && appliedLevels && <div className="space-y-2 rounded-card border border-base bg-base/30 p-2.5">
            <div className="flex flex-wrap items-center gap-2 text-xs">
                <span className="font-medium text-ds-fg-soft">品级</span>
                <label className="flex items-center gap-1 text-muted">
                    <span className="sr-only">最低装备品级</span>
                    <input type="text" inputMode="numeric" value={minLevelInput} onChange={event => setMinLevelInput(event.target.value)} onBlur={() => commitLevelInput('min')} onKeyDown={event => {
                        if (event.key === 'Enter') {
                            event.preventDefault();
                            commitLevelInput('min');
                            event.currentTarget.blur();
                        }
                    }} aria-label="最低装备品级" aria-describedby={levelRangeDescriptionId} className="w-20 rounded-tag border border-base bg-surface px-2 py-1 text-xs text-main outline-none focus:border-ds-success focus:ring-2 focus:ring-ds-success-soft" />
                    至
                </label>
                <label className="flex items-center gap-1 text-muted">
                    <span className="sr-only">最高装备品级</span>
                    <input type="text" inputMode="numeric" value={maxLevelInput} onChange={event => setMaxLevelInput(event.target.value)} onBlur={() => commitLevelInput('max')} onKeyDown={event => {
                        if (event.key === 'Enter') {
                            event.preventDefault();
                            commitLevelInput('max');
                            event.currentTarget.blur();
                        }
                    }} aria-label="最高装备品级" aria-describedby={levelRangeDescriptionId} className="w-20 rounded-tag border border-base bg-surface px-2 py-1 text-xs text-main outline-none focus:border-ds-success focus:ring-2 focus:ring-ds-success-soft" />
                </label>
                <span id={levelRangeDescriptionId} className="font-mono tabular-nums text-muted">可选 {levelBounds.min}–{levelBounds.max}</span>
            </div>
            {levelBounds.max > levelBounds.min && <DualRangeSlider min={levelBounds.min} max={levelBounds.max} value={appliedLevels} onChange={updateLevelRange} className="mx-1" />}
        </div>}

        <div className="flex flex-wrap gap-1.5" role="group" aria-label="按部位筛选装备">
            {types.map(type => {
                const selected = selectedType === type;
                return <button key={type} type="button" onClick={() => setSelectedType(type)} aria-pressed={selected} className={`rounded-tag border px-2.5 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong ${selected ? 'border-ds-success-soft bg-ds-success-soft text-ds-success-strong' : 'border-base bg-surface text-muted hover:bg-base'}`}>{type}</button>;
            })}
        </div>

        <div className="flex flex-wrap gap-1.5" role="group" aria-label="按绑定方式筛选装备">
            {[
                { value: null, label: '全部绑定' },
                { value: 2, label: '装备后绑定' },
                { value: 3, label: '拾取后绑定' },
            ].map(option => {
                const selected = selectedBindType === option.value;
                return <button key={option.label} type="button" onClick={() => setSelectedBindType(option.value)} aria-pressed={selected} className={`rounded-tag border px-2.5 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong ${selected ? 'border-ds-success-soft bg-ds-success-soft text-ds-success-strong' : 'border-base bg-surface text-muted hover:bg-base'}`}>{option.label}</button>;
            })}
        </div>

        <div className="flex flex-wrap gap-1.5" role="group" aria-label="按属性筛选装备">
            {TRIAL_DRAW_ATTRIBUTE_FILTERS.map(filter => {
                const selected = attributeFilters.has(filter.key);
                return <button key={filter.key} type="button" onClick={() => setAttributeFilters(current => {
                    const next = new Set(current);
                    if (next.has(filter.key)) next.delete(filter.key); else next.add(filter.key);
                    return next;
                })} aria-pressed={selected} className={`rounded-tag border px-2 py-1 text-[11px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong ${selected ? 'border-ds-success-soft bg-ds-success-soft text-ds-success-strong' : 'border-base bg-surface text-muted hover:bg-base'}`}>{filter.label}</button>;
            })}
        </div>

        {clearAction}

        {filteredEquipments.length > 0 ? <div className="grid grid-cols-1 gap-2 pr-1 sm:grid-cols-2 lg:grid-cols-3">
            {filteredEquipments.map(item => {
                const selected = selectedEquipmentId === item.ID.toString();
                const attributes = getTrialDrawEquipmentAttributes(item);
                const iconId = item._IconID ?? item.IconID;
                const iconUrl = iconId ? `https://icon.jx3box.com/icon/${iconId}.png` : null;
                return <button key={`${item.ID}-${item.UiID || item.Name}-${item.Level}`} type="button" disabled={isSaving} onClick={() => onSelect(item.ID.toString())}
                    className={`group min-h-24 rounded-card border p-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong disabled:cursor-not-allowed disabled:opacity-60 ${selected ? 'border-ds-success-soft bg-ds-success-soft/60' : 'border-base bg-surface hover:border-ds-success-soft hover:bg-base/50'}`}>
                    <div className="flex items-start gap-1.5">
                        <span className="relative mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-tag border border-base bg-base/30">
                            <Package className={`h-4 w-4 ${selected ? 'text-ds-success-strong' : 'text-muted group-hover:text-ds-success-strong'}`} />
                            {iconUrl && <img src={iconUrl} alt="" loading="lazy" onError={event => { event.currentTarget.style.display = 'none'; }} className="absolute inset-0 h-full w-full object-cover" />}
                        </span>
                        {selected && <Check className="mt-0.5 h-4 w-4 shrink-0 text-ds-success-strong" aria-label="已选择" />}
                        <span className="min-w-0 flex-1 break-words text-sm font-medium text-main">{item.Name}</span>
                    </div>
                    <div className="mt-2 space-y-1 pl-10 text-[11px] text-muted">
                        <p>{getTrialDrawEquipmentSlot(item)} · 品级 <span className="font-mono tabular-nums">{item.Level}</span></p>
                        <p>{getTrialDrawEquipmentBindLabel(item.BindType)}</p>
                        <p className="line-clamp-2">{attributes.join('、') || '属性未标注'}</p>
                    </div>
                </button>;
            })}
        </div> : <div className="flex min-h-32 flex-col items-center justify-center gap-2 rounded-card border border-dashed border-base bg-base/30 px-4 py-6 text-center text-muted">
            <Package className="h-5 w-5" />
            <span className="text-sm">没有找到匹配的装备</span>
            <span className="text-xs">请调整品级、部位、绑定方式或属性筛选</span>
        </div>}
        {matchingEquipments.length > filteredEquipments.length && <p className="text-[11px] text-muted">当前条件匹配 {matchingEquipments.length} 条；为保持流畅，最多显示 {maxResults} 条，请继续缩小筛选条件。</p>}
    </div>;
};
