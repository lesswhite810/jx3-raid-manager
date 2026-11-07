import React, { useState, useMemo, useCallback, useEffect } from 'react';
import { TrialDraw, TrialPlaceRecord, Account, Config, Season } from '../types';
import { Trophy, Check, Copy, Target, Search, X, BarChart3, Clock } from 'lucide-react';
import { AddTrialRecordModal } from './AddTrialRecordModal';
import { TrialRoleRecordsModal } from './TrialRoleRecordsModal';
import { TrialFlipStatsModal } from './TrialFlipStatsModal';
import { getLastMonday } from '../utils/cooldownManager';
import { db } from '../services/db';
import { toast } from '../utils/toastManager';
import { filterRaidRoles } from '../utils/raidRoleUtils';
import { SectIcon } from './SectIcon';
import { calculateTrialFlipStats } from '../utils/trialFlipStats';
import { isConfirmedTrialRecord } from '../utils/trialRecordUtils';
import { getTrialWeeklyDrawProgress } from '../utils/trialWeeklyDrawProgress';
import { getCardClass, getAddButtonClass, getViewButtonClass, SECONDARY_BUTTON_CLASS, CardStatus } from '../utils/cardStatusStyles';
import {
    getTrialProgressPeriod,
    isTrialRecordInProgressPeriod,
    toTrialProgressTimestamp,
    TrialProgressPeriod,
} from '../utils/trialProgress';

interface TrialPlaceManagerProps {
    records: TrialPlaceRecord[];
    accounts: Account[];
    config?: Config;
    onRefreshRecords?: () => void;
    /**
     * 配置变更回调。
     *
     * 原本本组件直接 `db.getConfig()` + `db.saveConfig()` 读写数据库，
     * 与 App 的自动保存形成双写：两边各自持有配置副本，谁后写谁覆盖。
     * 现在统一上抛给 App，由 App 的单一自动保存路径落库。
     */
    onConfigChange?: (updater: (prev: Config) => Config) => void;
}

export const TrialPlaceManager: React.FC<TrialPlaceManagerProps> = ({
    records,
    accounts,
    config,
    onRefreshRecords,
    onConfigChange
}) => {
    const [isAdding, setIsAdding] = useState(false);
    const [viewRecordsRole, setViewRecordsRole] = useState<any>(null);
    const [viewFlipStatsRole, setViewFlipStatsRole] = useState<any>(null);
    const [recordToConfirm, setRecordToConfirm] = useState<TrialPlaceRecord | null>(null);
    const [roleSearchTerm, setRoleSearchTerm] = useState('');
    const [autoFillEnabled, setAutoFillEnabled] = useState(false);

    useEffect(() => {
        if (config?.trial?.autoFillEnabled !== undefined) {
            setAutoFillEnabled(config.trial.autoFillEnabled);
        }
    }, [config?.trial?.autoFillEnabled]);

    const handleToggleAutoFill = useCallback((enabled: boolean) => {
        setAutoFillEnabled(enabled);
        // 走 App 的统一配置保存路径，避免与本组件直连数据库形成双写
        if (onConfigChange) {
            onConfigChange(prev => ({
                ...prev,
                trial: {
                    ...prev.trial,
                    autoFillEnabled: enabled,
                },
            }));
        } else {
            console.warn('[TrialPlaceManager] 缺少 onConfigChange，自动填充开关未持久化');
        }
    }, [onConfigChange]);

    const allRoles = useMemo(() => {
        return accounts
            .filter(acc => !acc.disabled)
            .flatMap(acc => acc.roles
                .filter(role => !role.disabled && !role.isClient && role.visibility?.trial !== false)
                .map(role => ({
                    ...role,
                    accountId: acc.id,
                    accountName: acc.accountName,
                    password: acc.password, // Pass password for display
                    equipmentScore: role.equipmentScore // Ensure equipmentScore is passed
                }))
            );
    }, [accounts]);

    const [selectedRole, setSelectedRole] = useState<any>(null); // For passing to modal

    // undefined 表示仍在加载；null 表示无法确定周期，必须阻止录入以保护已有进度。
    const [trialProgressPeriod, setTrialProgressPeriod] = useState<TrialProgressPeriod | null | undefined>(undefined);
    const [currentSeason, setCurrentSeason] = useState<Season | null>(null);

    useEffect(() => {
        let cancelled = false;
        setTrialProgressPeriod(undefined);

        Promise.all([db.getCurrentSeason(), db.getSeasons(), db.getGameVersions()])
            .then(([currentSeason, seasons, versions]) => {
                if (!cancelled) {
                    setCurrentSeason(currentSeason);
                    setTrialProgressPeriod(getTrialProgressPeriod(currentSeason, seasons, versions));
                }
            })
            .catch(() => {
                if (!cancelled) {
                    setCurrentSeason(null);
                    setTrialProgressPeriod(null);
                }
            });

        return () => {
            cancelled = true;
        };
    }, []);

    // Statistics per role
    const roleStats = useMemo(() => {
        const stats = new Map<string, {
            /** 保留既有通关次数口径，供层数、排序和三态样式使用。 */
            weeklyCount: number;
            /** 卡片面向用户展示的本周翻牌次数，可超过三次。 */
            weeklyDrawCount: number;
            /** 手工翻牌按父通关日期归属的次数，需要在卡片明确说明。 */
            manualDateFallbackDrawCount: number;
            /** 该角色本周内仍待处理的通关轮次数，按游戏周与大版本双窗口限定。 */
            pendingCount: number;
            maxLayer: number;
            lastRunDate?: string;
        }>();

        const now = new Date();
        const startOfWeek = getLastMonday(now);
        const seasonStartTime = currentSeason ? toTrialProgressTimestamp(currentSeason.startDate) : null;

        allRoles.forEach(role => {
            const allRoleRecords = records.filter(record => record.roleId === role.id);
            const roleRecords = allRoleRecords.filter(isConfirmedTrialRecord);
            const versionRecords = roleRecords.filter(record => isTrialRecordInProgressPeriod(record, trialProgressPeriod));
            const thisWeekRecords = versionRecords.filter(record => {
                const timestamp = toTrialProgressTimestamp(record.date);
                return timestamp !== null && timestamp >= startOfWeek.getTime();
            });
            const seasonRecords = seasonStartTime === null
                ? []
                : roleRecords.filter(record => {
                    const timestamp = toTrialProgressTimestamp(record.date);
                    return timestamp !== null && timestamp >= seasonStartTime;
                });
            const maxLayer = seasonRecords.length > 0 ? Math.max(...seasonRecords.map(record => record.layer)) : 0;
            const lastRunRecord = roleRecords.reduce<TrialPlaceRecord | undefined>((latest, record) => {
                const timestamp = toTrialProgressTimestamp(record.date);
                const latestTimestamp = latest ? toTrialProgressTimestamp(latest.date) : null;
                return timestamp !== null && (latestTimestamp === null || timestamp > latestTimestamp) ? record : latest;
            }, undefined);
            const lastRunDate = lastRunRecord ? String(lastRunRecord.date) : undefined;
            const weeklyDrawProgress = getTrialWeeklyDrawProgress(allRoleRecords, {
                weekStart: startOfWeek,
                progressPeriod: trialProgressPeriod,
            });

            stats.set(role.id, {
                weeklyCount: thisWeekRecords.length,
                weeklyDrawCount: weeklyDrawProgress.drawCount,
                manualDateFallbackDrawCount: weeklyDrawProgress.manualDateFallbackDrawCount,
                pendingCount: weeklyDrawProgress.pendingRunCount,
                maxLayer,
                lastRunDate
            });
        });
        return stats;
    }, [allRoles, records, trialProgressPeriod, currentSeason]);

    const roleFlipStats = useMemo(() => {
        const stats = new Map<string, ReturnType<typeof calculateTrialFlipStats>>();
        allRoles.forEach(role => {
            stats.set(
                role.id,
                calculateTrialFlipStats(records.filter(record => record.roleId === role.id))
            );
        });
        return stats;
    }, [allRoles, records]);

    const sortedRoles = useMemo(() => {
        const sorted = [...allRoles];
        sorted.sort((a, b) => {
            const statsA = roleStats.get(a.id) || {
                weeklyCount: 0, weeklyDrawCount: 0, manualDateFallbackDrawCount: 0, pendingCount: 0, maxLayer: 0, lastRunDate: undefined,
            };
            const statsB = roleStats.get(b.id) || {
                weeklyCount: 0, weeklyDrawCount: 0, manualDateFallbackDrawCount: 0, pendingCount: 0, maxLayer: 0, lastRunDate: undefined,
            };

            // 1. 三态排序：未清(0) > 部分清(1-2次) > 完全清(3次)，口径与卡片配色一致，均按本周翻牌次数
            const getStatus = (count: number): number => {
                if (count === 0) return 0; // 未清
                if (count >= 3) return 2; // 完全清
                return 1; // 部分清
            };
            const aStatus = getStatus(statsA.weeklyDrawCount);
            const bStatus = getStatus(statsB.weeklyDrawCount);
            if (aStatus !== bStatus) return aStatus - bStatus;

            // 2. Equipment Score (Desc)
            const aScore = a.equipmentScore || 0;
            const bScore = b.equipmentScore || 0;
            if (aScore !== bScore) return bScore - aScore;

            // 3. Last Record Date (Desc - Newest first)
            const aDate = statsA.lastRunDate ? new Date(statsA.lastRunDate).getTime() : 0;
            const bDate = statsB.lastRunDate ? new Date(statsB.lastRunDate).getTime() : 0;
            if (aDate !== bDate) return bDate - aDate;

            // 4. Server (Asc)
            if (a.server !== b.server) return a.server.localeCompare(b.server);

            // 5. Name (Asc)
            return a.name.localeCompare(b.name);
        });
        return sorted;
    }, [allRoles, roleStats]);

    const filteredRoles = useMemo(() => {
        return filterRaidRoles(sortedRoles, roleSearchTerm);
    }, [sortedRoles, roleSearchTerm]);

    const handleOpenAddModal = (role?: any) => {
        if (trialProgressPeriod === undefined) {
            toast.info('试炼进度数据正在加载，请稍后再试');
            return;
        }
        if (trialProgressPeriod === null) {
            toast.error('无法确定当前试炼大版本，请检查赛季配置后重试');
            return;
        }
        if (role) {
            setSelectedRole(role);
        } else {
            setSelectedRole(null);
        }
        setRecordToConfirm(null);
        setIsAdding(true);
    };

    const handleOpenRecordsModal = (role: any) => {
        setViewRecordsRole(role);
    };

    const handleConfirmTrialRecord = useCallback((record: TrialPlaceRecord) => {
        const role = allRoles.find(candidate => candidate.id === record.roleId);
        if (!role) {
            toast.error('未找到自动记录对应的角色，无法确认');
            return;
        }
        setViewRecordsRole(null);
        setSelectedRole(role);
        setRecordToConfirm(record);
        setIsAdding(true);
    }, [allRoles]);

    const handleOpenFlipStatsModal = (role: any) => {
        setViewFlipStatsRole(role);
    };

    // 新增与确认均由这里唯一写入，避免弹窗与页面分别落库。
    const handleSaveTrialRecord = useCallback(async (
        record: TrialPlaceRecord,
        options?: { preserveFlippedIndex?: boolean },
    ) => {
        await db.addTrialRecord(record, options?.preserveFlippedIndex);
        onRefreshRecords?.();
    }, [onRefreshRecords]);

    const handleUpdateTrialDrawFlippedIndex = useCallback(async (
        recordId: string,
        drawId: string,
        flippedIndex: number | null,
    ) => {
        await db.updateTrialDrawFlippedIndex(recordId, drawId, flippedIndex);
        onRefreshRecords?.();
    }, [onRefreshRecords]);

    const handleUpdateTrialDrawEquipment = useCallback(async (
        _recordId: string,
        drawId: string,
        cardIndex: number,
        equipmentId: string | null,
    ) => {
        await db.updateTrialDrawEquipment(drawId, cardIndex, equipmentId);
        onRefreshRecords?.();
    }, [onRefreshRecords]);

    const handleUpdateTrialDrawEquipmentReview = useCallback(async (
        _recordId: string,
        draw: TrialDraw,
        reviewed: boolean,
    ) => {
        try {
            await db.setTrialDrawEquipmentReviewed(draw.id, reviewed, draw.flippedIndex,
                [draw.card1, draw.card2, draw.card3, draw.card4, draw.card5]);
        } finally {
            // 冲突时也刷新，避免用户重复按旧画面确认。
            onRefreshRecords?.();
        }
    }, [onRefreshRecords]);

    // 删除试炼记录：调用数据库删除，然后重新查询
    const handleDeleteTrialRecord = useCallback(async (recordId: string) => {
        try {
            await db.deleteTrialRecord(recordId);
            onRefreshRecords?.();
            toast.success('删除试炼记录成功');
        } catch (error) {
            console.error('删除试炼记录失败:', error);
            toast.error('删除试炼记录失败');
        }
    }, [onRefreshRecords]);

    // State for copy feedback
    const [copiedField, setCopiedField] = useState<string | null>(null);

    const copyToClipboard = async (text: string, fieldId: string) => {
        try {
            await navigator.clipboard.writeText(text);
            setCopiedField(fieldId);
            setTimeout(() => setCopiedField(null), 2000);
        } catch (err) {
            console.error('Failed to copy:', err);
        }
    };

    const getMaskedPassword = (password: string | undefined) => {
        if (!password) return '••••••';
        return '••••••';
    };

    return (
        <div className="space-y-8 animate-in fade-in duration-300">
            {/* Header / Role Grid */}
            <div className="space-y-4">
                <div className="flex items-center justify-between">
                    <h2 className="text-lg font-bold text-main flex items-center gap-2">
                        <Target className="w-5 h-5 text-primary" />
                        角色挑战进度
                    </h2>
                    <label className="flex items-center gap-2 cursor-pointer select-none">
                        <span className="text-xs text-muted">自动填充</span>
                        <div
                            className={`relative w-10 h-5 rounded-full transition-colors ${autoFillEnabled ? 'bg-primary' : 'bg-slate-300 dark:bg-slate-600'}`}
                            onClick={() => handleToggleAutoFill(!autoFillEnabled)}
                        >
                            <div
                                className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform ${autoFillEnabled ? 'translate-x-5' : 'translate-x-0.5'}`}
                            />
                        </div>
                    </label>
                </div>

                <div className="relative max-w-md">
                    <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
                    <input
                        type="text"
                        data-page-search-input="true"
                        value={roleSearchTerm}
                        onChange={(event) => setRoleSearchTerm(event.target.value)}
                        placeholder="搜索角色、区服或账号"
                        className="w-full pl-10 pr-10 py-2.5 rounded-lg border border-base bg-surface text-main text-sm focus:ring-1 focus:ring-primary focus:border-primary transition-all placeholder:text-muted"
                    />
                    {roleSearchTerm && (
                        <button
                            type="button"
                            onClick={() => setRoleSearchTerm('')}
                            className="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded-md text-muted hover:text-main hover:bg-base transition-colors"
                            title="清空搜索"
                        >
                            <X className="w-4 h-4" />
                        </button>
                    )}
                </div>

                {filteredRoles.length === 0 ? (
                    <div className="text-center py-8 text-muted bg-surface rounded-xl border border-base border-dashed">
                        {roleSearchTerm
                            ? `没有找到与“${roleSearchTerm}”匹配的角色`
                            : '请先在账号管理中添加并启用角色'}
                    </div>
                ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                        {filteredRoles.map(role => {
                            const stats = roleStats.get(role.id) || {
                                weeklyCount: 0, weeklyDrawCount: 0, manualDateFallbackDrawCount: 0, pendingCount: 0, maxLayer: 0, lastRunDate: undefined,
                            };
                            const pendingCount = stats.pendingCount;
                            // 三态按「本周翻牌次数」判定：卡片上唯一展示的进度数字就是
                            // 「本周翻牌 N 次」，若用通关次数判定会出现「显示 4 次却是未打满」
                            // 的自我矛盾（一次通关内可翻多张牌）。
                            const getTrialStatus = (): 'none' | 'partial' | 'complete' => {
                                if (stats.weeklyDrawCount === 0) return 'none';
                                if (stats.weeklyDrawCount >= 3) return 'complete';
                                return 'partial';
                            };
                            // 按钮禁用逻辑：不限制添加次数
                            const canAddTrial = true;
                            const trialStatus = getTrialStatus();
                            const pendingReviewDescription = pendingCount > 0
                                ? `本周存在 ${pendingCount} 条待确认试炼记录`
                                : undefined;

                            // 卡片 / 按钮样式统一由 utils/cardStatusStyles.ts 提供，
                            // 与团队副本、百战共用同一份映射。
                            // 待确认优先于通关进度：避免已通关记录看起来像已完全处理。
                            const cardStatus: CardStatus = pendingCount > 0 ? 'pending' : trialStatus;
                            const getCardStyle = () => getCardClass(cardStatus);

                            const getIconStyle = () => {
                                if (pendingCount > 0) {
                                    return (
                                        <div className="w-6 h-6 rounded-full bg-ds-warning-soft dark:bg-ds-warning-soft/30 flex items-center justify-center flex-shrink-0">
                                            <Clock className="w-3.5 h-3.5 text-ds-warning-strong dark:text-ds-warning-strong" />
                                        </div>
                                    );
                                }
                                if (trialStatus === 'complete') {
                                    return (
                                        <div className="w-6 h-6 rounded-full bg-ds-neutral-soft dark:bg-ds-neutral-soft/30 flex items-center justify-center flex-shrink-0">
                                            <Check className="w-3.5 h-3.5 text-ds-neutral dark:text-ds-neutral" />
                                        </div>
                                    );
                                } else if (trialStatus === 'partial') {
                                    return (
                                        <div className="w-6 h-6 rounded-full bg-ds-info-soft dark:bg-ds-info-soft/30 flex items-center justify-center flex-shrink-0">
                                            <div className="w-2.5 h-2.5 rounded-full bg-ds-info" />
                                        </div>
                                    );
                                } else {
                                    return (
                                        <div className="w-6 h-6 rounded-full bg-ds-success-soft dark:bg-ds-success-soft/30 flex items-center justify-center flex-shrink-0">
                                            <div className="w-2.5 h-2.5 rounded-full bg-ds-success" />
                                        </div>
                                    );
                                }
                            };

                            // 实心按钮底色用 *-strong 档配 ds-fg-inverse，层级保持原设计：
                            // 未清 / 完全清为实心主操作，部分清 / 待确认为描边。
                            const getButtonAddStyle = () => getAddButtonClass(cardStatus, !canAddTrial);

                            const getButtonViewStyle = () => getViewButtonClass(cardStatus);

                            // RaidDetail-like Card Style
                            return (
                                <div
                                    key={role.id}
                                    className={`p-4 rounded-card border-2 transition-all duration-300 ${getCardStyle()}`}
                                    role="group"
                                    aria-label={pendingReviewDescription}
                                    title={pendingReviewDescription}
                                >
                                    <div className="flex items-center justify-between mb-3">
                                        <div className="flex items-center gap-2 flex-1 min-w-0">
                                            {/* Status Icon */}
                                            {getIconStyle()}

                                            <div className="min-w-0 flex-1">
                                                <div className="font-semibold text-main truncate flex items-center gap-2 flex-wrap">
                                                    <span className="truncate" title={`${role.name}·${role.server}`}>{role.name}·{role.server}</span>
                                                    {role.martial && (
                                                        <SectIcon sectName={role.martial} variant="image" size="sm" />
                                                    )}
                                                    {role.equipmentScore !== undefined && role.equipmentScore !== null && role.equipmentScore > 0 && (
                                                        <span className="text-xs bg-blue-100 dark:bg-blue-950/40 text-blue-700 dark:text-blue-300 px-2 py-1 rounded-md font-medium flex-shrink-0">
                                                            装分 {role.equipmentScore.toLocaleString()}
                                                        </span>
                                                    )}
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    <div className="flex items-center justify-between text-sm mb-3">
                                        <div className="flex items-center gap-2">
                                            <Trophy className={`w-3.5 h-3.5 flex-shrink-0 ${stats.maxLayer > 0 ? 'text-ds-warning-strong' : 'text-muted'}`} />
                                            <span className="text-xs text-muted">
                                                本赛季最高: <span className="font-medium text-main">{stats.maxLayer > 0 ? `${stats.maxLayer}层` : '-'}</span>
                                            </span>
                                        </div>

                                        <div className="flex flex-col items-end gap-0.5">
                                            <div className="flex items-baseline gap-1">
                                            <span className="text-xs text-muted">本周翻牌:</span>
                                            <span className={`font-bold ${pendingCount > 0 ? 'text-ds-warning-strong' : trialStatus === 'complete' ? 'text-ds-neutral' : trialStatus === 'partial' ? 'text-ds-info-strong' : 'text-ds-success-strong'}`}>
                                                {stats.weeklyDrawCount}<span className="text-xs text-muted font-normal">次</span>
                                            </span>
                                            </div>
                                            {stats.manualDateFallbackDrawCount > 0 && (
                                                <span className="text-[11px] text-muted">
                                                    含手工 {stats.manualDateFallbackDrawCount} 次按通关日期计入
                                                </span>
                                            )}
                                        </div>
                                    </div>

                                    {/* Account Info Section (Clean Copy from RaidDetail) */}
                                    <div className="mt-3 pt-3 border-t border-base space-y-2">
                                        <div className="flex items-center gap-2">
                                            <div className="text-xs text-muted flex-shrink-0">账号</div>
                                            <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
                                                <span className="text-xs text-main truncate flex-1">{role.accountName}</span>
                                                <button
                                                    onClick={(e) => {
                                                        e.stopPropagation(); // Although no parent click logic yet, good practice
                                                        copyToClipboard(role.accountName, `account-${role.id}`);
                                                    }}
                                                    className={`flex-shrink-0 p-1 rounded transition-colors ${copiedField === `account-${role.id}`
                                                        ? 'text-ds-success'
                                                        : 'text-muted hover:text-main hover:bg-surface'
                                                        }`}
                                                    title={copiedField === `account-${role.id}` ? '已复制!' : '复制账号'}
                                                >
                                                    {copiedField === `account-${role.id}` ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                                                </button>
                                            </div>
                                        </div>

                                        <div className="flex items-center gap-2">
                                            <div className="text-xs text-muted flex-shrink-0">密码</div>
                                            {role.password ? (
                                                <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
                                                    <span className="text-xs text-main font-mono truncate flex-1">
                                                        {getMaskedPassword(role.password)}
                                                    </span>
                                                    <button
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            copyToClipboard(role.password ?? '', `password-${role.id}`);
                                                        }}
                                                        className={`flex-shrink-0 p-1 rounded transition-colors ${copiedField === `password-${role.id}`
                                                            ? 'text-ds-success'
                                                            : 'text-muted hover:text-main hover:bg-surface'
                                                            }`}
                                                        title="复制密码"
                                                    >
                                                        {copiedField === `password-${role.id}` ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                                                    </button>
                                                </div>
                                            ) : (
                                                <div className="flex-1 min-w-0 bg-base rounded px-2 py-1">
                                                    <span className="text-xs text-muted truncate flex-1">未配置密码</span>
                                                </div>
                                            )}
                                        </div>
                                    </div>

                                    {/* Action Buttons */}
                                    <div className="mt-3 grid grid-cols-3 gap-2">
                                        <button
                                            onClick={() => handleOpenAddModal(role)}
                                            disabled={!canAddTrial}
                                            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${!canAddTrial
                                                ? 'bg-base text-muted cursor-not-allowed border border-base'
                                                : getButtonAddStyle()
                                                }`}
                                        >
                                            添加记录
                                        </button>
                                        <button
                                            onClick={() => handleOpenFlipStatsModal(role)}
                                            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${SECONDARY_BUTTON_CLASS} flex items-center justify-center gap-1`}
                                        >
                                            <BarChart3 className="w-3.5 h-3.5" />
                                            翻牌统计
                                        </button>
                                        <button
                                            onClick={() => handleOpenRecordsModal(role)}
                                            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${getButtonViewStyle()}`}
                                        >
                                            查看详情
                                        </button>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>

            <AddTrialRecordModal
                isOpen={isAdding}
                onClose={() => {
                    setIsAdding(false);
                    setRecordToConfirm(null);
                }}
                onSubmit={handleSaveTrialRecord}
                accounts={accounts}
                initialRole={selectedRole}
                initialRecord={recordToConfirm || undefined}
                autoFillEnabled={autoFillEnabled}
                records={records}
                trialProgressPeriod={trialProgressPeriod}
            />

            {
                viewRecordsRole && (
                    <TrialRoleRecordsModal
                        isOpen={!!viewRecordsRole}
                        onClose={() => setViewRecordsRole(null)}
                        role={viewRecordsRole}
                        records={records}
                        onDeleteRecord={handleDeleteTrialRecord}
                        onConfirmRecord={handleConfirmTrialRecord}
                        onUpdateDrawFlippedIndex={handleUpdateTrialDrawFlippedIndex}
                        onUpdateDrawEquipment={handleUpdateTrialDrawEquipment}
                        onUpdateDrawEquipmentReview={handleUpdateTrialDrawEquipmentReview}
                    />
                )
            }
            {
                viewFlipStatsRole && (
                    <TrialFlipStatsModal
                        isOpen={!!viewFlipStatsRole}
                        onClose={() => setViewFlipStatsRole(null)}
                        roleName={viewFlipStatsRole.name}
                        server={viewFlipStatsRole.server}
                        stats={roleFlipStats.get(viewFlipStatsRole.id) || calculateTrialFlipStats([])}
                    />
                )
            }
        </div >
    );
};
