import React, { useState, useMemo, useCallback } from 'react';
import { BaizhanRecord, Account } from '../types';
import { Swords, Check, Copy } from 'lucide-react';
import { AddBaizhanRecordModal } from './AddBaizhanRecordModal';
import { BaizhanRoleRecordsModal } from './BaizhanRoleRecordsModal';
import { getLastMonday } from '../utils/cooldownManager';
import { db } from '../services/db';
import { toast } from '../utils/toastManager';

interface BaizhanManagerProps {
    records: BaizhanRecord[];
    accounts: Account[];
    onEditRecord?: (record: BaizhanRecord) => void;
    onRefreshRecords?: () => void;
}

export const BaizhanManager: React.FC<BaizhanManagerProps> = ({
    records,
    accounts,
    onEditRecord,
    onRefreshRecords
}) => {
    const [isAdding, setIsAdding] = useState(false);
    const [viewRecordsRole, setViewRecordsRole] = useState<any>(null);

    const allRoles = useMemo(() => {
        return accounts
            .filter(acc => !acc.disabled)
            .flatMap(acc => acc.roles
                .filter(role => !role.disabled && !role.isClient)
                .map(role => ({
                    ...role,
                    accountId: acc.id,
                    accountName: acc.accountName,
                    password: acc.password,
                    equipmentScore: role.equipmentScore
                }))
            );
    }, [accounts]);

    const [selectedRole, setSelectedRole] = useState<any>(null);

    // Statistics per role
    const roleStats = useMemo(() => {
        const stats = new Map<string, { weeklyCount: number, weeklyIncome: number, lastRunDate?: string }>();

        // 统计本周数据，使用周一 7:00
        const now = new Date();
        const startOfWeek = getLastMonday(now);

        // 辅助函数：兼容时间戳和ISO字符串
        const getRecordTime = (date: string | number): number => {
            return typeof date === 'number' ? date : new Date(date).getTime();
        };

        allRoles.forEach(role => {
            const roleRecords = records.filter(r => r.roleId === role.id);
            const thisWeekRecords = roleRecords.filter(r => getRecordTime(r.date) >= startOfWeek.getTime());

            // 本周收入合计
            const weeklyIncome = thisWeekRecords.reduce((sum, r) => sum + (r.goldIncome || 0), 0);

            // 最后运行日期
            const lastRunRecord = [...roleRecords].sort((a, b) => getRecordTime(b.date) - getRecordTime(a.date))[0];
            const lastRunDate = lastRunRecord ? String(lastRunRecord.date) : undefined;

            stats.set(role.id, {
                weeklyCount: thisWeekRecords.length,
                weeklyIncome,
                lastRunDate
            });
        });
        return stats;
    }, [allRoles, records]);

    const sortedRoles = useMemo(() => {
        const sorted = [...allRoles];
        sorted.sort((a, b) => {
            const statsA = roleStats.get(a.id) || { weeklyCount: 0, weeklyIncome: 0, lastRunDate: undefined };
            const statsB = roleStats.get(b.id) || { weeklyCount: 0, weeklyIncome: 0, lastRunDate: undefined };

            // 1. Availability - haven't run this week first
            const aCanRun = statsA.weeklyCount === 0 ? 0 : 1;
            const bCanRun = statsB.weeklyCount === 0 ? 0 : 1;
            if (aCanRun !== bCanRun) return aCanRun - bCanRun;

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

    const handleOpenAddModal = (role?: any) => {
        setSelectedRole(role || null);
        setIsAdding(true);
    };

    const handleOpenRecordsModal = (role: any) => {
        setViewRecordsRole(role);
    };

    // 添加百战记录：调用数据库添加，然后重新查询
    const handleAddBaizhanRecord = useCallback(async (record: BaizhanRecord) => {
        try {
            await db.addBaizhanRecord(record);
            onRefreshRecords?.();
            toast.success('添加百战记录成功');
        } catch (error) {
            console.error('添加百战记录失败:', error);
            toast.error('添加百战记录失败');
        }
    }, [onRefreshRecords]);

    // 删除百战记录：调用数据库删除，然后重新查询
    const handleDeleteBaizhanRecord = useCallback(async (recordId: string) => {
        try {
            await db.deleteBaizhanRecord(recordId);
            onRefreshRecords?.();
            toast.success('删除百战记录成功');
        } catch (error) {
            console.error('删除百战记录失败:', error);
            toast.error('删除百战记录失败');
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
                        <Swords className="w-5 h-5 text-purple-500" />
                        角色百战进度
                    </h2>
                </div>

                {sortedRoles.length === 0 ? (
                    <div className="text-center py-8 text-muted bg-surface rounded-xl border border-base border-dashed">
                        请先在账号管理中添加并启用角色
                    </div>
                ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                        {sortedRoles.map(role => {
                            const stats = roleStats.get(role.id) || { weeklyCount: 0, weeklyIncome: 0, lastRunDate: undefined };

                            // 百战每周1次：未清(0) / 完全清(1+)
                            const baizhanStatus: 'none' | 'complete' = stats.weeklyCount === 0 ? 'none' : 'complete';

                            // 根据状态设置样式
                            const getCardStyle = () => {
                                if (baizhanStatus === 'complete') {
                                    return 'bg-gradient-to-br from-slate-50 to-gray-50 dark:from-slate-900/10 dark:to-gray-900/10 border-slate-200 dark:border-slate-700 hover:shadow-md';
                                } else {
                                    return 'bg-gradient-to-br from-emerald-50 to-teal-50 dark:from-emerald-900/10 dark:to-teal-900/10 border-emerald-200 dark:border-emerald-800 hover:shadow-lg hover:border-emerald-300';
                                }
                            };

                            const getIconStyle = () => {
                                if (baizhanStatus === 'complete') {
                                    return (
                                        <div className="w-6 h-6 rounded-full bg-slate-200 dark:bg-slate-700 flex items-center justify-center flex-shrink-0">
                                            <Check className="w-3.5 h-3.5 text-slate-500 dark:text-slate-400" />
                                        </div>
                                    );
                                } else {
                                    return (
                                        <div className="w-6 h-6 rounded-full bg-emerald-100 dark:bg-emerald-900/30 flex items-center justify-center flex-shrink-0">
                                            <div className="w-2.5 h-2.5 rounded-full bg-emerald-500 animate-pulse" />
                                        </div>
                                    );
                                }
                            };

                            const getSectStyle = () => {
                                if (baizhanStatus === 'complete') {
                                    return 'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-400';
                                } else {
                                    return 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400';
                                }
                            };

                            const getButtonAddStyle = () => {
                                if (baizhanStatus === 'complete') {
                                    return 'bg-slate-500 text-white hover:bg-slate-600 hover:shadow-md transform hover:-translate-y-0.5';
                                } else {
                                    return 'bg-emerald-500 text-white hover:bg-emerald-600 hover:shadow-md transform hover:-translate-y-0.5';
                                }
                            };

                            const getButtonViewStyle = () => {
                                if (baizhanStatus === 'complete') {
                                    return 'bg-white text-slate-700 border border-slate-200 hover:bg-slate-50 hover:border-slate-300';
                                } else {
                                    return 'bg-white text-emerald-700 border border-emerald-200 hover:bg-emerald-50 hover:border-emerald-300';
                                }
                            };

                            return (
                                <div
                                    key={role.id}
                                    className={`p-4 rounded-xl border-2 transition-all duration-300 ${getCardStyle()}`}
                                >
                                    <div className="flex items-center justify-between mb-3">
                                        <div className="flex items-center gap-2 flex-1 min-w-0">
                                            {/* Status Icon */}
                                            {getIconStyle()}

                                            <div className="min-w-0 flex-1">
                                                <div className="font-semibold text-main truncate flex items-center gap-2 flex-wrap">
                                                    <span className="truncate" title={`${role.name}@${role.server}`}>{role.name}@{role.server}</span>
                                                    {role.sect && (
                                                        <span className={`text-xs px-2 py-1 rounded-md font-medium flex-shrink-0 ${getSectStyle()}`}>
                                                            {role.sect}
                                                        </span>
                                                    )}
                                                    {role.equipmentScore !== undefined && role.equipmentScore !== null && (
                                                        <span className="text-xs bg-blue-100 text-blue-700 px-2 py-1 rounded-md font-medium flex-shrink-0">
                                                            {role.equipmentScore.toLocaleString()}
                                                        </span>
                                                    )}
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    <div className="flex justify-start text-sm mb-3">
                                        <div className="flex items-baseline gap-1">
                                            <span className="text-xs text-muted">本周收入:</span>
                                            <span className={`font-bold font-mono ${stats.weeklyIncome > 0 ? 'text-emerald-600' : 'text-main'}`}>
                                                {stats.weeklyIncome > 0 ? `+${stats.weeklyIncome.toLocaleString()}` : '-'}
                                                {stats.weeklyIncome > 0 && <span className="text-xs text-muted font-normal">金</span>}
                                            </span>
                                        </div>
                                    </div>

                                    {/* Account Info Section */}
                                    <div className="mt-3 pt-3 border-t border-base space-y-2">
                                        <div className="flex items-center gap-2">
                                            <div className="text-xs text-muted flex-shrink-0">账号</div>
                                            <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
                                                <span className="text-xs text-main truncate flex-1">{role.accountName}</span>
                                                <button
                                                    onClick={(e) => {
                                                        e.stopPropagation();
                                                        copyToClipboard(role.accountName, `baizhan-account-${role.id}`);
                                                    }}
                                                    className={`flex-shrink-0 p-1 rounded transition-colors ${copiedField === `baizhan-account-${role.id}`
                                                        ? 'text-emerald-600'
                                                        : 'text-muted hover:text-main hover:bg-surface'
                                                        }`}
                                                    title={copiedField === `baizhan-account-${role.id}` ? '已复制!' : '复制账号'}
                                                >
                                                    {copiedField === `baizhan-account-${role.id}` ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                                                </button>
                                            </div>
                                        </div>

                                        <div className="flex items-center gap-2">
                                            <div className="text-xs text-muted flex-shrink-0">密码</div>
                                            {(role as any).password ? (
                                                <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
                                                    <span className="text-xs text-main font-mono truncate flex-1">
                                                        {getMaskedPassword((role as any).password)}
                                                    </span>
                                                    <button
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            copyToClipboard((role as any).password, `baizhan-password-${role.id}`);
                                                        }}
                                                        className={`flex-shrink-0 p-1 rounded transition-colors ${copiedField === `baizhan-password-${role.id}`
                                                            ? 'text-emerald-600'
                                                            : 'text-muted hover:text-main hover:bg-surface'
                                                            }`}
                                                        title="复制密码"
                                                    >
                                                        {copiedField === `baizhan-password-${role.id}` ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
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
                                    <div className="mt-3 grid grid-cols-2 gap-2">
                                        <button
                                            onClick={() => handleOpenAddModal(role)}
                                            disabled={baizhanStatus === 'complete'}
                                            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${baizhanStatus === 'complete'
                                                ? 'bg-base text-muted cursor-not-allowed border border-base'
                                                : getButtonAddStyle()
                                                }`}
                                        >
                                            添加记录
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

            <AddBaizhanRecordModal
                isOpen={isAdding}
                onClose={() => setIsAdding(false)}
                onSubmit={handleAddBaizhanRecord}
                accounts={accounts}
                initialRole={selectedRole}
            />

            {
                viewRecordsRole && (
                    <BaizhanRoleRecordsModal
                        isOpen={!!viewRecordsRole}
                        onClose={() => setViewRecordsRole(null)}
                        role={viewRecordsRole}
                        records={records}
                        onDeleteRecord={handleDeleteBaizhanRecord}
                        onEditRecord={onEditRecord}
                    />
                )
            }
        </div >
    );
};
