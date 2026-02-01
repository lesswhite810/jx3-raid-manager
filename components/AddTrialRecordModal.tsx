import React, { useState, useEffect } from 'react';
import { Account, TrialPlaceRecord } from '../types';
import { X, Calendar, Layers, Trophy, Save, AlertCircle, HelpCircle, Check, Package } from 'lucide-react';
import { generateUUID } from '../utils/uuid';
import { toast } from '../utils/toastManager';
import { getEquip, JX3Equip } from '../services/jx3BoxApi';

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
    onSubmit: (record: TrialPlaceRecord) => void;
    accounts: Account[];
    initialRole?: RoleWithStatus;
}

const BOSS_LEVEL_1 = ['寅', '雷神', '子'];
const BOSS_LEVEL_2_3 = ['叶冬辰', '韦柔丝', '濯尘', '藤原樱奈', '方鹤影', '苏髓'];

// --- Helper Functions & Components ---

/**
 * Enhanced attribute formatting helper
 */
const getFormattedAttributes = (item: JX3Equip) => {
    const attrs: { label: string, color?: string }[] = [];

    // 1. Standard Attributes
    if (item.attributes && Array.isArray(item.attributes)) {
        item.attributes.forEach(attr => {
            // Check for special effect type first
            if (attr.type === 'atSkillEventHandler') {
                attrs.push({
                    label: '特效',
                    color: '#ffcc00'
                });
                return;
            }

            // Try AttributeTypes first
            let name = item.AttributeTypes?.[attr.type];

            // Fallback to parsing label
            if (!name && attr.label) {
                // Remove 提高... and numbers
                name = attr.label.replace(/提高.*$/, '').replace(/[0-9]+$/, '');
            }

            if (name) {
                // Simplify suffixes
                name = name.replace(/等级$|值$/, '');

                // Remove prefixes "外功" and "内功"
                name = name.replace(/^外功|^内功/, '');

                // Specific mappings
                if (name === '攻击') name = '攻击';
                if (name === '会心效果') name = '会效';
                if (name === '治疗成效') name = '治疗';
                // "无双等级" -> "无双" (handled by regex)
                // "外功破防等级" -> "外功破防" -> "破防" (handled by regex)

                attrs.push({
                    label: name,
                    color: (attr.color && attr.color.toLowerCase() !== '#ffffff' && attr.color.toLowerCase() !== 'white')
                        ? attr.color
                        : undefined // Map white to undefined to use theme text color
                });
            }
        });
    }

    // 2. Special Effects (atSkillEventHandler) - handled in loop now

    return attrs;
};

const hasAttribute = (item: JX3Equip, keyword: string) => {
    const attrs = getFormattedAttributes(item);
    return attrs.some(a => a.label.includes(keyword));
};

/**
 * Reusable Equipment Display Component ensures consistency
 * Used in both the list and the card
 */
const EquipDisplay: React.FC<{ item: JX3Equip, simple?: boolean }> = ({ item, simple }) => {
    const attributes = getFormattedAttributes(item);
    const iconUrl = item.IconID ? `https://icon.jx3box.com/icon/${item.IconID}.png` : null;

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

                {/* Bottom: Attributes */}
                <div className="w-full mt-1.5 pt-1 border-t border-white/5 flex-shrink-0">
                    <div className="grid grid-cols-2 gap-x-1 gap-y-0.5 w-full text-left px-0.5 max-h-[60px] overflow-hide">
                        {attributes.length > 0 ? (
                            attributes.slice(0, 6).map((attr, idx) => (
                                <div key={idx} className="text-[10px] leading-tight truncate text-center" style={attr.color ? { color: attr.color } : {}}>
                                    {attr.label}
                                </div>
                            ))
                        ) : (
                            <div className="col-span-2 text-[10px] text-muted text-center">无属性</div>
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
                {/* Header: Name + Level + BindType */}
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

                {/* Attributes: Single row */}
                <div className="flex items-center gap-x-2 text-xs mt-0.5">
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
    initialRole
}) => {
    const [selectedRoleId, setSelectedRoleId] = useState<string>(initialRole?.id || '');
    const [layer, setLayer] = useState<number>(50);
    const [boss1, setBoss1] = useState<string>('');
    const [boss2, setBoss2] = useState<string>('');
    const [boss3, setBoss3] = useState<string>('');

    // 5 Cards State
    const [flipIndex, setFlipIndex] = useState<number>(1);
    const [cardItems, setCardItems] = useState<Record<number, string>>({
        1: '', 2: '', 3: '', 4: '', 5: ''
    });
    const [selectedCard, setSelectedCard] = useState<number | null>(null);

    // Equipment selector state
    const [showEquipSelector, setShowEquipSelector] = useState(false);
    const [allEquipments, setAllEquipments] = useState<JX3Equip[]>([]);
    const [isLoadingEquipments, setIsLoadingEquipments] = useState(false);

    // Level Filter State
    const [levelBounds, setLevelBounds] = useState({ min: 10000, max: 40000 });
    const [minLevel, setMinLevel] = useState<number>(27000);
    const [maxLevel, setMaxLevel] = useState<number>(36000);

    const [notes, setNotes] = useState<string>('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Attribute filter state
    const ATTR_FILTERS = [
        { key: '会心', label: '会心' },
        { key: '会效', label: '会效' },
        { key: '破防', label: '破防' },
        { key: '加速', label: '加速' },
        { key: '破招', label: '破招' },
        { key: '无双', label: '无双' },
        { key: '特效', label: '特效' },
        { key: '全能', label: '全能' },
    ];
    const [attrFilters, setAttrFilters] = useState<Set<string>>(new Set());

    useEffect(() => {
        if (isOpen) {
            setSelectedRoleId(initialRole?.id || '');
            loadAllEquipments();
        }
    }, [isOpen, initialRole]);

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
        try {
            const items = await getEquip('无修');

            // Calculate Min/Max and Update State
            if (items.length > 0) {
                const levels = items.map(i => i.Level);
                const min = Math.min(...levels);
                const max = Math.max(...levels);
                setLevelBounds({ min, max });

                // Only update current filter if it's outside valid range or default
                if (minLevel < min) setMinLevel(min);
                if (maxLevel > max) setMaxLevel(max);
            }

            // Define Sort Order (Using TypeLabel as requested)
            const TYPE_ORDER: Record<string, number> = {
                '武器': 0,
                '帽子': 1, '上衣': 2, '腰带': 3, '护腕': 4, '下装': 5, '鞋子': 6,
                '项链': 7, '腰坠': 8, '戒指': 9, '暗器': 10
            };

            // Sort items: Level Desc -> Type -> Physical -> Magical
            const sorted = items.sort((a, b) => {
                // 1. Level Descending
                if (b.Level !== a.Level) return b.Level - a.Level;

                // 2. Equipment Type (TypeLabel)
                const typeA = TYPE_ORDER[a.TypeLabel || ''] ?? 99;
                const typeB = TYPE_ORDER[b.TypeLabel || ''] ?? 99;
                if (typeA !== typeB) return typeA - typeB;

                // 3. Physical
                const aPhy = hasAttribute(a, '外功');
                const bPhy = hasAttribute(b, '外功');
                if (aPhy && !bPhy) return -1;
                if (!aPhy && bPhy) return 1;

                // 4. Magical (Neigong) - Tie breaker
                return 0;
            });

            setAllEquipments(sorted);
        } catch (err) {
            console.error(err);
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
        }
        setShowEquipSelector(false);
        setSelectedCard(null);
    };

    const getFilteredEquipments = () => {
        let items = allEquipments;

        // Level Range Filter
        items = items.filter(item => item.Level >= minLevel && item.Level <= maxLevel);

        // Attribute Checkbox Filter
        if (attrFilters.size > 0) {
            items = items.filter(item => {
                const attrs = getFormattedAttributes(item).map(a => a.label).join(' ');
                // All selected attributes must be present
                return Array.from(attrFilters).every(filter => attrs.includes(filter));
            });
        }

        return items;
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);

        if (!selectedRoleId) {
            setError('请选择角色');
            return;
        }
        if (layer < 1 || layer > 100) {
            setError('层数必须在 1-100 之间');
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

        const jingJianIndices = Object.entries(cardItems)
            .filter(([_, itemId]) => itemId && itemId.trim().length > 0)
            .map(([idx, _]) => parseInt(idx))
            .sort((a, b) => a - b);

        // Convert IDs back to Names for storage compatibility
        const itemsByName: Record<number, string> = {};
        Object.entries(cardItems).forEach(([key, itemId]) => {
            const idx = parseInt(key);
            if (!itemId) {
                itemsByName[idx] = '';
                return;
            }
            const equip = allEquipments.find(e => e.ID.toString() === itemId);
            itemsByName[idx] = equip ? equip.Name : itemId; // Fallback to ID if not found (shouldn't happen)
        });

        const droppedItemId = cardItems[flipIndex];
        const droppedEquip = droppedItemId ? allEquipments.find(e => e.ID.toString() === droppedItemId) : null;
        const droppedEquipmentName = droppedEquip ? droppedEquip.Name : undefined;

        try {
            const record: TrialPlaceRecord = {
                id: generateUUID(),
                accountId: role.accountId,
                roleId: role.id,
                roleName: role.name,
                server: `${role.region} ${role.server}`,
                date: new Date().toISOString(),
                layer,
                bosses: [boss1, boss2, boss3],
                cards: {
                    total: 5,
                    flippedIndex: flipIndex,
                    jingJianIndices: jingJianIndices,
                    droppedEquipment: droppedEquipmentName,
                    items: itemsByName
                },
                type: 'trial',
                notes: notes.trim()
            };

            onSubmit(record);
            toast.success('试炼记录添加成功');
            onClose();
        } catch (err) {
            console.error(err);
            setError('添加失败，请重试');
        } finally {
            setIsSubmitting(false);
        }
    };

    if (!isOpen) return null;

    const filteredEquipments = getFilteredEquipments();

    // Helper to get full equipment object for card display
    const getEquipForCard = (id: string) => {
        return allEquipments.find(e => e.ID.toString() === id);
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm">
            <div className="bg-surface w-full max-w-3xl rounded-xl shadow-2xl border border-base overflow-hidden animate-in fade-in zoom-in duration-200 max-h-[95vh] flex flex-col">
                {/* Header */}
                <div className="flex items-center justify-between px-6 py-4 border-b border-base bg-base/50">
                    <div className="flex items-center gap-6">
                        <div className="flex items-center gap-2">
                            <div className="p-2 bg-amber-500/10 rounded-lg">
                                <Trophy className="w-5 h-5 text-amber-500" />
                            </div>
                            <h2 className="text-lg font-bold text-main">记录试炼</h2>
                        </div>

                        {/* Role Selector in Header */}
                        <div className="h-6 w-px bg-base mx-2"></div>
                        <div className="flex items-center gap-3">
                            {initialRole ? (
                                <div className="flex items-center gap-2 px-3 py-1.5 bg-base rounded-full border border-base/60 pr-4">
                                    <div className="w-6 h-6 rounded-full bg-primary/10 flex items-center justify-center text-primary font-bold text-xs">
                                        {initialRole.name.charAt(0)}
                                    </div>
                                    <div className="flex flex-col leading-none">
                                        <span className="font-bold text-main text-xs">{initialRole.name}</span>
                                        <span className="text-[10px] text-muted">{initialRole.server}</span>
                                    </div>
                                </div>
                            ) : (
                                <select
                                    className="pl-2 pr-8 py-1.5 rounded-lg bg-base border border-base text-sm focus:ring-2 focus:ring-primary/20 outline-none cursor-pointer hover:bg-base/80"
                                    value={selectedRoleId}
                                    onChange={e => setSelectedRoleId(e.target.value)}
                                >
                                    <option value="">选择挑战角色...</option>
                                    {allRoles.map(r => (
                                        <option key={r.id} value={r.id}>{r.name} @ {r.server}</option>
                                    ))}
                                </select>
                            )}
                        </div>
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

                    {/* Challenge Info Bar */}
                    <div className="bg-base/30 rounded-xl p-4 border border-base/50 flex flex-wrap gap-6 items-end">
                        {/* Layer Input */}
                        <div className="w-24">
                            <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-1.5">挑战层数</label>
                            <div className="relative group">
                                <Layers className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <input
                                    type="number"
                                    min="1"
                                    max="120"
                                    value={layer}
                                    onChange={e => setLayer(parseInt(e.target.value) || 0)}
                                    className="w-full pl-9 pr-2 py-2 rounded-lg bg-surface border border-base font-bold text-lg text-main focus:ring-2 focus:ring-primary/20 outline-none text-center"
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
                                    className="w-full px-3 py-2 rounded-lg bg-surface border border-base text-sm font-medium outline-none focus:ring-2 focus:ring-primary/20 hover:border-primary/30 transition-colors"
                                >
                                    <option value="" className="text-muted">第一关...</option>
                                    {BOSS_LEVEL_1.map(boss => (
                                        <option key={boss} value={boss}>{boss}</option>
                                    ))}
                                </select>
                                <select
                                    value={boss2}
                                    onChange={e => setBoss2(e.target.value)}
                                    className="w-full px-3 py-2 rounded-lg bg-surface border border-base text-sm font-medium outline-none focus:ring-2 focus:ring-primary/20 hover:border-primary/30 transition-colors"
                                >
                                    <option value="" className="text-muted">第二关...</option>
                                    {BOSS_LEVEL_2_3.map(boss => (
                                        <option key={boss} value={boss} disabled={boss3 === boss}>{boss}</option>
                                    ))}
                                </select>
                                <select
                                    value={boss3}
                                    onChange={e => setBoss3(e.target.value)}
                                    className="w-full px-3 py-2 rounded-lg bg-surface border border-base text-sm font-medium outline-none focus:ring-2 focus:ring-primary/20 hover:border-primary/30 transition-colors"
                                >
                                    <option value="" className="text-muted">第三关...</option>
                                    {BOSS_LEVEL_2_3.map(boss => (
                                        <option key={boss} value={boss} disabled={boss2 === boss}>{boss}</option>
                                    ))}
                                </select>
                            </div>
                        </div>

                        {/* Date Display (Read Only) */}
                        <div className="w-32 opacity-80">
                            <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-1.5">日期</label>
                            <div className="relative">
                                <Calendar className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted" />
                                <div className="w-full pl-8 pr-3 py-2.5 rounded-lg bg-base/50 border border-base text-xs font-medium text-muted cursor-default">
                                    {new Date().toLocaleDateString()}
                                </div>
                            </div>
                        </div>
                    </div>

                    {/* Cards Section */}
                    <div className="space-y-4">
                        <div className="flex items-center justify-between">
                            <h3 className="text-sm font-semibold text-main flex items-center gap-2">
                                <div className="w-1 h-4 bg-primary rounded"></div>
                                翻牌记录
                            </h3>
                            <div className="text-xs text-muted flex items-center gap-1">
                                <HelpCircle className="w-3 h-3" />
                                点击卡牌设置装备，标记翻开的牌
                            </div>
                        </div>

                        <div className="grid grid-cols-5 gap-3">
                            {[1, 2, 3, 4, 5].map(idx => {
                                const itemId = cardItems[idx];
                                const isFlipped = flipIndex === idx;
                                const hasItem = itemId && itemId.trim().length > 0;
                                const equipData = hasItem ? getEquipForCard(itemId) : null;

                                return (
                                    <div key={idx} className="flex flex-col gap-2 group/card">
                                        {/* Card */}
                                        <button
                                            type="button"
                                            onClick={() => handleCardClick(idx)}
                                            className={`
                                                relative w-full aspect-[3/4] rounded-xl border-2 p-0 flex flex-col items-center justify-start overflow-hidden
                                                transition-all duration-300 hover:-translate-y-1 hover:shadow-xl
                                                ${hasItem
                                                    ? 'bg-surface border-amber-400/50 dark:border-amber-600/50'
                                                    : 'bg-surface border-dashed border-base/60 hover:border-primary/50 hover:bg-base/30'
                                                }
                                                ${isFlipped
                                                    ? 'ring-2 ring-primary ring-offset-2 ring-offset-surface shadow-lg shadow-primary/10'
                                                    : ''
                                                }
                                            `}
                                        >
                                            {/* Card Number Watermark */}
                                            <div className="absolute -bottom-4 -right-2 text-[60px] font-black text-base/40 pointer-events-none select-none z-0">
                                                {idx}
                                            </div>

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
                                            <div className="absolute inset-0 bg-primary/10 opacity-0 group-hover/card:opacity-100 transition-opacity flex items-center justify-center backdrop-blur-[1px] z-20">
                                                <div className="bg-surface/90 text-primary text-xs font-bold px-3 py-1.5 rounded-full shadow-lg transform scale-90 group-hover/card:scale-100 transition-transform">
                                                    {hasItem ? '更换装备' : '选择装备'}
                                                </div>
                                            </div>
                                        </button>

                                        {/* Flip Toggle */}
                                        <label className="flex items-center justify-center cursor-pointer select-none">
                                            <input
                                                type="radio"
                                                name="flippedCard"
                                                checked={isFlipped}
                                                onChange={() => setFlipIndex(idx)}
                                                className="hidden"
                                            />
                                            <div className={`
                                                flex items-center justify-center gap-1.5 w-full py-1.5 rounded-lg text-[10px] font-bold tracking-wide transition-all border
                                                ${isFlipped
                                                    ? 'bg-primary text-white border-primary shadow-md shadow-primary/20 scale-105'
                                                    : 'bg-base/50 text-muted border-transparent hover:bg-base hover:text-main'
                                                }
                                            `}>
                                                {isFlipped && <Check className="w-3 h-3" />}
                                                {isFlipped ? '当前翻开' : '标记翻开'}
                                            </div>
                                        </label>
                                    </div>
                                );
                            })}
                        </div>
                    </div>

                    {/* Notes */}
                    <div>
                        <label className="block text-xs font-medium text-muted mb-1.5">备注（可选）</label>
                        <textarea
                            value={notes}
                            onChange={e => setNotes(e.target.value)}
                            className="w-full p-3 rounded-lg bg-base border border-base text-sm text-main focus:ring-2 focus:ring-primary/20 outline-none resize-none h-20"
                            placeholder="记录一些细节..."
                        />
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
                            className="flex-1 py-2.5 rounded-lg bg-primary text-white font-bold shadow-lg shadow-primary/25 hover:bg-primary-hover hover:shadow-primary/40 active:scale-[0.98] transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                        >
                            {isSubmitting ? (
                                <span className="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                            ) : (
                                <>
                                    <Save className="w-4 h-4" />
                                    保存记录
                                </>
                            )}
                        </button>
                    </div>
                </form>
            </div>

            {/* Equipment Selector - Bottom Sheet */}
            {showEquipSelector && (
                <div
                    className="fixed inset-0 z-[60] bg-black/30 backdrop-blur-sm animate-in fade-in duration-200"
                    onClick={() => {
                        setShowEquipSelector(false);
                    }}
                >
                    <div
                        className="absolute bottom-0 left-0 right-0 bg-surface rounded-t-2xl shadow-2xl border-t border-base max-h-[45vh] flex flex-col animate-in slide-in-from-bottom duration-300"
                        onClick={e => e.stopPropagation()}
                    >
                        {/* Header */}
                        <div className="p-3 border-b border-base flex-shrink-0">
                            <div className="flex items-center justify-between mb-2">
                                <h3 className="text-sm font-bold text-main">
                                    为 #{selectedCard} 号牌选择装备
                                </h3>
                                <button
                                    type="button"
                                    onClick={() => {
                                        setShowEquipSelector(false);
                                    }}
                                    className="text-muted hover:text-main"
                                >
                                    <X className="w-4 h-4" />
                                </button>
                            </div>

                            {/* Search and Filters */}
                            <div>
                                {/* Level Range Filter */}
                                <div className="flex items-center gap-2 mb-3">
                                    <span className="text-sm font-bold text-main whitespace-nowrap">品质筛选</span>
                                    <div className="flex-1 px-2">
                                        <div className="flex items-center justify-between text-xs text-muted mb-1 font-mono">
                                            <span>{minLevel}</span>
                                            <span>{maxLevel}</span>
                                        </div>
                                        <div className="relative h-4 flex items-center select-none touch-none">
                                            {/* Track */}
                                            <div className="absolute left-0 right-0 h-1 bg-base rounded-full overflow-hidden">
                                                <div
                                                    className="absolute h-full bg-primary/30"
                                                    style={{
                                                        left: `${((minLevel - levelBounds.min) / (levelBounds.max - levelBounds.min)) * 100}%`,
                                                        right: `${100 - ((maxLevel - levelBounds.min) / (levelBounds.max - levelBounds.min)) * 100}%`
                                                    }}
                                                />
                                            </div>

                                            {/* Min Slider */}
                                            <input
                                                type="range"
                                                min={levelBounds.min}
                                                max={levelBounds.max}
                                                value={minLevel}
                                                onChange={(e) => {
                                                    const val = Math.min(Number(e.target.value), maxLevel - 1);
                                                    setMinLevel(val);
                                                }}
                                                className="absolute inset-0 w-full h-full opacity-0 cursor-pointer pointer-events-none appearance-none [&::-webkit-slider-thumb]:pointer-events-auto [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-red-500"
                                            />
                                            {/* Custom Thumb Min */}
                                            <div
                                                className="absolute w-3.5 h-3.5 bg-white border border-gray-300 rounded-full shadow-sm pointer-events-none"
                                                style={{
                                                    left: `${((minLevel - levelBounds.min) / (levelBounds.max - levelBounds.min)) * 100}%`,
                                                    transform: 'translateX(-50%)'
                                                }}
                                            />

                                            {/* Max Slider */}
                                            <input
                                                type="range"
                                                min={levelBounds.min}
                                                max={levelBounds.max}
                                                value={maxLevel}
                                                onChange={(e) => {
                                                    const val = Math.max(Number(e.target.value), minLevel + 1);
                                                    setMaxLevel(val);
                                                }}
                                                className="absolute inset-0 w-full h-full opacity-0 cursor-pointer pointer-events-none appearance-none [&::-webkit-slider-thumb]:pointer-events-auto [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:appearance-none"
                                            />
                                            {/* Custom Thumb Max */}
                                            <div
                                                className="absolute w-3.5 h-3.5 bg-white border border-gray-300 rounded-full shadow-sm pointer-events-none"
                                                style={{
                                                    left: `${((maxLevel - levelBounds.min) / (levelBounds.max - levelBounds.min)) * 100}%`,
                                                    transform: 'translateX(-50%)'
                                                }}
                                            />
                                        </div>
                                    </div>
                                </div>



                                {/* Attribute Filters */}
                                <div className="flex flex-wrap items-center gap-x-3 gap-y-1 mt-2 text-xs">
                                    <span className="text-muted font-medium">属性筛选：</span>
                                    {ATTR_FILTERS.map(filter => (
                                        <label key={filter.key} className="flex items-center gap-1 cursor-pointer select-none">
                                            <input
                                                type="checkbox"
                                                checked={attrFilters.has(filter.key)}
                                                onChange={(e) => {
                                                    const newFilters = new Set(attrFilters);
                                                    if (e.target.checked) {
                                                        newFilters.add(filter.key);
                                                    } else {
                                                        newFilters.delete(filter.key);
                                                    }
                                                    setAttrFilters(newFilters);
                                                }}
                                                className="w-3.5 h-3.5 rounded border-gray-400 text-primary focus:ring-primary/20"
                                            />
                                            <span className="text-main">{filter.label}</span>
                                        </label>
                                    ))}
                                </div>
                            </div>
                        </div>

                        {/* Equipment List */}
                        <div className="flex-1 overflow-y-auto min-h-0">
                            {isLoadingEquipments ? (
                                <div className="p-8 text-center text-muted text-sm">
                                    <div className="w-6 h-6 border-2 border-primary/30 border-t-primary rounded-full animate-spin mx-auto mb-2"></div>
                                    加载装备中...
                                </div>
                            ) : filteredEquipments.length > 0 ? (
                                <div className="divide-y divide-base/50">
                                    {/* Clear option */}
                                    <button
                                        type="button"
                                        onClick={handleClearEquipment}
                                        className="w-full px-3 py-2 text-left hover:bg-red-50 dark:hover:bg-red-900/10 transition-colors"
                                    >
                                        <span className="text-sm font-medium text-red-500">清空选择</span>
                                    </button>

                                    {/* Equipment items */}
                                    {filteredEquipments.map((equip, idx) => (
                                        <div
                                            key={idx}
                                        >
                                            <button
                                                type="button"
                                                onClick={() => handleSelectEquipment(equip.ID.toString())}
                                                className="w-full px-3 py-2 text-left hover:bg-base transition-colors group"
                                            >
                                                <EquipDisplay item={equip} />
                                            </button>
                                        </div>
                                    ))}
                                </div>
                            ) : (
                                <div className="p-8 text-center text-muted text-sm">
                                    {minLevel !== 27000 || maxLevel !== 36000 || attrFilters.size > 0 ? '未找到匹配的装备' : '暂无装备数据'}
                                </div>
                            )}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
