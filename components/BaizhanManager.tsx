import React, { useState, useMemo } from 'react';
import { BaizhanRecord, Account } from '../types';
import { Swords, Check, Copy, Target } from 'lucide-react';
import { AddBaizhanRecordModal } from './AddBaizhanRecordModal';
import { BaizhanRoleRecordsModal } from './BaizhanRoleRecordsModal';
import { getLastMonday7AM } from '../utils/cooldownManager';

interface BaizhanManagerProps {
    records: BaizhanRecord[];
    accounts: Account[];
    onAddRecord: (record: BaizhanRecord) => void;
    onDeleteRecord?: (recordId: string) => void;
}

export const BaizhanManager: React.FC<BaizhanManagerProps> = ({
    records,
    accounts,
    onAddRecord,
    onDeleteRecord
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
        const stats = new Map<string, { weeklyCount: number, highestTier: number, lastRunDate?: string }>();

        // 副本 CD 每周一 07:00 刷新，复用 cooldownManager 的统一函数
        const now = new Date();
        const startOfWeek = getLastMonday7AM(now);

        allRoles.forEach(role => {
            const roleRecords = records.filter(r => r.roleId === role.id);
            const thisWeekRecords = roleRecords.filter(r => new Date(r.date) >= startOfWeek);

            // Calculate highest tier based on boss tier
            const { getBossById } = require('../data/baizhanBosses');
            const tiers = roleRecords.map(r => {
                const boss = getBossById(r.bossId);
                return boss?.tier || 0;
            });
            const highestTier = tiers.length > 0 ? Math.max(...tiers) : 0;

            // Find last run date
            const lastRunRecord = [...roleRecords].sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime())[0];
            const lastRunDate = lastRunRecord ? lastRunRecord.date : undefined;

            stats.set(role.id, {
                weeklyCount: thisWeekRecords.length,
                highestTier,
                lastRunDate
            });
        });
        return stats;
    }, [allRoles, records]);

    const sortedRoles = useMemo(() => {
        const sorted = [...allRoles];
        sorted.sort((a, b) => {
            const statsA = roleStats.get(a.id) || { weeklyCount: 0, highestTier: 0, lastRunDate: undefined };
            const statsB = roleStats.get(b.id) || { weeklyCount: 0, highestTier: 0, lastRunDate: undefined };

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
        if (!password) return '•••••';
        return '•••••';
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
                            const stats = roleStats.get(role.id) || { weeklyCount: 0, highestTier: 0, lastRunDate: undefined };
                            const canRun = stats.weeklyCount === 0;

                            return (
                                <div
                                    key={role.id}
                                    className={`p-4 rounded-xl border-2 transition-all duration-300 ${canRun
                                        ? 'bg-gradient-to-br from-purple-50 to-pink-50 dark:from-purple-900/10 dark:to-pink-900/10 border-purple-200 dark:border-purple-800 hover:shadow-lg hover:border-purple-300'
                                        : 'bg-gradient-to-br from-base to-base border-base hover:shadow-md'
                                        }`}
                                >
                                    <div className="flex items-center justify-between mb-3">
                                        <div className="flex items-center gap-2 flex-1 min-w-0">
                                            {/* Status Icon */}
                                            {canRun ? (
                                                <div className="w-6 h-6 rounded-full bg-purple-100 dark:bg-purple-900/30 flex items-center justify-center flex-shrink-0">
                                                    <div className="w-2.5 h-2.5 rounded-full bg-purple-500 animate-pulse" />
                                                </div>
                                            ) : (
                                                <div className="w-6 h-6 rounded-full bg-base flex items-center justify-center flex-shrink-0">
                                                    <Check className="w-4 h-4 text-muted" />
                                                </div>
                                            )}

                                            <div className="min-w-0 flex-1">
                                                <div className="font-semibold text-main truncate flex items-center gap-2 flex-wrap">
                                                    <span className="truncate" title={`${role.name}@${role.server}`}>{role.name}@{role.server}</span>
                                                    {role.sect && (
                                                        <span className={`text-xs px-2 py-1 rounded-md font-medium flex-shrink-0 ${canRun
                                                            ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400'
                                                            : 'bg-base text-muted'
                                                            }`}>
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

                                    <div className="flex items-center justify-between text-sm mb-3">
                                        <div className="flex items-center gap-2">
                                            <Target className={`w-3.5 h-3.5 flex-shrink-0 ${stats.highestTier > 0 ? 'text-amber-500' : 'text-muted'}`} />
                                            <span className="text-xs text-muted">
                                                最高阶数: <span className="font-medium text-main">{stats.highestTier > 0 ? `${stats.highestTier}阶` : '-'}</span>
                                            </span>
                                        </div>

                                        <div className="flex items-baseline gap-1">
                                            <span className="text-xs text-muted">本周进度:</span>
                                            <span className={`font-bold ${stats.weeklyCount > 0 ? 'text-green-500' : 'text-main'}`}>
                                                {stats.weeklyCount}<span className="text-xs text-muted font-normal">次</span>
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
                                                        ? 'text-purple-600'
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
                                                            ? 'text-purple-600'
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
                                            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${canRun
                                                ? 'bg-purple-500 text-white hover:bg-purple-600 hover:shadow-md transform hover:-translate-y-0.5'
                                                : 'bg-base text-muted hover:bg-base/80 hover:text-main'
                                                }`}
                                        >
                                            添加记录
                                        </button>
                                        <button
                                            onClick={() => handleOpenRecordsModal(role)}
                                            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${canRun
                                                ? 'bg-white text-purple-700 border border-purple-200 hover:bg-purple-50 hover:border-purple-300'
                                                : 'bg-white text-amber-700 border border-amber-200 hover:bg-amber-50 hover:border-amber-300'
                                                }`}
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
                onSubmit={onAddRecord}
                accounts={accounts}
                initialRole={selectedRole}
            />

            {viewRecordsRole && (
                <BaizhanRoleRecordsModal
                    isOpen={!!viewRecordsRole}
                    onClose={() => setViewRecordsRole(null)}
                    role={viewRecordsRole}
                    records={records}
                    onDeleteRecord={onDeleteRecord}
                />
            )}
        </div>
    );
};
