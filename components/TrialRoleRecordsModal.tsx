import React, { useState, useMemo, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { TrialDraw, TrialPlaceRecord } from '../types';
import { X, Calendar, Trophy, Search, CheckCircle, Circle, Trash2, AlertCircle, Loader2, ChevronDown } from 'lucide-react';
import { SectIcon } from './SectIcon';
import { EquipmentCatalogState, TrialDrawEquipmentEditor } from './TrialDrawEquipmentEditor';
import { db } from '../services/db';
import { JX3Equip, JX3EquipMagicType } from '../services/jx3BoxApi';
import { getLastMonday, getNextMonday } from '../utils/cooldownManager';
import { getTrialEquipmentLevelRange, getTrialRecordEquipmentEntries, isTrialEquipmentLevelInRange } from '../utils/trialRecordUtils';
import { getTrialEquipmentSource, isTrialEquipmentFromSource } from '../utils/trialEquipmentSource';
import { trialRecordNeedsReview } from '../utils/trialReviewState';
import { classifyTrialRewards } from '../utils/trialRewards';
import { isTrialRecordInWeek } from '../utils/trialWeeklyDrawProgress';
import { toTrialProgressTimestamp } from '../utils/trialProgress';

interface RoleDisplayData {
    id: string;
    name: string;
    server: string;
    sect?: string;
    martial?: string;
    accountName: string;
}

interface TrialRoleRecordsModalProps {
    isOpen: boolean;
    onClose: () => void;
    role: RoleDisplayData;
    records: TrialPlaceRecord[];
    onDeleteRecord?: (recordId: string) => void;
    onConfirmRecord?: (record: TrialPlaceRecord) => void;
    /** 仅补录指定翻牌子笔的牌位，避免回写父轮的旧牌面。 */
    onUpdateDrawFlippedIndex?: (recordId: string, drawId: string, flippedIndex: number | null) => Promise<void> | void;
    /** 仅补录指定翻牌子笔的一张牌，不推断其他牌或核对状态。 */
    onUpdateDrawEquipment?: (recordId: string, drawId: string, cardIndex: number, equipmentId: string | null) => Promise<void> | void;
    /** 显式确认或撤销本次五张牌面的核对状态。 */
    onUpdateDrawEquipmentReview?: (recordId: string, draw: TrialDraw, reviewed: boolean) => Promise<void> | void;
}

const formatDate = (dateString: string | number) => {
    const timestamp = toTrialProgressTimestamp(dateString);
    if (timestamp === null || timestamp <= 0) return '时间待确认';
    const date = new Date(timestamp);
    return date.toLocaleDateString('zh-CN', {
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit'
    });
};

/** 父轮沿用分钟展示；翻牌收据需要保留秒，避免连续翻牌看起来像同一条。 */
const formatReceiptTime = (dateString: string | number) => {
    const value = typeof dateString === 'string' && /^\d+$/.test(dateString) ? Number(dateString) : dateString;
    const timestamp = toTrialProgressTimestamp(value);
    if (timestamp === null || timestamp <= 0) return '时间未记录';
    const date = new Date(timestamp);
    return date.toLocaleTimeString('zh-CN', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false,
    });
};

const ATTR_NAME_MAP: Record<string, string> = {
    'atVitalityBase': '体质',
    'atSpiritBase': '根骨',
    'atStrengthBase': '力道',
    'atAgilityBase': '身法',
    'atSpunkBase': '元气',
    'atPhysicsAttackPowerBase': '外功',
    'atMagicAttackPowerBase': '内功',
    'atSolarAttackPowerBase': '内功',
    'atLunarAttackPowerBase': '内功',
    'atPoisonAttackPowerBase': '内功',
    'atNeutralAttackPowerBase': '内功',
    'atPhysicsCriticalStrike': '会心',
    'atMagicCriticalStrike': '会心',
    'atAllTypeCriticalStrike': '会心',
    'atPhysicsCriticalDamagePowerBase': '会效',
    'atMagicCriticalDamagePowerBase': '会效',
    'atPhysicsOvercome': '破防',
    'atPhysicsOvercomeBase': '破防',
    'atMagicOvercome': '破防',
    'atMagicOvercomeBase': '破防',
    'atSolarOvercomeBase': '破防',
    'atLunarOvercomeBase': '破防',
    'atPoisonOvercomeBase': '破防',
    'atNeutralOvercomeBase': '破防',
    'atSurplusValueBase': '破招',
    'atStrainBase': '无双',
    'atHaste': '加速',
    'atToughnessBase': '御劲',
    'atDecriticalDamagePowerBase': '化劲',
    'atPhysicsShieldBase': '外防',
    'atMagicShield': '内防',
    'atDodge': '闪避',
    'atParry': '招架',
    'atHit': '命中',
    'atTherapyPowerBase': '治疗',
    'atSkillEventHandler': '特效',
};

const getFormattedAttributes = (item: JX3Equip, maxCount: number = 7) => {
    const attrs: { label: string; color?: string }[] = [];
    const seen = new Set<string>();

    const addAttr = (label: string, color?: string) => {
        if (label && !seen.has(label)) {
            seen.add(label);
            attrs.push({ label, color });
        }
    };

    for (let i = 1; i <= 16; i++) {
        const magicType = item[`_Magic${i}Type`] as JX3EquipMagicType | undefined;
        if (!magicType) continue;

        if (typeof magicType === 'object' && magicType.attr && Array.isArray(magicType.attr)) {
            const attrType = magicType.attr[0];
            if (attrType && typeof attrType === 'string') {
                const label = ATTR_NAME_MAP[attrType];
                if (label) {
                    addAttr(label, attrType === 'atSkillEventHandler' ? '#ffcc00' : undefined);
                }
            }
        } else if (typeof magicType === 'string' && magicType) {
            const label = ATTR_NAME_MAP[magicType];
            if (label) {
                addAttr(label, magicType === 'atSkillEventHandler' ? '#ffcc00' : undefined);
            }
        }
    }

    if (attrs.length === 0 && item.attributes && Array.isArray(item.attributes)) {
        item.attributes.forEach((attr) => {
            if (attr.color === 'green' && attr.type) {
                const label = ATTR_NAME_MAP[attr.type];
                if (label) {
                    addAttr(label, attr.type === 'atSkillEventHandler' ? '#ffcc00' : undefined);
                }
            }
        });
    }
    
    return attrs.slice(0, maxCount);
};

// 获取绑定类型标签
const getBindTypeLabel = (val: any) => {
    switch (Number(val)) {
        case 1: return '不绑定';
        case 2: return '装绑';
        case 3: return '拾绑';
        default: return null;
    }
};

export const TrialRoleRecordsModal: React.FC<TrialRoleRecordsModalProps> = ({
    isOpen,
    onClose,
    role,
    records,
    onDeleteRecord,
    onConfirmRecord,
    onUpdateDrawFlippedIndex,
    onUpdateDrawEquipment,
    onUpdateDrawEquipmentReview,
}) => {
    const [deletingRecordId, setDeletingRecordId] = useState<string | null>(null);
    // 时间范围切换：'week' 仅本周（默认，性能优先），'all' 全部历史记录
    const [timeRange, setTimeRange] = useState<'week' | 'all'>('week');
    // 分页：全部模式下记录可能很多，分批渲染避免一次性渲染大量复杂卡片
    const [displayLimit, setDisplayLimit] = useState(20);
    const [recordToDelete, setRecordToDelete] = useState<TrialPlaceRecord | null>(null);
    const [showConfirmDialog, setShowConfirmDialog] = useState(false);
    /** 完整本地目录只用于解析已保存的历史 ID，不受当前赛季选择范围影响。 */
    const [catalogEquipments, setCatalogEquipments] = useState<JX3Equip[]>([]);
    /** 当前赛季可选装备；与完整目录分离，避免历史记录显示丢失。 */
    const [equipments, setEquipments] = useState<JX3Equip[]>([]);
    const [equipmentCatalogState, setEquipmentCatalogState] = useState<EquipmentCatalogState>('loading');
    const [equipmentCatalogMessage, setEquipmentCatalogMessage] = useState<string | null>(null);
    const [editingDrawKey, setEditingDrawKey] = useState<string | null>(null);
    const [updatingDrawKey, setUpdatingDrawKey] = useState<string | null>(null);
    const [drawUpdateError, setDrawUpdateError] = useState<string | null>(null);
    // 只记录用户手动切换的状态；未操作过的自动记录按核对情况决定默认展开。
    const [expandedRecords, setExpandedRecords] = useState<Record<string, boolean>>({});

    // 详情展示及补录只读取本地目录，避免打开记录详情时产生网络请求或影响游戏。
    useEffect(() => {
        if (!isOpen) return;

        let cancelled = false;
        const getEquipmentType = (item: JX3Equip) => (
            Number(item.DetailType) === 6 || Number(item.SubType) === 1
                ? 'weapon'
                : [4, 5, 7].includes(Number(item.SubType)) ? 'trinket' : 'armor'
        );
        const parseEquipment = (item: unknown): JX3Equip | null => {
            try {
                const parsed: unknown = typeof item === 'string' ? JSON.parse(item) : item;
                if (!parsed || typeof parsed !== 'object') return null;
                const candidate = parsed as Record<string, unknown>;
                return typeof candidate.ID === 'number' && typeof candidate.Name === 'string'
                    ? parsed as JX3Equip
                    : null;
            } catch {
                return null;
            }
        };
        const loadEquipments = async () => {
            setEquipmentCatalogState('loading');
            setEquipmentCatalogMessage(null);
            setEquipments([]);
            setCatalogEquipments([]);
            try {
                const [storedResult, seasonResult] = await Promise.allSettled([
                    db.getEquipmentsStrict(),
                    db.getCurrentSeason(),
                ]);
                if (storedResult.status === 'rejected') throw storedResult.reason;
                const validStoredItems = storedResult.value.map(parseEquipment).filter((item): item is JX3Equip => item !== null);
                if (!cancelled) setCatalogEquipments(validStoredItems);
                if (seasonResult.status === 'rejected') throw seasonResult.reason;
                const season = seasonResult.value;
                const bounds = getTrialEquipmentLevelRange(season);
                if (!season || !bounds) {
                    throw new Error('当前赛季或试炼装备品级范围不可用，请关闭后重新打开记录窗口重试。');
                }
                const source = getTrialEquipmentSource(season);
                const matchesCurrentSeason = (item: JX3Equip) => (
                    isTrialEquipmentLevelInRange(item.Level, bounds)
                    && isTrialEquipmentFromSource(item, getEquipmentType(item), source)
                );
                const scopedItems = validStoredItems.filter(matchesCurrentSeason);

                if (cancelled) return;
                if (scopedItems.length === 0) {
                    setEquipments([]);
                    setEquipmentCatalogState('empty');
                    setEquipmentCatalogMessage('本地目录未找到当前赛季符合来源、部位和品级范围的试炼装备。');
                    return;
                }
                setEquipments(scopedItems.sort((left, right) => (
                    right.Level - left.Level
                    || (left.TypeLabel || '').localeCompare(right.TypeLabel || '', 'zh-CN')
                    || left.Name.localeCompare(right.Name, 'zh-CN')
                    || left.ID - right.ID
                )));
                setEquipmentCatalogState('ready');
            } catch (error) {
                if (!cancelled) {
                    setEquipmentCatalogState('error');
                    setEquipmentCatalogMessage(error instanceof Error ? error.message : '装备目录读取失败，请重试。');
                }
            }
        };
        void loadEquipments();
        return () => { cancelled = true; };
    }, [isOpen]);

    // 锁定背景滚动
    useEffect(() => {
        if (isOpen) {
            const originalOverflow = document.body.style.overflow;
            document.body.style.overflow = 'hidden';
            return () => {
                document.body.style.overflow = originalOverflow;
            };
        }
    }, [isOpen]);

    // 根据 ID 查找装备
    const findEquipmentById = (id: string | undefined): JX3Equip | null => {
        if (!id || !id.trim()) return null;
        
        const equip = catalogEquipments.find(e => e.ID?.toString() === id);
        if (equip) return equip;
        
        const suffix = id.match(/(?:^|_)(\d+)$/)?.[1];
        if (suffix) {
            const matches = catalogEquipments.filter(e => e.ID?.toString() === suffix);
            if (matches.length === 1) return matches[0];
        }
        
        return null;
    };

    const updateDrawFlippedIndex = async (recordId: string, draw: TrialDraw, flippedIndex: number | null) => {
        if (!onUpdateDrawFlippedIndex) return;
        const drawKey = `${recordId}-${draw.id}`;
        setUpdatingDrawKey(drawKey);
        setDrawUpdateError(null);
        try {
            await onUpdateDrawFlippedIndex(recordId, draw.id, flippedIndex);
            setEditingDrawKey(null);
        } catch (error) {
            console.error('补录翻牌位失败:', error);
            setDrawUpdateError('补录牌位失败，请重试');
        } finally {
            setUpdatingDrawKey(null);
        }
    };

    // Sort records specific to this role
    const weekInfo = useMemo(() => {
        const now = new Date();
        const weekStart = getLastMonday(now);
        const weekEnd = getNextMonday(now);
        return { start: weekStart, end: weekEnd };
    }, []);

    // 该角色的全部记录（按时间倒序）
    const allRoleRecords = useMemo(() => {
        return records
            .filter(r => r.roleId === role.id)
            .sort((a, b) => {
                const timeA = toTrialProgressTimestamp(a.date) ?? Number.NEGATIVE_INFINITY;
                const timeB = toTrialProgressTimestamp(b.date) ?? Number.NEGATIVE_INFINITY;
                return timeB - timeA;
            });
    }, [records, role.id]);

    // 按时间范围过滤后的记录
    const roleRecords = useMemo(() => {
        if (timeRange === 'all') {
            return allRoleRecords;
        }
        return allRoleRecords.filter(record => isTrialRecordInWeek(record, weekInfo.start, weekInfo.end));
    }, [allRoleRecords, timeRange, weekInfo]);

    // 分页：仅渲染前 displayLimit 条，避免一次性渲染大量复杂卡片导致卡顿
    const visibleRecords = useMemo(() => {
        return roleRecords.slice(0, displayLimit);
    }, [roleRecords, displayLimit]);

    // 切换时间范围时重置分页
    useEffect(() => {
        setDisplayLimit(20);
    }, [timeRange]);

    // 切换角色时重置时间范围为「本周」
    useEffect(() => {
        setTimeRange('week');
    }, [role.id]);


    const handleDeleteClick = (record: TrialPlaceRecord) => {
        setRecordToDelete(record);
        setShowConfirmDialog(true);
    };

    const handleDeleteConfirm = async () => {
        if (!recordToDelete || !onDeleteRecord) return;

        setDeletingRecordId(recordToDelete.id);
        setShowConfirmDialog(false); // Close dialog immediately

        // Simulate async delay if needed or just call
        // In a real app we might wait for promise, but here we just trigger
        onDeleteRecord(recordToDelete.id);

        // Reset local state
        setDeletingRecordId(null);
        setRecordToDelete(null);
    };

    if (!isOpen) return null;

    return createPortal(
        <div className="fixed inset-0 bg-ds-scrim backdrop-blur-sm flex items-center justify-center z-[100] p-4 overflow-hidden">
            <div role="dialog" aria-modal="true" aria-labelledby="trial-records-title" className="bg-surface border border-base rounded-2xl shadow-ds-modal w-full max-w-3xl max-h-[90vh] overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200">
                {/* Header - 对齐团本风格 */}
                <div className="px-6 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm flex-shrink-0">
                    <div className="min-w-0">
                        <h2 id="trial-records-title" className="text-lg font-bold text-main">试炼之地记录</h2>
                        <div className="flex items-center gap-1.5 text-xs text-muted mt-0.5">
                            <span className="font-medium text-main truncate">{role.name}@{role.server}</span>
                            {role.martial && (
                                <>
                                    <span className="text-muted/40 flex-shrink-0">·</span>
                                    <SectIcon sectName={role.martial} />
                                </>
                            )}
                        </div>
                    </div>
                    <button
                        onClick={onClose}
                        className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50 active:scale-95"
                        aria-label="关闭"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>


                {/* 时间范围切换 + 计数 */}
                <div className="px-4 pt-3 pb-2 flex items-center justify-between gap-2 flex-shrink-0">
                    <div className="inline-flex rounded-lg border border-base bg-base/30 p-0.5">
                        <button
                            type="button"
                            onClick={() => setTimeRange('week')}
                            aria-pressed={timeRange === 'week'}
                            className={`px-3 py-1 text-xs rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong ${timeRange === 'week'
                                ? 'bg-ds-success-soft text-ds-success-strong font-medium'
                                : 'text-muted hover:bg-surface hover:text-main'
                                }`}
                        >
                            本周
                        </button>
                        <button
                            type="button"
                            onClick={() => setTimeRange('all')}
                            aria-pressed={timeRange === 'all'}
                            className={`px-3 py-1 text-xs rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong ${timeRange === 'all'
                                ? 'bg-ds-success-soft text-ds-success-strong font-medium'
                                : 'text-muted hover:bg-surface hover:text-main'
                                }`}
                        >
                            全部
                        </button>
                    </div>
                    <span className="text-xs text-muted">
                        共 {roleRecords.length} 轮记录
                    </span>
                </div>

                {/* Records List */}
                <div className="flex-1 overflow-y-auto p-4 scroll-smooth">
                    {roleRecords.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-16 text-muted">
                            <div className="w-16 h-16 bg-base rounded-full flex items-center justify-center mb-4">
                                <Search className="w-8 h-8 text-muted/50" />
                            </div>
                            <p className="text-[1rem] font-medium mb-2">暂无记录</p>
                        </div>
                    ) : (
                        <div className="space-y-3">
                            {visibleRecords.map((record) => {
                                const isPending = record.status === 'pending';
                                const bossNames = record.bosses.map((boss, index) => boss || `第${index + 1}关待确认`);
                                const draws = Array.isArray(record.draws)
                                    ? [...record.draws].sort((left, right) => (
                                        Number(left.source === 'manual') - Number(right.source === 'manual')
                                        || left.sequence - right.sequence
                                    ))
                                    : [];
                                const receiptTimeCounts = draws.reduce((counts, draw) => {
                                    if (draw.source === 'manual' || !draw.receiptTime || draw.receiptTime === '0') return counts;
                                    const receiptTime = String(draw.receiptTime);
                                    counts.set(receiptTime, (counts.get(receiptTime) || 0) + 1);
                                    return counts;
                                }, new Map<string, number>());
                                const firstDrawIdByReceiptTime = draws.reduce((firstIds, draw) => {
                                    if (draw.source === 'manual' || !draw.receiptTime || draw.receiptTime === '0') return firstIds;
                                    const receiptTime = String(draw.receiptTime);
                                    if (!firstIds.has(receiptTime)) {
                                        firstIds.set(receiptTime, draw.id);
                                    }
                                    return firstIds;
                                }, new Map<string, string>());
                                const hasDraws = draws.length > 0;
                                const manualDrawCount = draws.filter(draw => draw.source === 'manual').length;
                                const autoDrawCount = draws.length - manualDrawCount;
                                const hasUnmatchedManualDraws = manualDrawCount > 0 && autoDrawCount > 0;
                                const canEditManualDraws = hasDraws && record.source === 'manual' && !record.sourceKey && autoDrawCount === 0;
                                const needsReview = trialRecordNeedsReview(record);
                                const hasAutomaticDraws = record.source === 'auto' || autoDrawCount > 0;
                                const isDrawsExpanded = expandedRecords[record.id] ?? (!hasAutomaticDraws || needsReview);
                                const hasKnownFlipPosition = record.flippedIndex >= 1 && record.flippedIndex <= 5;
                                return (
                                <div
                                    key={record.id}
                                    className={`p-4 sm:p-5 rounded-xl border transition-colors ${needsReview
                                        ? 'border-ds-warning bg-ds-warning-soft/40'
                                        : 'border-base bg-surface'
                                        }`}
                                    title={needsReview ? '本轮存在待确认信息' : undefined}
                                    aria-label={needsReview ? `${formatDate(record.date)} 的试炼记录，存在待确认信息` : undefined}
                                >
                                    <div>
                                        {needsReview && <span className="sr-only">本轮存在待确认信息</span>}
                                        <div className="min-w-0">
                                            {/* Line 1: Time, Layer, Flipped Index */}
                                            <div className="flex items-center gap-3 mb-2 flex-wrap">
                                                <div className="flex items-center gap-1.5 text-xs text-ds-fg-soft">
                                                    <Calendar className="w-3.5 h-3.5 flex-shrink-0" />
                                                    <span className="font-medium">{formatDate(record.date)}</span>
                                                </div>
                                                <div className="flex items-center gap-1" title="层数">
                                                    <Trophy className={`w-3.5 h-3.5 flex-shrink-0 ${record.layer > 0 ? 'text-muted' : 'text-ds-warning-strong'}`} />
                                                    <span className={`text-sm font-semibold ${record.layer > 0 ? 'text-main' : 'text-ds-warning-strong'}`}>
                                                        {record.layer > 0 ? `${record.layer} 层` : '层数待确认'}
                                                    </span>
                                                </div>
                                                {hasDraws ? (
                                                    <div className="flex items-center gap-1 rounded border border-base bg-base px-2 py-0.5 text-xs text-main">
                                                        {hasUnmatchedManualDraws ? <span>日志 {autoDrawCount} 次 · 手工 {manualDrawCount} 条待归属</span> : <><span>翻牌</span><span className="font-bold">{draws.length} 次</span></>}
                                                    </div>
                                                ) : (
                                                    <div className={`flex items-center gap-1 rounded border px-2 py-0.5 text-xs ${hasKnownFlipPosition
                                                        ? 'border-ds-success-soft bg-ds-success-soft text-ds-success-strong'
                                                        : 'border-ds-warning-soft bg-ds-warning-soft text-ds-warning-strong'}`}>
                                                        <span>{hasKnownFlipPosition ? '翻牌' : '翻牌位未知'}</span>
                                                        {hasKnownFlipPosition && <span className="font-bold">{record.flippedIndex}</span>}
                                                    </div>
                                                )}
                                                {record.source === 'auto' && !isPending && (
                                                    <div className="px-2 py-0.5 rounded border border-ds-success-soft bg-ds-success-soft text-xs font-medium text-ds-success-strong">
                                                        自动记录
                                                    </div>
                                                )}
                                                <div className="ml-auto flex items-center gap-1">
                                                    {onConfirmRecord && (
                                                        <button
                                                            onClick={() => onConfirmRecord(record)}
                                                            className={`inline-flex w-32 max-w-full items-center justify-center px-3 py-1.5 rounded-lg border text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary ${isPending
                                                                ? 'border-primary bg-primary text-primary-foreground hover:bg-primary-hover dark:bg-ds-accent dark:text-ds-fg-inverse dark:hover:brightness-110'
                                                                : 'border-base bg-surface text-main hover:bg-base'}`}
                                                        >
                                                            {canEditManualDraws ? '编辑手工记录' : hasDraws ? (isPending ? '补录通关信息' : '编辑通关信息') : (isPending ? '补录确认' : '编辑补录')}
                                                        </button>
                                                    )}
                                                    {onDeleteRecord && (
                                                        <button
                                                            onClick={() => handleDeleteClick(record)}
                                                            disabled={deletingRecordId === record.id}
                                                            className="p-2 text-muted hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-900/20 rounded-lg transition-colors active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-300 dark:focus-visible:ring-red-800"
                                                            title="删除记录"
                                                            aria-label="删除记录"
                                                        >
                                                            {deletingRecordId === record.id ? (
                                                                <Loader2 className="w-4 h-4 animate-spin" />
                                                            ) : (
                                                                <Trash2 className="w-4 h-4" />
                                                            )}
                                                        </button>
                                                    )}
                                                </div>
                                            </div>

                                            {/* Line 2: Boss */}
                                            <div className="mb-2">
                                                <div className="flex items-center gap-1.5 text-xs text-ds-fg-soft bg-base/50 p-2 rounded-lg">
                                                    <span className="font-medium flex-shrink-0">Boss:</span>
                                                    <span className="flex-1 break-words">{bossNames.length > 0 ? bossNames.join(', ') : 'Boss 待确认'}</span>
                                                </div>
                                            </div>

                                            {hasDraws && (
                                                <div className="mb-2 space-y-3">
                                                    <button type="button" aria-expanded={isDrawsExpanded}
                                                        onClick={() => setExpandedRecords(current => ({ ...current, [record.id]: !isDrawsExpanded }))}
                                                        className="flex w-full items-center justify-between gap-2 border-t border-base pt-3 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong">
                                                        <span className="text-sm font-semibold text-main">翻牌记录</span>
                                                        <span className="flex items-center gap-1 text-xs text-muted">{hasUnmatchedManualDraws ? `日志 ${autoDrawCount} 次 · 手工 ${manualDrawCount} 条待归属` : `共 ${draws.length} 次`}<ChevronDown className={`h-4 w-4 transition-transform ${isDrawsExpanded ? 'rotate-180' : ''}`} /></span>
                                                    </button>
                                                    {isDrawsExpanded && hasUnmatchedManualDraws && <p className="rounded border border-ds-warning-soft bg-ds-warning-soft px-3 py-2 text-xs text-ds-warning-strong">手工翻牌与日志记录可能重复，暂无法确定对应关系；手工明细已保留，统计暂以日志记录为准。</p>}
                                                    {isDrawsExpanded && draws.map((draw) => {
                                                        const drawKey = `${record.id}-${draw.id}`;
                                                        const isEditing = editingDrawKey === drawKey;
                                                        const isUpdating = updatingDrawKey === drawKey;
                                                        const knownPosition = draw.flippedIndex >= 1 && draw.flippedIndex <= 5;
                                                        const sameSecondReceipts = draw.source !== 'manual' && Boolean(draw.receiptTime) && draw.receiptTime !== '0'
                                                            && (Boolean(draw.sequenceAmbiguous) || (receiptTimeCounts.get(String(draw.receiptTime)) || 0) > 1);
                                                        const showSameSecondGroupHeading = sameSecondReceipts
                                                            && firstDrawIdByReceiptTime.get(String(draw.receiptTime)) === draw.id;
                                                        const rewardItems = Array.isArray(draw.rewardItems) ? draw.rewardItems : [];
                                                        const { flipRewards, hasObservedRewards } = classifyTrialRewards(rewardItems);
                                                        const staminaCosts = Array.isArray(draw.staminaCosts)
                                                            ? draw.staminaCosts.filter(cost => Number.isFinite(cost) && cost > 0)
                                                            : [];
                                                        const inferredStaminaCost = staminaCosts.reduce((sum, cost) => sum + cost, 0);
                                                        const totalStaminaCost = typeof draw.staminaCost === 'number' && draw.staminaCost > 0
                                                            ? draw.staminaCost
                                                            : inferredStaminaCost;
                                                        const staminaText = totalStaminaCost > 0
                                                            ? `精力 ${totalStaminaCost.toLocaleString('zh-CN')}${staminaCosts.length > 1 ? `（${staminaCosts.join(' + ')}）` : ''}`
                                                            : '精力待确认';
                                                        const cardEntries = [1, 2, 3, 4, 5]
                                                            .map(cardIndex => {
                                                                const equipId = draw[`card${cardIndex}` as keyof Pick<TrialDraw, 'card1' | 'card2' | 'card3' | 'card4' | 'card5'>];
                                                                return equipId ? { cardIndex, equip: findEquipmentById(equipId) } : null;
                                                            })
                                                            .filter((entry): entry is { cardIndex: number; equip: JX3Equip | null } => entry !== null);

                                                        return (
                                                            <React.Fragment key={draw.id}>
                                                            {showSameSecondGroupHeading && (
                                                                <div className="rounded-lg border border-ds-warning-soft bg-ds-warning-soft px-3 py-1.5 text-xs text-ds-warning-strong">
                                                                    同秒收据（{formatReceiptTime(draw.receiptTime)}），先后待确认
                                                                </div>
                                                            )}
                                                            <div className="rounded-lg border border-base bg-base/30 p-3 text-sm">
                                                                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                                                                    <span className="font-medium text-main">
                                                                        {hasUnmatchedManualDraws && draw.source === 'manual' ? '手工翻牌（待归属）' : sameSecondReceipts ? '同秒翻牌收据' : `第 ${draw.sequence} 次翻牌`}
                                                                    </span>
                                                                    <span className="text-ds-fg-soft">{formatReceiptTime(draw.receiptTime)}</span>
                                                                    <span className={`rounded px-2 py-0.5 text-xs font-medium ${knownPosition ? 'bg-ds-success-soft text-ds-success-strong' : 'bg-ds-warning-soft text-ds-warning-strong'}`}>
                                                                        {knownPosition ? `第 ${draw.flippedIndex} 张` : '牌位未知'}
                                                                    </span>
                                                                    <span className="text-xs text-ds-fg-soft">{staminaText}</span>
                                                                    {draw.equipmentUnreviewed && (
                                                                        <span className="rounded border border-ds-warning-soft bg-ds-warning-soft px-2 py-0.5 text-xs font-medium text-ds-warning-strong">装备待确认</span>
                                                                    )}
                                                                    {onUpdateDrawFlippedIndex && (
                                                                        <button
                                                                            type="button"
                                                                            disabled={isUpdating}
                                                                            onClick={() => {
                                                                                setDrawUpdateError(null);
                                                                                setEditingDrawKey(isEditing ? null : drawKey);
                                                                            }}
                                                                            className={`sm:ml-auto inline-flex w-32 max-w-full items-center justify-center rounded border px-3 py-1.5 text-xs font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong disabled:cursor-not-allowed disabled:opacity-60 ${!isEditing && !knownPosition
                                                                                ? 'border-ds-success-soft bg-ds-success-soft text-ds-success-strong hover:bg-ds-success-soft/70'
                                                                                : 'border-base bg-surface text-main hover:bg-base'}`}
                                                                        >
                                                                            {isUpdating ? '保存中…' : isEditing ? '收起补录' : knownPosition ? '修改牌位' : '补录牌位'}
                                                                        </button>
                                                                    )}
                                                                </div>
                                                                <div className="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-ds-fg-soft">
                                                                    <span className="font-medium">翻牌奖励：</span>
                                                                    {flipRewards.length > 0
                                                                        ? flipRewards.map((item, itemIndex) => <span key={`${draw.id}-flip-${itemIndex}`} className="rounded border border-base bg-surface px-1.5 py-0.5">{item.name} × {item.quantity}</span>)
                                                                        : <span>{hasObservedRewards ? '未识别到翻牌奖励' : '待确认'}</span>}
                                                                </div>
                                                                {onUpdateDrawEquipment && (
                                                                    <TrialDrawEquipmentEditor
                                                                        draw={draw}
                                                                        equipments={equipments}
                                                                        catalogState={equipmentCatalogState}
                                                                        catalogMessage={equipmentCatalogMessage}
                                                                        findEquipmentById={findEquipmentById}
                                                                        onUpdateEquipment={(cardIndex, equipmentId) => (
                                                                            onUpdateDrawEquipment(record.id, draw.id, cardIndex, equipmentId)
                                                                        )}
                                                                        onUpdateEquipmentReview={onUpdateDrawEquipmentReview ? (reviewed => onUpdateDrawEquipmentReview(record.id, draw, reviewed)) : undefined}
                                                                    />
                                                                )}
                                                                {!onUpdateDrawEquipment && cardEntries.length > 0 && (
                                                                    <div className="mt-1.5 flex flex-wrap gap-x-2 gap-y-1 text-ds-fg-soft">
                                                                        <span className="font-medium">已识别装备：</span>
                                                                        {cardEntries.map(({ cardIndex, equip }) => (
                                                                            <span key={`${draw.id}-card-${cardIndex}`} className={cardIndex === draw.flippedIndex ? 'text-ds-success-strong' : undefined}>
                                                                                卡 {cardIndex}：{equip?.Name || (equipmentCatalogState === 'loading' ? '装备目录加载中' : '装备目录未找到')}
                                                                            </span>
                                                                        ))}
                                                                    </div>
                                                                )}
                                                                {isEditing && (
                                                                    <div className="mt-2 flex flex-wrap items-center gap-1.5 border-t border-base/60 pt-2">
                                                                        <span className="mr-1 text-muted">本次牌位：</span>
                                                                        {[1, 2, 3, 4, 5].map(position => (
                                                                            <button
                                                                                key={`${draw.id}-position-${position}`}
                                                                                type="button"
                                                                                disabled={isUpdating}
                                                                                onClick={() => updateDrawFlippedIndex(record.id, draw, draw.flippedIndex === position ? null : position)}
                                                                                aria-pressed={draw.flippedIndex === position}
                                                                                title={draw.flippedIndex === position ? `再次点击取消第 ${position} 张牌位` : `标记为第 ${position} 张牌位`}
                                                                                className={`rounded border px-2 py-1 text-[11px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong disabled:cursor-not-allowed disabled:opacity-60 ${draw.flippedIndex === position
                                                                                    ? 'border-ds-success-soft bg-ds-success-soft text-ds-success-strong'
                                                                                    : 'border-base bg-surface text-main hover:bg-base'}`}
                                                                            >
                                                                                第 {position} 张
                                                                            </button>
                                                                        ))}
                                                                    </div>
                                                                )}
                                                            </div>
                                                            </React.Fragment>
                                                        );
                                                    })}
                                                    {drawUpdateError && (
                                                        <div className="rounded border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-600 dark:border-red-900/40 dark:bg-red-900/20 dark:text-red-300">
                                                            {drawUpdateError}
                                                        </div>
                                                    )}
                                                </div>
                                            )}

                                            {/* Line 3: Cards Info (Equipment) */}
                                            {!hasDraws && <div className="text-xs">
                                                {/* Dropped Equipment */}
                                                {(() => {
                                                    const recordedEquipmentEntries = getTrialRecordEquipmentEntries(record)
                                                        .map(entry => ({
                                                            ...entry,
                                                            equip: findEquipmentById(entry.equipId)
                                                        }));
                                                    const equipmentsToDisplay = recordedEquipmentEntries
                                                        .filter((entry): entry is {
                                                            cardIndex: number;
                                                            equipId: string;
                                                            isFlipped: boolean;
                                                            equip: JX3Equip;
                                                        } => Boolean(entry.equip));

                                                    if (recordedEquipmentEntries.length === 0) return (
                                                        <div className="flex items-center gap-2 text-muted px-3 py-2 bg-base/30 rounded-lg border border-base/50 italic">
                                                            {record.equipmentUnreviewed
                                                                ? '尚未识别到装备，不能据此判断本轮无装备'
                                                                : '未获得有效装备/物品'}
                                                        </div>
                                                    );

                                                    const missingEquipmentEntries = recordedEquipmentEntries.filter(entry => !entry.equip);

                                                    return (
                                                        <div className="space-y-2">
                                                            {record.equipmentUnreviewed && (
                                                                <div className="flex items-center gap-2 text-ds-warning-strong px-3 py-2 bg-ds-warning-soft rounded-lg border border-ds-warning-soft">
                                                                    已显示画面识别到的装备。未观察的牌仍待核对，不计入翻中率、位置出现率或 Boss 出装备统计。
                                                                </div>
                                                            )}
                                                            {missingEquipmentEntries.map(entry => (
                                                                <div key={`${record.id}-missing-${entry.cardIndex}`} className="flex items-center gap-2 rounded-lg border border-ds-warning-soft bg-ds-warning-soft px-3 py-2 text-xs text-ds-warning-strong">
                                                                    卡 {entry.cardIndex} 已记录装备 ID「{entry.equipId}」，{equipmentCatalogState === 'loading'
                                                                        ? '装备目录加载中'
                                                                        : equipmentCatalogState === 'error'
                                                                            ? '装备目录读取失败，已保留原始识别结果'
                                                                            : equipmentCatalogState === 'empty'
                                                                                ? '装备目录为空，已保留原始识别结果'
                                                                                : '当前目录未找到，已保留原始识别结果'}。
                                                                </div>
                                                            ))}
                                                            {equipmentsToDisplay.map(({ equip, cardIndex, isFlipped }, index) => {
                                                                const attrs = getFormattedAttributes(equip);
                                                                const iconId = equip._IconID ?? equip.IconID;
                                                                const iconUrl = iconId ? `https://icon.jx3box.com/icon/${iconId}.png` : null;
                                                                const bindLabel = getBindTypeLabel(equip.BindType);

                                                                return (
                                                                    <div key={`${record.id}-${equip.ID || index}`} className="w-full flex items-center gap-3 px-3 py-2 bg-base/50 rounded-lg border border-base/50">
                                                                        <div className="flex flex-col items-center gap-1 flex-shrink-0">
                                                                            {isFlipped ? (
                                                                                <CheckCircle className="w-4 h-4 text-ds-success-strong" />
                                                                            ) : (
                                                                                <Circle className="w-4 h-4 text-muted" />
                                                                            )}
                                                                            <span className="text-[10px] text-muted whitespace-nowrap">卡{cardIndex}</span>
                                                                        </div>
                                                                        {iconUrl && (
                                                                            <div className="w-8 h-8 rounded border border-base bg-base overflow-hidden flex-shrink-0">
                                                                                <img src={iconUrl} alt="" className="w-full h-full object-cover" />
                                                                            </div>
                                                                        )}
                                                                        <div className="flex flex-col min-w-0 flex-1">
                                                                            <div className="flex items-center gap-2 mb-0.5 flex-wrap">
                                                                                <span className={`font-medium truncate text-sm ${isFlipped
                                                                                    ? 'text-ds-success-strong'
                                                                                    : 'text-main'
                                                                                    }`}>
                                                                                    {equip.Name}
                                                                                </span>
                                                                                <span className={`px-1.5 py-0.5 rounded text-[10px] border ${isFlipped
                                                                                    ? 'bg-ds-success-soft text-ds-success-strong border-ds-success-soft dark:bg-ds-success-soft/20 dark:text-ds-success-strong dark:border-ds-success-soft'
                                                                                    : 'bg-base text-muted border-base'
                                                                                    }`}>
                                                                                    {isFlipped
                                                                                        ? '已翻到'
                                                                                        : record.equipmentUnreviewed
                                                                                            ? '画面识别到'
                                                                                            : '未翻到'}
                                                                                </span>
                                                                            </div>
                                                                            <div className="flex items-center gap-1.5 text-xs text-muted mb-0.5 flex-wrap">
                                                                                {bindLabel && <span className="text-ds-warning-strong">{bindLabel}</span>}
                                                                                <span>品级 {equip.Level}</span>
                                                                            </div>
                                                                            {attrs.length > 0 && (
                                                                                <div className="flex flex-wrap gap-1">
                                                                                    {attrs.map((a, i) => (
                                                                                        <span
                                                                                            key={i}
                                                                                            className="px-1.5 py-0.5 rounded bg-white/50 dark:bg-black/20 text-[10px] border border-black/5 dark:border-white/5"
                                                                                            style={a.color ? { color: a.color } : undefined}
                                                                                        >
                                                                                            {a.label}
                                                                                        </span>
                                                                                    ))}
                                                                                </div>
                                                                            )}
                                                                        </div>
                                                                    </div>
                                                                );
                                                            })}
                                                        </div>
                                                    );
                                                })()}
                                            </div>}
                                        </div>

                                    </div>
                                </div>
                                );
                            })}
                            {visibleRecords.length < roleRecords.length && (
                                <button
                                    type="button"
                                    onClick={() => setDisplayLimit(prev => prev + 20)}
                                    className="w-full py-2.5 mt-1 text-xs text-muted hover:text-main border border-base rounded-lg hover:bg-surface transition-colors"
                                >
                                    加载更多（剩 {roleRecords.length - visibleRecords.length} 条）
                                </button>
                            )}
                        </div>
                    )}
                </div>

                {/* Footer Close Button */}
                <div className="bg-base px-5 py-3 border-t border-base flex-shrink-0">
                    <button
                        onClick={onClose}
                        className="w-full py-2.5 border border-base text-main rounded-xl font-medium hover:bg-surface active:bg-base transition-all duration-200 flex items-center justify-center gap-2 text-sm"
                    >
                        关闭
                    </button>
                </div>
            </div>

            {/* Delete Confirmation Dialog */}
            {showConfirmDialog && (
                <div
                    className="fixed inset-0 bg-ds-scrim backdrop-blur-sm flex items-center justify-center z-[110] animate-in fade-in duration-200"
                    onClick={() => {
                        setShowConfirmDialog(false);
                        setRecordToDelete(null);
                    }}
                >
                    <div
                        className="bg-surface rounded-2xl shadow-ds-modal max-w-sm w-full mx-4 border border-base animate-in zoom-in-95 duration-200"
                        onClick={(e) => e.stopPropagation()}
                    >
                        {/* Header */}
                        <div className="p-5 pb-0">
                            <div className="flex items-start gap-4">
                                <div className="w-12 h-12 bg-red-100 dark:bg-red-900/30 rounded-xl flex items-center justify-center flex-shrink-0 border border-red-200 dark:border-red-800">
                                    <AlertCircle className="w-6 h-6 text-red-600 dark:text-red-400" />
                                </div>
                                <div className="flex-1 min-w-0">
                                    <h3 className="text-lg font-bold text-main">确认删除</h3>
                                    <p className="text-sm text-muted mt-1">
                                        确定要删除这条 <span className="font-semibold text-main">{recordToDelete?.layer}层</span> 的试炼记录吗？
                                    </p>
                                </div>
                            </div>
                        </div>

                        {/* Record Preview */}
                        {recordToDelete && (
                            <div className="px-5 py-3">
                                <div className="bg-base/50 rounded-xl p-3 border border-base text-sm">
                                    <div className="flex items-center gap-2 text-muted">
                                        <Calendar className="w-3.5 h-3.5" />
                                        <span>{new Date(recordToDelete.date).toLocaleDateString()}</span>
                                        <span className="mx-1">·</span>
                                        <span>Boss: {recordToDelete.bosses.join(', ')}</span>
                                    </div>
                                </div>
                            </div>
                        )}

                        {/* Warning */}
                        <div className="px-5 pb-4">
                            <p className="text-xs text-ds-warning-strong dark:text-ds-warning-strong flex items-center gap-1.5">
                                <AlertCircle className="w-3.5 h-3.5" />
                                此操作不可恢复
                            </p>
                        </div>

                        {/* Actions */}
                        <div className="px-5 py-4 bg-base/50 border-t border-base rounded-b-2xl flex justify-end gap-3">
                            <button
                                onClick={() => {
                                    setShowConfirmDialog(false);
                                    setRecordToDelete(null);
                                }}
                                className="px-4 py-2.5 border border-base text-main rounded-xl hover:bg-surface active:bg-base transition-all duration-200 text-sm font-medium cursor-pointer"
                            >
                                取消
                            </button>
                            <button
                                onClick={handleDeleteConfirm}
                                className="px-4 py-2.5 bg-red-600 hover:bg-red-700 active:bg-red-800 text-white rounded-xl transition-all duration-200 text-sm font-medium flex items-center gap-2 cursor-pointer shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-300 dark:focus-visible:ring-red-800"
                            >
                                <Trash2 className="w-4 h-4" />
                                确认删除
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>,
        document.body
    );
};
