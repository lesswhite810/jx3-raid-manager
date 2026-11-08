
import React, { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { Account, TrialDraw, TrialPlaceRecord } from '../types';
import { Package, Check, X, AlertCircle, Layers, Trophy, Save, HelpCircle } from 'lucide-react';
import { TrialEquipmentCatalogState, TrialEquipmentPicker } from './TrialEquipmentPicker';
import { generateUUID } from '../utils/uuid';
import { toast } from '../utils/toastManager';
import { getTrialEquipments, JX3Equip, JX3EquipMagicType } from '../services/jx3BoxApi';
import { db } from '../services/db';
import { DateTimePicker } from './DateTimePicker';
import { getBaseServerName } from '../utils/serverUtils';
import { findTrialBossEquipmentStats } from '../utils/trialFlipStats';
import {
    deriveTrialEquipmentLevelRange,
    getTrialEquipmentLevelRange,
    isTrialEquipmentLevelInRange,
} from '../utils/trialRecordUtils';
import { getLatestTrialRecordForRole, getTrialLayerForRole, toTrialProgressTimestamp, TrialProgressPeriod } from '../utils/trialProgress';
import { getLastMonday } from '../utils/cooldownManager';
import { getTrialDrawEquipmentAttributes, getTrialDrawEquipmentSlot } from '../utils/trialDrawEquipmentFilters';

interface RoleWithStatus {
    id: string;
    name: string;
    server: string;
    region: string;
    accountId: string;
    accountName: string;
    sect: string;
}

interface AddTrialRecordModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (record: TrialPlaceRecord, options?: { preserveFlippedIndex?: boolean }) => Promise<void> | void;
    accounts: Account[];
    initialRole?: RoleWithStatus;
    /** 自动扫描的待确认记录会以原始 ID 打开，确认后在原行补齐。 */
    initialRecord?: TrialPlaceRecord;
    autoFillEnabled?: boolean;
    records?: TrialPlaceRecord[];
    trialProgressPeriod?: TrialProgressPeriod | null;
}

const BOSS_LEVEL_1 = ['寅', '雷神', '子'];
const BOSS_LEVEL_2_3 = ['叶冬辰', '韦柔丝', '濯尘', '藤原樱奈', '方鹤影', '苏髓'];

interface ManualTrialDrawDraft {
    id: string;
    flippedIndex: number;
    cardItems: Record<number, string>;
    equipmentUnreviewed: boolean;
    /** 仅本次提交使用：用户明确选择或清空的牌位，不能被旧自动观察覆盖。 */
    editedCardIndices: number[];
}

const createManualTrialDrawDraft = (): ManualTrialDrawDraft => ({
    id: generateUUID(),
    // 手工新建没有画面或日志证据；未选择牌位、未核对五张牌前不得进入装备统计。
    flippedIndex: 0,
    cardItems: { 1: '', 2: '', 3: '', 4: '', 5: '' },
    equipmentUnreviewed: true,
    editedCardIndices: [],
});

// --- Helper Functions & Components ---

export const ATTR_NAME_MAP: Record<string, string> = {
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
    'atAllTypeCriticalDamagePowerBase': '会效',
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

export const getFormattedAttributes = (item: JX3Equip, maxCount: number = 7) => {
    const attrs: { label: string, color?: string }[] = [];
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

export const hasAttribute = (item: JX3Equip, keyword: string) => {
    const attrs = getFormattedAttributes(item);
    return attrs.some(a => a.label.includes(keyword));
};

/**
 * Reusable Equipment Display Component ensures consistency
 * Used in both the list and the card
 */
export const EquipDisplay: React.FC<{ item: JX3Equip, simple?: boolean }> = ({ item, simple }) => {
    const attributes = getFormattedAttributes(item);
    const iconId = item._IconID ?? item.IconID;
    const iconUrl = iconId ? `https://icon.jx3box.com/icon/${iconId}.png` : null;

    const getBindTypeLabel = (val: any) => {
        switch (Number(val)) {
            case 1: return '不绑定';
            case 2: return '装备后绑定';
            case 3: return '拾取后绑定';
            default: return null;
        }
    };

    const bindLabel = getBindTypeLabel(item.BindType);

    if (simple) {

        // Redesigned Card Mode - Vertical Stack
        return (
            <div className="flex flex-col h-full w-full overflow-hidden items-center text-center py-2 px-1 bg-transparent">
                {/* Top: Icon */}
                <div className="flex-shrink-0 mb-1">
                    {iconUrl ? (
                        <div className="w-11 h-11 rounded border border-gray-600 bg-[#1a1a2e] overflow-hidden shadow-sm">
                            <img src={iconUrl} alt={item.Name} className="w-full h-full object-cover" />
                        </div>
                    ) : (
                        <div className="w-11 h-11 rounded border border-gray-600 bg-[#1a1a2e] flex items-center justify-center">
                            <span className="text-[10px] text-gray-500">?</span>
                        </div>
                    )}
                </div>

                {/* Middle: Content Block - Name Top, Info Bottom */}
                <div className="w-full flex-1 flex flex-col justify-center min-h-0 gap-0.5 px-0.5">
                    {/* Row 1: Name */}
                    <div className="w-full font-bold text-xs text-[#fe2dfe] leading-tight text-left line-clamp-2" title={item.Name}>
                        {item.Name}
                    </div>

                    {/* Row 2: Bind Type + Level (Flex Between) */}
                    <div className="w-full flex justify-between items-center mt-0.5">
                        {/* Left: Bind Type */}
                        <div className="text-[9px] text-gray-500 opacity-90 leading-none transform scale-95 origin-left whitespace-nowrap">
                            {bindLabel || ''}
                        </div>

                        {/* Right: Level */}
                        <div className="text-[10px] font-medium text-main opacity-90 leading-none whitespace-nowrap">
                            {item.Level}品
                        </div>
                    </div>
                </div>

                <div className="w-full mt-1.5 pt-1 border-t border-white/5 flex-shrink-0">
                    <div className="flex flex-wrap gap-x-1.5 gap-y-0.5 w-full px-0.5">
                        {attributes.length > 0 ? (
                            attributes.map((attr, idx) => (
                                <span key={idx} className="text-[10px] leading-tight whitespace-nowrap" style={attr.color ? { color: attr.color } : {}}>
                                    {attr.label}
                                </span>
                            ))
                        ) : (
                            <span className="text-[10px] text-muted">无属性</span>
                        )}
                    </div>
                </div>
            </div>
        );
    }

    // List Mode (Dropdown) - Detailed with Icon
    return (
        <div className="flex items-start gap-2 w-full">
            {/* Icon */}
            {iconUrl && (
                <div className="w-10 h-10 rounded border border-gray-600 bg-[#1a1a2e] flex-shrink-0 overflow-hidden">
                    <img src={iconUrl} alt={item.Name} className="w-full h-full object-cover" loading="lazy" />
                </div>
            )}

            <div className="flex-1 min-w-0 py-0.5">
                <div className="flex justify-between items-center">
                    <span className="font-bold text-sm text-[#fe2dfe] truncate">
                        {item.Name}
                    </span>
                    <div className="flex items-center gap-2 flex-shrink-0 ml-2">
                        {bindLabel && (
                            <span className="text-xs text-gray-500">{bindLabel}</span>
                        )}
                        <span className="text-sm font-medium text-main">
                            {item.Level}品
                        </span>
                    </div>
                </div>

                <div className="flex flex-wrap gap-x-2 text-xs mt-1">
                    {attributes.length > 0 ? (
                        attributes.map((attr, idx) => (
                            <span
                                key={idx}
                                style={attr.color ? { color: attr.color } : {}}
                                className={!attr.color ? "text-[#00bcd4]" : ""}
                            >
                                {attr.label}
                            </span>
                        ))
                    ) : (
                        <span className="text-muted">无属性信息</span>
                    )}
                </div>
            </div>
        </div>
    );
};


export const AddTrialRecordModal: React.FC<AddTrialRecordModalProps> = ({
    isOpen,
    onClose,
    onSubmit,
    accounts,
    initialRole,
    initialRecord,
    autoFillEnabled = false,
    records = [],
    trialProgressPeriod = null
}) => {
    // 自动记录的多次翻牌已各自落在子笔中；父轮表单不可将其误写回旧的单翻牌字段。
    const hasRecordedDraws = (initialRecord?.draws?.length ?? 0) > 0;
    // 仅纯手工记录可维护翻牌明细。自动来源或已被自动证据认领的手工父记录
    // 仍只允许编辑通关信息，避免覆盖已关联的日志、精力和奖励。
    const hasUnownedOrAutomaticDraw = Boolean(initialRecord?.draws?.some(draw => draw.source !== 'manual'));
    const canEditManualDraws = !initialRecord || (
        (initialRecord.source ?? 'manual') === 'manual'
        && !initialRecord.sourceKey
        && !hasUnownedOrAutomaticDraw
    );
    const hasProtectedDraws = hasRecordedDraws && !canEditManualDraws;
    const [selectedRoleId, setSelectedRoleId] = useState<string>(initialRole?.id || '');
    const [layer, setLayer] = useState<number>(1);
    const [boss1, setBoss1] = useState<string>('');
    const [boss2, setBoss2] = useState<string>('');
    const [boss3, setBoss3] = useState<string>('');

    // 5 Cards State
    // 0 表示日志未提供翻牌位，不应推断为 1 号位。
    const [flipIndex, setFlipIndex] = useState<number>(1);
    const [hasExplicitFlipIndexChange, setHasExplicitFlipIndexChange] = useState(false);
    const [cardItems, setCardItems] = useState<Record<number, string>>({
        1: '', 2: '', 3: '', 4: '', 5: ''
    });
    // 自动识别可能在编辑弹窗打开后才落库。只将用户实际动过的牌位写回，
    // 其余空牌位交给后端保留晚到的识别结果。
    const [editedCardIndices, setEditedCardIndices] = useState<Set<number>>(new Set());
    const [equipmentUnreviewed, setEquipmentUnreviewed] = useState(false);
    const [selectedCard, setSelectedCard] = useState<number | null>(null);
    // 手工新增时可在同一轮下录入多次翻牌。自动记录已有子笔时，这个表单
    // 只编辑通关信息，不能把手工草稿覆盖到自动明细。
    const [manualDraws, setManualDraws] = useState<ManualTrialDrawDraft[]>(() => [createManualTrialDrawDraft()]);
    const [activeManualDrawIndex, setActiveManualDrawIndex] = useState(0);

    // Equipment selector state
    const [showEquipSelector, setShowEquipSelector] = useState(false);
    const [allEquipments, setAllEquipments] = useState<JX3Equip[]>([]);
    const [isLoadingEquipments, setIsLoadingEquipments] = useState(false);
    const [equipmentRangeMessage, setEquipmentRangeMessage] = useState<string | null>(null);
    const [equipmentCatalogState, setEquipmentCatalogState] = useState<TrialEquipmentCatalogState>('loading');


    const [notes, setNotes] = useState<string>('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [recordDate, setRecordDate] = useState<string>('');
    const hasInitializedOpenDraft = useRef(false);

    // 将日期转换为输入框显示用的字符串，支持时间戳和ISO字符串
    const formatDateForInput = (date: Date | string | number): string => {
        const d = date instanceof Date ? date : new Date(toTrialProgressTimestamp(date) ?? NaN);
        const year = d.getFullYear();
        const month = String(d.getMonth() + 1).padStart(2, '0');
        const day = String(d.getDate()).padStart(2, '0');
        const hours = String(d.getHours()).padStart(2, '0');
        const minutes = String(d.getMinutes()).padStart(2, '0');
        return `${year}-${month}-${day}T${hours}:${minutes}`;
    };

    // 将日期字符串转换为时间戳存储
    const formatDateFromInput = (dateStr: string): number => {
        if (!dateStr) return Date.now();
        return new Date(dateStr).getTime();
    };

    const applyRoleProgress = useCallback((roleId: string) => {
        setBoss1('');
        setBoss2('');
        setBoss3('');
        setLayer(getTrialLayerForRole(records, roleId, trialProgressPeriod));

        if (!autoFillEnabled || !roleId) return;

        const latestWeeklyRecord = getLatestTrialRecordForRole(
            records,
            roleId,
            trialProgressPeriod,
            getLastMonday(new Date()).getTime()
        );
        if (!latestWeeklyRecord) return;

        setLayer(latestWeeklyRecord.layer);
        if (Array.isArray(latestWeeklyRecord.bosses) && latestWeeklyRecord.bosses.length >= 3) {
            setBoss1(latestWeeklyRecord.bosses[0]);
            setBoss2(latestWeeklyRecord.bosses[1]);
            setBoss3(latestWeeklyRecord.bosses[2]);
        }
    }, [autoFillEnabled, records, trialProgressPeriod]);

    // 只在每次打开时初始化一次；records 或异步元数据更新不能覆盖用户正在编辑的草稿。
    useEffect(() => {
        if (!isOpen) {
            hasInitializedOpenDraft.current = false;
            return;
        }
        if (hasInitializedOpenDraft.current) return;

        hasInitializedOpenDraft.current = true;
        const roleId = initialRecord?.roleId || initialRole?.id || '';
        setSelectedRoleId(roleId);
        if (initialRecord) {
            setLayer(initialRecord.layer);
            setBoss1(initialRecord.bosses[0] || '');
            setBoss2(initialRecord.bosses[1] || '');
            setBoss3(initialRecord.bosses[2] || '');
            setFlipIndex(initialRecord.flippedIndex >= 1 && initialRecord.flippedIndex <= 5 ? initialRecord.flippedIndex : 0);
            setEquipmentUnreviewed(Boolean(initialRecord.equipmentUnreviewed));
            setCardItems({
                1: initialRecord.card1 || '',
                2: initialRecord.card2 || '',
                3: initialRecord.card3 || '',
                4: initialRecord.card4 || '',
                5: initialRecord.card5 || '',
            });
            if ((initialRecord.source ?? 'manual') === 'manual' && !initialRecord.sourceKey) {
                const savedDraws = Array.isArray(initialRecord.draws) && initialRecord.draws.length > 0
                    ? initialRecord.draws.map(draw => ({
                        id: draw.id,
                        flippedIndex: draw.flippedIndex,
                        cardItems: {
                            1: draw.card1 || '', 2: draw.card2 || '', 3: draw.card3 || '',
                            4: draw.card4 || '', 5: draw.card5 || '',
                        },
                        equipmentUnreviewed: Boolean(draw.equipmentUnreviewed),
                        editedCardIndices: draw.editedCardIndices || [],
                    }))
                    : [{
                        id: generateUUID(),
                        flippedIndex: initialRecord.flippedIndex >= 1 && initialRecord.flippedIndex <= 5
                            ? initialRecord.flippedIndex : 0,
                        cardItems: {
                            1: initialRecord.card1 || '', 2: initialRecord.card2 || '', 3: initialRecord.card3 || '',
                            4: initialRecord.card4 || '', 5: initialRecord.card5 || '',
                        },
                        equipmentUnreviewed: Boolean(initialRecord.equipmentUnreviewed),
                        editedCardIndices: [],
                    }];
                const firstDraw = savedDraws[0];
                setManualDraws(savedDraws);
                setActiveManualDrawIndex(0);
                setFlipIndex(firstDraw.flippedIndex);
                setCardItems({ ...firstDraw.cardItems });
                setEquipmentUnreviewed(firstDraw.equipmentUnreviewed);
            }
        } else {
            applyRoleProgress(roleId);
            setFlipIndex(0);
            setEquipmentUnreviewed(true);
            setCardItems({ 1: '', 2: '', 3: '', 4: '', 5: '' });
        }
        setHasExplicitFlipIndexChange(false);
        setEditedCardIndices(new Set());
        setSelectedCard(null);
        if (!initialRecord) {
            setManualDraws([createManualTrialDrawDraft()]);
            setActiveManualDrawIndex(0);
        }
        setShowEquipSelector(false);
        setNotes(initialRecord?.notes || '');
        setError(null);
        setIsSubmitting(false);
        setRecordDate(formatDateForInput(initialRecord?.date || new Date()));
        loadAllEquipments();
    }, [isOpen, applyRoleProgress, initialRole?.id, initialRecord]);

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

    useEffect(() => {
        const handleEsc = (e: KeyboardEvent) => {
            if (e.key === 'Escape' && showEquipSelector) {
                setShowEquipSelector(false);
            }
        };
        window.addEventListener('keydown', handleEsc);
        return () => window.removeEventListener('keydown', handleEsc);
    }, [showEquipSelector]);

    const loadAllEquipments = async () => {
        setIsLoadingEquipments(true);
        setEquipmentCatalogState('loading');
        setEquipmentRangeMessage(null);
        setAllEquipments([]);
        try {
            const season = await db.getCurrentSeason();
            const configuredBounds = getTrialEquipmentLevelRange(season);
            if (!configuredBounds) {
                setEquipmentRangeMessage('当前赛季信息或试炼装备品级范围不可用，请关闭后重试。');
                setEquipmentCatalogState('empty');
                return;
            }
            const hasConfiguredBound = configuredBounds.min !== undefined || configuredBounds.max !== undefined;

            // 赛季来源决定装备名称和可选部位。新来源或已配置边界时不读旧的
            // SQLite 缓存；历史来源未配置边界时保持本地缓存兼容。
            const items = await getTrialEquipments(season, {
                minLevel: configuredBounds.min,
                maxLevel: configuredBounds.max,
            });
            if (items.length === 0) {
                setEquipmentRangeMessage(
                    hasConfiguredBound
                        ? '未获取到当前赛季试炼装备数据，请检查网络后重试；仍可录入无装备试炼。'
                        : '未获取到试炼装备数据，请检查网络后重试。'
                );
                setEquipmentCatalogState('empty');
                return;
            }

            // 先根据已确认的单边/双边范围过滤，再从过滤结果推导尚未确认的一端。
            // 不允许用旧赛季缓存的全量品级反推出范围，避免把低于当前赛季下限的
            // 装备混入选择器。
            const seasonItems = items.filter(item =>
                isTrialEquipmentLevelInRange(item.Level, configuredBounds)
            );
            const actualRange = deriveTrialEquipmentLevelRange(seasonItems.map(item => item.Level));
            if (!actualRange) {
                setEquipmentRangeMessage('当前赛季尚无符合品级范围的试炼装备，可继续录入无装备试炼。');
                setEquipmentCatalogState('empty');
                return;
            }

            // Define Sort Order (Using TypeLabel as requested)
            const TYPE_ORDER: Record<string, number> = {
                '武器': 0,
                '帽子': 1, '上衣': 2, '腰带': 3, '护腕': 4, '下装': 5, '鞋子': 6,
                '项链': 7, '腰坠': 8, '戒指': 9, '暗器': 10
            };

            // Sort items: Level Desc -> Type -> Physical -> Magical
            const sorted = seasonItems.sort((a, b) => {
                // 1. Level Descending
                if (b.Level !== a.Level) return b.Level - a.Level;

                // 2. Equipment Type (TypeLabel)
                const typeA = TYPE_ORDER[getTrialDrawEquipmentSlot(a)] ?? 99;
                const typeB = TYPE_ORDER[getTrialDrawEquipmentSlot(b)] ?? 99;
                if (typeA !== typeB) return typeA - typeB;

                // 3. Physical
                const aPhy = getTrialDrawEquipmentAttributes(a).includes('外功');
                const bPhy = getTrialDrawEquipmentAttributes(b).includes('外功');
                if (aPhy && !bPhy) return -1;
                if (!aPhy && bPhy) return 1;

                // 4. Magical (Neigong) - Tie breaker
                return 0;
            });

            setAllEquipments(sorted);
            setEquipmentCatalogState('ready');
        } catch (err) {
            console.error(err);
            setEquipmentRangeMessage('加载装备数据失败，请关闭后重新打开记录窗口重试。');
            setEquipmentCatalogState('error');
            toast.error('加载装备数据失败');
        } finally {
            setIsLoadingEquipments(false);
        }
    };

    const allRoles = accounts
        .filter(account => !account.disabled)
        .flatMap(account =>
            account.roles.map(role => ({
                ...role,
                accountId: account.id,
                accountName: account.accountName
            }))
        )
        .filter(role => !role.disabled && !role.isClient);

    const handleCardClick = (cardIdx: number) => {
        setSelectedCard(cardIdx);
        setShowEquipSelector(true);
    };

    const handleSelectEquipment = (equipId: string) => {
        if (selectedCard !== null) {
            setCardItems(prev => ({
                ...prev,
                [selectedCard]: equipId
            }));
            setEditedCardIndices(prev => new Set(prev).add(selectedCard));
        }
        setShowEquipSelector(false);
        setSelectedCard(null);
    };

    const handleClearEquipment = () => {
        if (selectedCard !== null) {
            setCardItems(prev => ({
                ...prev,
                [selectedCard]: ''
            }));
            setEditedCardIndices(prev => new Set(prev).add(selectedCard));
        }
        setShowEquipSelector(false);
        setSelectedCard(null);
    };

    const handleConfirmNoEquipment = () => {
        setCardItems({ 1: '', 2: '', 3: '', 4: '', 5: '' });
        setEditedCardIndices(new Set([1, 2, 3, 4, 5]));
        setEquipmentUnreviewed(false);
    };

    const handleConfirmAllCardsReviewed = () => {
        // 用户明确确认五张牌都已核对后，当前空位才可作为“未出装备”统计。
        setEditedCardIndices(new Set([1, 2, 3, 4, 5]));
        setEquipmentUnreviewed(false);
    };

    const getActiveManualDrawDraft = (): ManualTrialDrawDraft => ({
        id: manualDraws[activeManualDrawIndex]?.id || generateUUID(),
        flippedIndex: flipIndex,
        cardItems: { ...cardItems },
        equipmentUnreviewed,
        editedCardIndices: Array.from(editedCardIndices),
    });

    const loadManualDrawDraft = (draft: ManualTrialDrawDraft, index: number) => {
        setManualDraws(current => current.map((entry, entryIndex) => (
            entryIndex === activeManualDrawIndex ? getActiveManualDrawDraft() : entry
        )));
        setActiveManualDrawIndex(index);
        setFlipIndex(draft.flippedIndex);
        setCardItems({ ...draft.cardItems });
        setEquipmentUnreviewed(draft.equipmentUnreviewed);
        setEditedCardIndices(new Set(draft.editedCardIndices));
        setSelectedCard(null);
        setShowEquipSelector(false);
    };

    const addManualDraw = () => {
        const activeDraft = getActiveManualDrawDraft();
        const nextDraft = createManualTrialDrawDraft();
        setManualDraws(current => current.map((entry, index) => (
            index === activeManualDrawIndex ? activeDraft : entry
        )).concat(nextDraft));
        setActiveManualDrawIndex(manualDraws.length);
        setFlipIndex(nextDraft.flippedIndex);
        setCardItems({ ...nextDraft.cardItems });
        setEquipmentUnreviewed(nextDraft.equipmentUnreviewed);
        setEditedCardIndices(new Set(nextDraft.editedCardIndices));
        setSelectedCard(null);
        setShowEquipSelector(false);
    };

    const removeActiveManualDraw = () => {
        if (manualDraws.length <= 1) return;
        const retained = manualDraws.filter((_, index) => index !== activeManualDrawIndex);
        const nextIndex = Math.min(activeManualDrawIndex, retained.length - 1);
        const nextDraft = retained[nextIndex];
        if (!nextDraft) return;
        setManualDraws(retained);
        setActiveManualDrawIndex(nextIndex);
        setFlipIndex(nextDraft.flippedIndex);
        setCardItems({ ...nextDraft.cardItems });
        setEquipmentUnreviewed(nextDraft.equipmentUnreviewed);
        setEditedCardIndices(new Set(nextDraft.editedCardIndices));
        setSelectedCard(null);
        setShowEquipSelector(false);
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);

        if (!selectedRoleId) {
            setError('请选择角色');
            return;
        }
        if (!Number.isInteger(layer) || layer < 1 || layer > 120) {
            setError('层数必须在 1-120 之间');
            return;
        }
        if (!boss1 || !boss2 || !boss3) {
            setError('请填写所有三个 Boss');
            return;
        }

        setIsSubmitting(true);

        const role = allRoles.find(r => r.id === selectedRoleId);
        if (!role) {
            setError('找不到选中的角色');
            setIsSubmitting(false);
            return;
        }

        try {
            const recordId = initialRecord?.id || generateUUID();
            const activeDraft = getActiveManualDrawDraft();
            const drafts = canEditManualDraws
                ? manualDraws.map((draft, index) => index === activeManualDrawIndex ? activeDraft : draft)
                : [];
            const shouldPersistManualDraws = canEditManualDraws && (
                drafts.length > 1 || (initialRecord?.draws?.length ?? 0) > 0
            );
            const legacyDraw = drafts[0] || activeDraft;
            const manualDrawDetails: TrialDraw[] = shouldPersistManualDraws
                ? drafts.map((draft, index) => ({
                    id: draft.id,
                    sourceKey: `manual:${recordId}:${draft.id}`,
                    source: 'manual',
                    sequence: index + 1,
                    // 手工记录没有收据时间；0 是后端约定的未知时间，不伪造同秒收据。
                    receiptTime: 0,
                    flippedIndex: draft.flippedIndex,
                    rewardItems: [],
                    card1: draft.cardItems[1] || '',
                    card2: draft.cardItems[2] || '',
                    card3: draft.cardItems[3] || '',
                    card4: draft.cardItems[4] || '',
                    card5: draft.cardItems[5] || '',
                    equipmentUnreviewed: draft.equipmentUnreviewed,
                    editedCardIndices: draft.editedCardIndices,
                }))
                : [];
            const record: TrialPlaceRecord = {
                id: recordId,
                accountId: role.accountId,
                roleId: role.id,
                roleName: role.name,
                server: role.server,
                // 日期控件只显示到分钟；仅补录翻牌时保留日志中的准确通关时间。
                date: initialRecord && recordDate === formatDateForInput(initialRecord.date)
                    ? toTrialProgressTimestamp(initialRecord.date) ?? formatDateFromInput(recordDate)
                    : formatDateFromInput(recordDate),
                layer,
                bosses: [boss1, boss2, boss3],
                // 父字段只兼容旧单次记录。多次翻牌的展示和统计始终以 draws 为准。
                card1: legacyDraw.cardItems[1] || '',
                card2: legacyDraw.cardItems[2] || '',
                card3: legacyDraw.cardItems[3] || '',
                card4: legacyDraw.cardItems[4] || '',
                card5: legacyDraw.cardItems[5] || '',
                flippedIndex: legacyDraw.flippedIndex,
                type: 'trial',
                notes: notes.trim(),
                source: initialRecord?.source || 'manual',
                status: 'confirmed',
                sourceKey: initialRecord?.sourceKey,
                equipmentUnreviewed: legacyDraw.equipmentUnreviewed,
                // 多次手工翻牌的修改标记只属于子笔；父记录保留旧单次兼容字段。
                editedCardIndices: initialRecord && !shouldPersistManualDraws
                    ? Array.from(editedCardIndices)
                    : [],
                draws: manualDrawDetails,
            };

            await onSubmit(record, {
                preserveFlippedIndex: Boolean(
                    initialRecord
                    && !hasExplicitFlipIndexChange
                    && flipIndex === initialRecord.flippedIndex
                ),
            });
            toast.success(initialRecord?.status === 'pending' ? '试炼记录已确认' : '试炼记录已保存');
            onClose();
        } catch (err: any) {
            console.error(err);
            const errorMessage = err?.message || err?.toString() || '添加失败，请重试';
            setError(errorMessage);
            toast.error(errorMessage);
        } finally {
            setIsSubmitting(false);
        }
    };

    const selectedBossEquipmentStats = useMemo(() => {
        if (!boss1 || !boss2 || !boss3) {
            return null;
        }

        return findTrialBossEquipmentStats(records, [boss1, boss2, boss3]);
    }, [records, boss1, boss2, boss3]);
    const suggestedEquipmentPosition = selectedBossEquipmentStats?.bestEquipmentPosition?.position ?? null;

    const roleRecords = useMemo(() => {
        const roleId = initialRole?.id || selectedRoleId;
        if (!roleId) return [];
        return records.filter(r => r.roleId === roleId);
    }, [records, initialRole?.id, selectedRoleId]);

    const selectedBossEquipmentStatsForRole = useMemo(() => {
        const roleId = initialRole?.id || selectedRoleId;
        if (!boss1 || !boss2 || !boss3 || !roleId) return null;
        return findTrialBossEquipmentStats(roleRecords, [boss1, boss2, boss3]);
    }, [roleRecords, boss1, boss2, boss3, initialRole?.id, selectedRoleId]);

    const suggestedEquipmentPositionForRole = selectedBossEquipmentStatsForRole?.bestEquipmentPosition?.position ?? null;

    if (!isOpen) return null;

    // Helper to get full equipment object for card display
    const getEquipForCard = (id: string) => {
        return allEquipments.find(e => e.ID.toString() === id);
    };

    const flippedItemId = cardItems[flipIndex];
    const flippedEquip = flippedItemId ? getEquipForCard(flippedItemId) : null;
    const isTradable = flippedEquip && (flippedEquip.BindType === 1 || flippedEquip.BindType === 2);

    return createPortal(
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-ds-scrim backdrop-blur-sm overflow-hidden">
            <div className="bg-surface w-full max-w-3xl rounded-xl shadow-ds-modal border border-base overflow-hidden animate-in fade-in zoom-in duration-200 max-h-[95vh] flex flex-col">
                {/* Header */}
                <div className="flex items-center justify-between px-6 py-4 border-b border-base bg-base/50">
                    <div className="flex items-center gap-6">
                        <div className="flex items-center gap-2">
                            <div className="p-2 bg-slate-100 dark:bg-slate-800/50 rounded-xl border border-slate-200/50 dark:border-slate-700/50">
                                <Trophy className="w-5 h-5 text-ds-warning-strong/80" />
                            </div>
                            <div>
                                <h2 className="text-lg font-bold text-main">{hasProtectedDraws ? '编辑通关信息' : initialRecord ? (initialRecord.status === 'pending' ? '确认试炼记录' : '编辑试炼记录') : '新增试炼记录'}</h2>
                                {initialRole && (
                                    <p className="text-xs text-muted mt-0.5">
                                        <span className="font-medium text-main">{initialRole.name}·{getBaseServerName(initialRole.server)}</span>
                                    </p>
                                )}
                            </div>
                        </div>

                        {/* Role Selector in Header */}
                        {!initialRole && (
                            <>
                                <div className="h-6 w-px bg-base mx-2"></div>
                                <div className="flex items-center gap-3">
                                    <select
                                        className="pl-2 pr-8 py-1.5 rounded-lg bg-base border border-base text-sm focus:ring-2 focus:ring-primary/20 outline-none cursor-pointer hover:bg-base/80"
                                        value={selectedRoleId}
                                        onChange={e => {
                                            const roleId = e.target.value;
                                            setSelectedRoleId(roleId);
                                            applyRoleProgress(roleId);
                                        }}
                                    >
                                        <option value="">选择挑战角色...</option>
                                        {allRoles.map(r => (
                                            <option key={r.id} value={r.id}>{r.name} · {getBaseServerName(r.server)}</option>
                                        ))}
                                    </select>
                                </div>
                            </>
                        )}
                    </div>

                    <button
                        onClick={onClose}
                        className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-base text-muted hover:text-main transition-colors"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Form */}
                <form onSubmit={handleSubmit} className="p-6 space-y-6 overflow-y-auto">
                    {error && (
                        <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm flex items-center gap-2">
                            <AlertCircle className="w-4 h-4" />
                            {error}
                        </div>
                    )}

                    {/* Date Picker */}
                    <div className="relative z-50">
                        <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-1.5">记录日期</label>
                        <div className="relative w-full">
                            <DateTimePicker
                                value={recordDate}
                                onChange={setRecordDate}
                            />
                        </div>
                    </div>

                    {/* Challenge Info Bar */}
                    <div className="bg-base/30 rounded-xl p-4 border border-base/50 flex flex-wrap gap-6 items-end">
                        {/* Layer Input */}
                        <div className="w-24">
                            <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-1.5">挑战层数</label>
                            <div className="relative group">
                                <Layers className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <input
                                    type="number"
                                    value={layer}
                                    onChange={e => setLayer(parseInt(e.target.value) || 0)}
                                    className="w-full h-[38px] pl-9 pr-2 rounded-lg bg-surface border border-base font-bold text-sm text-main focus:ring-2 focus:ring-primary/20 outline-none text-center"
                                />
                            </div>
                        </div>

                        {/* Boss Selectors */}
                        <div className="flex-1 min-w-[300px]">
                            <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-1.5">关卡 BOSS</label>
                            <div className="grid grid-cols-3 gap-2">
                                <select
                                    value={boss1}
                                    onChange={e => setBoss1(e.target.value)}
                                    className="w-full h-[38px] px-3 rounded-lg bg-surface border border-base text-sm font-medium outline-none focus:ring-2 focus:ring-primary/20 hover:border-primary/30 transition-colors"
                                >
                                    <option value="" className="text-muted">第一关...</option>
                                    {BOSS_LEVEL_1.map(boss => (
                                        <option key={boss} value={boss}>{boss}</option>
                                    ))}
                                </select>
                                <select
                                    value={boss2}
                                    onChange={e => setBoss2(e.target.value)}
                                    className="w-full h-[38px] px-3 rounded-lg bg-surface border border-base text-sm font-medium outline-none focus:ring-2 focus:ring-primary/20 hover:border-primary/30 transition-colors"
                                >
                                    <option value="" className="text-muted">第二关...</option>
                                    {BOSS_LEVEL_2_3.map(boss => (
                                        <option key={boss} value={boss} disabled={boss3 === boss}>{boss}</option>
                                    ))}
                                </select>
                                <select
                                    value={boss3}
                                    onChange={e => setBoss3(e.target.value)}
                                    className="w-full h-[38px] px-3 rounded-lg bg-surface border border-base text-sm font-medium outline-none focus:ring-2 focus:ring-primary/20 hover:border-primary/30 transition-colors"
                                >
                                    <option value="" className="text-muted">第三关...</option>
                                    {BOSS_LEVEL_2_3.map(boss => (
                                        <option key={boss} value={boss} disabled={boss2 === boss}>{boss}</option>
                                    ))}
                                </select>
                            </div>
                        </div>
                    </div>

                    {/* Cards Section */}
                    {hasProtectedDraws ? (
                        <div className="rounded-lg border border-ds-success-soft bg-ds-success-soft px-3 py-3 text-sm text-ds-success-strong">
                            <div className="font-semibold">本轮已有 {initialRecord?.draws?.length} 条翻牌明细</div>
                            <p className="mt-1 text-xs leading-5">
                                每次的牌位、装备、奖励和精力消耗均独立保存。请在“查看记录”中逐笔补录牌位和装备；此处只确认通关信息，不会修改任一翻牌明细。
                            </p>
                        </div>
                    ) : (
                    <div className="space-y-4">
                        {canEditManualDraws && (
                            <div className="rounded-lg border border-base bg-base/30 p-3">
                                <div className="flex flex-wrap items-center gap-2">
                                    <div className="mr-auto">
                                        <div className="text-sm font-semibold text-main">本轮翻牌明细</div>
                                    </div>
                                    <button
                                        type="button"
                                        onClick={addManualDraw}
                                        className="inline-flex items-center justify-center rounded border border-ds-success-soft bg-ds-success-soft px-3 py-1.5 text-xs font-medium text-ds-success-strong transition-colors hover:bg-ds-success-soft/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong"
                                    >
                                        增加一次翻牌
                                    </button>
                                </div>
                                <div className="mt-3 flex flex-wrap gap-1.5" role="tablist" aria-label="选择本轮翻牌明细">
                                    {manualDraws.map((draw, index) => (
                                        <button
                                            key={draw.id}
                                            type="button"
                                            role="tab"
                                            aria-selected={activeManualDrawIndex === index}
                                            onClick={() => loadManualDrawDraft(draw, index)}
                                            className={`inline-flex min-w-20 items-center justify-center rounded border px-3 py-1.5 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong ${activeManualDrawIndex === index
                                                ? 'border-ds-success-soft bg-ds-success-soft text-ds-success-strong'
                                                : 'border-base bg-surface text-main hover:bg-base'}`}
                                        >
                                            第 {index + 1} 次
                                        </button>
                                    ))}
                                    {manualDraws.length > 1 && (
                                        <button
                                            type="button"
                                            onClick={removeActiveManualDraw}
                                            className="inline-flex items-center justify-center rounded border border-ds-warning-soft bg-ds-warning-soft px-3 py-1.5 text-xs font-medium text-ds-warning-strong transition-colors hover:bg-ds-warning-soft/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-warning-strong"
                                        >
                                            删除当前次
                                        </button>
                                    )}
                                </div>
                            </div>
                        )}
                        <div className="flex items-center justify-between">
                            <h3 className="text-sm font-semibold text-main flex items-center gap-2">
                                <div className="w-1 h-4 bg-primary rounded"></div>
                                {manualDraws.length > 1 ? `第 ${activeManualDrawIndex + 1} 次翻牌` : '翻牌记录'}
                            </h3>
                            <div className="text-xs text-muted flex items-center gap-1">
                                <HelpCircle className="w-3 h-3" />
                                点击卡牌设置装备
                            </div>
                        </div>

                        <div className="grid grid-cols-5 gap-3">
                            {[1, 2, 3, 4, 5].map(idx => {
                                const itemId = cardItems[idx];
                                const isFlipped = flipIndex === idx;
                                const hasItem = itemId && itemId.trim().length > 0;
                                const equipData = hasItem ? getEquipForCard(itemId) : null;
                                const isSuggestedEquipmentPosition = suggestedEquipmentPosition === idx;
                                const isSuggestedEquipmentPositionForRole = suggestedEquipmentPositionForRole === idx;

                                return (
                                    <div key={idx} className="flex flex-col gap-2 group/card">
                                        {/* Card */}
                                        <button
                                            type="button"
                                            onClick={() => handleCardClick(idx)}
                                            title={
                                                isSuggestedEquipmentPosition && isSuggestedEquipmentPositionForRole
                                                    ? `全部记录与当前角色该 Boss 顺序出装备最多：${idx}号位`
                                                    : isSuggestedEquipmentPosition
                                                        ? `全部记录中该 Boss 顺序出装备最多：${idx}号位`
                                                        : isSuggestedEquipmentPositionForRole
                                                            ? `当前角色该 Boss 顺序出装备最多：${idx}号位`
                                                            : undefined
                                            }
                                            className={`
                                                relative w-full aspect-[3/4] rounded-xl border-2 p-0 flex flex-col items-center justify-start overflow-hidden
                                                transition-colors duration-200
                                                ${isSuggestedEquipmentPosition
                                                    ? 'bg-ds-success-soft/80 dark:bg-ds-success-soft/20 border-ds-success dark:border-ds-success border-solid'
                                                    : isSuggestedEquipmentPositionForRole
                                                        ? 'bg-sky-50/80 dark:bg-sky-900/20 border-sky-400 dark:border-sky-600 border-solid'
                                                        : hasItem
                                                            ? 'bg-surface border-slate-200 dark:border-slate-700'
                                                            : 'bg-surface border-dashed border-base/60 hover:border-ds-success dark:hover:border-ds-success hover:bg-base/30'
                                                }
                                                ${isFlipped
                                                    ? 'ring-2 ring-ds-success dark:ring-ds-success ring-offset-2 ring-offset-surface'
                                                    : ''
                                                }
                                            `}
                                        >
                                            {/* Card Number Watermark */}
                                            <div className="absolute -bottom-4 -right-2 text-[60px] font-black text-muted/40 pointer-events-none select-none z-0">
                                                {idx}
                                            </div>

                                            {isSuggestedEquipmentPosition && (
                                                <div className="absolute left-1.5 top-1.5 z-30 rounded border border-ds-success-soft bg-ds-success-soft px-1.5 py-0.5 text-[10px] font-bold text-ds-success-strong dark:border-ds-success dark:bg-ds-success-soft/70 dark:text-ds-success-strong">
                                                    高发
                                                </div>
                                            )}
                                            {isSuggestedEquipmentPositionForRole && (
                                                <div className={`absolute z-30 rounded border border-sky-200 bg-sky-100 px-1.5 py-0.5 text-[10px] font-bold text-sky-700 dark:border-sky-700 dark:bg-sky-900/70 dark:text-sky-300 ${isSuggestedEquipmentPosition ? 'left-1.5 top-6' : 'left-1.5 top-1.5'}`}>
                                                    角色
                                                </div>
                                            )}

                                            {/* Equipment Display (Z-index to sit above watermark) */}
                                            <div className="relative z-10 w-full h-full p-1.5">
                                                {hasItem && equipData ? (
                                                    <EquipDisplay item={equipData} simple />
                                                ) : hasItem ? (
                                                    <div className="w-full h-full flex items-center justify-center text-center">
                                                        <span className="text-xs font-medium text-muted">
                                                            加载中...
                                                        </span>
                                                    </div>
                                                ) : (
                                                    <div className="w-full h-full flex flex-col items-center justify-center gap-2 opacity-40 group-hover/card:opacity-60 transition-opacity">
                                                        <Package className="w-8 h-8" />
                                                        <span className="text-[10px] font-medium uppercase tracking-widest">Card {idx}</span>
                                                    </div>
                                                )}
                                            </div>

                                            {/* Hover Overlay */}
                                            <div className="absolute inset-0 bg-ds-success/10 opacity-0 group-hover/card:opacity-100 transition-opacity flex items-center justify-center backdrop-blur-[1px] z-20">
                                                <div className="bg-surface/90 text-ds-success-strong dark:text-ds-success-strong text-xs font-medium px-3 py-1.5 rounded-full border border-ds-success-soft dark:border-ds-success-soft">
                                                    {hasItem ? '更换装备' : '选择装备'}
                                                </div>
                                            </div>
                                        </button>

                                        {/* Flip Toggle */}
                                        <button
                                            type="button"
                                            aria-pressed={isFlipped}
                                            aria-label={isFlipped ? `取消第 ${idx} 张翻牌位` : `标记第 ${idx} 张为翻牌位`}
                                            onClick={() => {
                                                setFlipIndex(currentIndex => currentIndex === idx ? 0 : idx);
                                                setHasExplicitFlipIndexChange(true);
                                            }}
                                            className={`
                                                flex items-center justify-center gap-1.5 w-full py-1.5 rounded-lg text-[10px] font-medium tracking-wide transition-colors border
                                                ${isFlipped
                                                    ? 'bg-ds-success-soft dark:bg-ds-success-soft/20 text-ds-success-strong dark:text-ds-success-strong border-ds-success-soft dark:border-ds-success-soft'
                                                    : 'bg-base/50 text-muted border-transparent hover:bg-base hover:text-main'
                                                }
                                            `}>
                                                {isFlipped && <Check className="w-3 h-3" />}
                                                {isFlipped ? '当前翻开' : '标记翻开'}
                                        </button>
                                    </div>
                                );
                            })}
                        </div>
                        {equipmentUnreviewed && (
                            <div
                                role="group"
                                aria-label="装备核对操作"
                                className="flex flex-wrap items-center gap-2 rounded-lg border border-ds-warning-soft bg-ds-warning-soft px-3 py-2 text-xs text-ds-warning-strong"
                            >
                                <button
                                    type="button"
                                    onClick={handleConfirmNoEquipment}
                                    className="rounded border border-ds-warning-strong/30 bg-surface px-2 py-1 font-medium hover:bg-ds-warning-soft/60"
                                >
                                    {manualDraws.length > 1 ? '确认本次无装备' : '确认本轮无装备'}
                                </button>
                                <button
                                    type="button"
                                    onClick={handleConfirmAllCardsReviewed}
                                    className="rounded border border-ds-warning-strong/30 bg-surface px-2 py-1 font-medium hover:bg-ds-warning-soft/60"
                                >
                                    已核对全部牌面
                                </button>
                            </div>
                        )}
                    </div>
                    )}

                    {/* Flipped Item Notes */}
                    <div className="grid grid-cols-1 gap-6">
                        <div>
                            <label className="block text-xs font-medium text-muted mb-1.5 flex items-center gap-1.5">
                                <Trophy className={`w-3.5 h-3.5 ${isTradable ? 'text-ds-success-strong/80' : 'text-slate-400'}`} />
                                {hasProtectedDraws
                                    ? '通关备注（不影响已保存的翻牌明细）'
                                    : flipIndex === 0
                                    ? '备注'
                                    : isTradable ? '开出可交易物品！(可添加备注)' : '当前翻开物品绑定'}
                            </label>
                            <textarea
                                value={notes}
                                onChange={e => setNotes(e.target.value)}
                                className="w-full p-3 rounded-lg bg-base border border-base text-sm text-main focus:ring-2 focus:ring-primary/20 outline-none resize-none h-[46px]"
                                placeholder="记录一些细节..."
                            />
                        </div>
                    </div>

                    {/* Actions */}
                    <div className="flex gap-3 pt-2">
                        <button
                            type="button"
                            onClick={onClose}
                            className="flex-1 py-2.5 rounded-lg border border-base text-muted font-medium hover:bg-base hover:text-main transition-colors"
                        >
                            取消
                        </button>
                        <button
                            type="submit"
                            disabled={isSubmitting}
                            className="flex-1 py-2.5 rounded-lg bg-primary text-white font-medium hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                        >
                            {isSubmitting ? (
                                <span className="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                            ) : (
                                <>
                                    <Save className="w-4 h-4" />
                                    {initialRecord ? (initialRecord.status === 'pending' ? '确认记录' : '保存修改') : '保存记录'}
                                </>
                            )}
                        </button>
                    </div>
                </form>
            </div>

            {/* Equipment Selector - Modal */}
            {showEquipSelector && (
                <div className="fixed inset-0 z-[60] flex items-center justify-center bg-ds-scrim p-4 backdrop-blur-sm" onClick={() => setShowEquipSelector(false)}>
                    <div className="flex h-[80vh] w-full max-w-5xl flex-col overflow-hidden rounded-card border border-base bg-surface shadow-ds-modal" onClick={event => event.stopPropagation()}>
                        <div className="flex items-center justify-between border-b border-base bg-base/50 px-4 py-3">
                            <h3 className="flex items-center gap-2 text-[1rem] font-bold text-main">
                                <Package className="h-4 w-4 text-primary" />选择装备
                                <span className="rounded-tag border border-base bg-surface px-2 py-0.5 text-xs font-normal text-muted">#{selectedCard}</span>
                            </h3>
                            <button type="button" onClick={() => setShowEquipSelector(false)} aria-label="关闭装备选择" className="rounded-tag p-1.5 text-muted transition-colors hover:bg-base hover:text-main">
                                <X className="h-5 w-5" />
                            </button>
                        </div>
                        <div className="min-h-0 flex-1 overflow-y-auto bg-base/30 p-4">
                            <TrialEquipmentPicker
                                equipments={allEquipments}
                                catalogState={isLoadingEquipments ? 'loading' : equipmentCatalogState}
                                catalogMessage={equipmentRangeMessage}
                                selectedEquipmentId={selectedCard === null ? '' : cardItems[selectedCard]}
                                onSelect={handleSelectEquipment}
                                onClear={handleClearEquipment}
                            />
                        </div>
                    </div>
                </div>
            )}
        </div>,
        document.body
    );
};
