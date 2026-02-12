import React, { useState, useMemo } from 'react';
import { TrialPlaceRecord, Account } from '../types';
import { Trophy, Check, Copy, Target } from 'lucide-react';
import { AddTrialRecordModal } from './AddTrialRecordModal';
import { TrialRoleRecordsModal } from './TrialRoleRecordsModal';

interface TrialPlaceManagerProps {
    records: TrialPlaceRecord[];
    accounts: Account[];
    onAddRecord: (record: TrialPlaceRecord) => void;
    onDeleteRecord?: (recordId: string) => void;
}

export const TrialPlaceManager: React.FC<TrialPlaceManagerProps> = ({
    records,
    accounts,
    onAddRecord,
    onDeleteRecord
}) => {
    // Filter and flat map roles
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
                    password: acc.password, // Pass password for display
                    equipmentScore: role.equipmentScore // Ensure equipmentScore is passed
                }))
            );
    }, [accounts]);

    const [selectedRole, setSelectedRole] = useState<any>(null); // For passing to modal

    // Statistics per role
    const roleStats = useMemo(() => {
        const stats = new Map<string, { weeklyCount: number, maxLayer: number, lastRunDate?: string }>();

        // Helper to get week start
        const now = new Date();
        const startOfWeek = new Date(now);
        const day = startOfWeek.getDay();
        const diff = startOfWeek.getDate() - day + (day === 0 ? -6 : 1);
        startOfWeek.setDate(diff);
        startOfWeek.setHours(0, 0, 0, 0);

        allRoles.forEach(role => {
            const roleRecords = records.filter(r => r.roleId === role.id);
            const thisWeekRecords = roleRecords.filter(r => new Date(r.date) >= startOfWeek);
            const maxLayer = roleRecords.length > 0 ? Math.max(...roleRecords.map(r => r.layer)) : 0;

            // Find last run date
            const lastRunRecord = [...roleRecords].sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime())[0];
            const lastRunDate = lastRunRecord ? lastRunRecord.date : undefined;

            stats.set(role.id, {
                weeklyCount: thisWeekRecords.length,
                maxLayer,
                lastRunDate
            });
        });
        return stats;
    }, [allRoles, records]);

    const sortedRoles = useMemo(() => {
        const sorted = [...allRoles];
        sorted.sort((a, b) => {
            const statsA = roleStats.get(a.id) || { weeklyCount: 0, maxLayer: 0, lastRunDate: undefined };
            const statsB = roleStats.get(b.id) || { weeklyCount: 0, maxLayer: 0, lastRunDate: undefined };

            // 1. Availability (canRun) - Runs < 3 first
            const aCanRun = statsA.weeklyCount < 3 ? 0 : 1;
            const bCanRun = statsB.weeklyCount < 3 ? 0 : 1;
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
                </div>

                {sortedRoles.length === 0 ? (
                    <div className="text-center py-8 text-muted bg-surface rounded-xl border border-base border-dashed">
                        请先在账号管理中添加并启用角色
                    </div>
                ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                        {sortedRoles.map(role => {
                            const stats = roleStats.get(role.id) || { weeklyCount: 0, maxLayer: 0, lastRunDate: undefined };
                            const isAtLimit = stats.weeklyCount >= 3;
                            const canRun = !isAtLimit;

                            // RaidDetail-like Card Style
                            return (
                                <div
                                    key={role.id}
                                    className={`p-4 rounded-xl border-2 transition-all duration-300 ${canRun
                                        ? 'bg-gradient-to-br from-emerald-50 to-teal-50 dark:from-emerald-900/10 dark:to-teal-900/10 border-emerald-200 dark:border-emerald-800 hover:shadow-lg hover:border-emerald-300'
                                        : 'bg-gradient-to-br from-base to-base border-base hover:shadow-md'
                                        }`}
                                >
                                    <div className="flex items-center justify-between mb-3">
                                        <div className="flex items-center gap-2 flex-1 min-w-0">
                                            {/* Status Icon */}
                                            {canRun ? (
                                                <div className="w-6 h-6 rounded-full bg-emerald-100 dark:bg-emerald-900/30 flex items-center justify-center flex-shrink-0">
                                                    <div className="w-2.5 h-2.5 rounded-full bg-emerald-500 animate-pulse" />
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
                                                            ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400'
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
                                            <Trophy className={`w-3.5 h-3.5 flex-shrink-0 ${stats.maxLayer > 0 ? 'text-amber-500' : 'text-muted'}`} />
                                            <span className="text-xs text-muted">
                                                最高层数: <span className="font-medium text-main">{stats.maxLayer > 0 ? `${stats.maxLayer}层` : '-'}</span>
                                            </span>
                                        </div>

                                        <div className="flex items-baseline gap-1">
                                            <span className="text-xs text-muted">本周进度:</span>
                                            <span className={`font-bold ${stats.weeklyCount >= 3 ? 'text-green-500' : 'text-main'}`}>
                                                {stats.weeklyCount}<span className="text-xs text-muted font-normal">/3</span>
                                            </span>
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
                                                        ? 'text-emerald-600'
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
                                            {(role as any).password ? ( // Cast to any or define password on Role if known
                                                <div className="flex items-center gap-1 flex-1 min-w-0 bg-base rounded px-2 py-1">
                                                    <span className="text-xs text-main font-mono truncate flex-1">
                                                        {getMaskedPassword((role as any).password)}
                                                    </span>
                                                    <button
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            copyToClipboard((role as any).password, `password-${role.id}`);
                                                        }}
                                                        className={`flex-shrink-0 p-1 rounded transition-colors ${copiedField === `password-${role.id}`
                                                            ? 'text-emerald-600'
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
                                    <div className="mt-3 grid grid-cols-2 gap-2">
                                        <button
                                            onClick={() => handleOpenAddModal(role)}
                                            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${canRun
                                                ? 'bg-emerald-500 text-white hover:bg-emerald-600 hover:shadow-md transform hover:-translate-y-0.5'
                                                : 'bg-base text-muted hover:bg-base/80 hover:text-main'
                                                }`}
                                        >
                                            添加记录
                                        </button>
                                        <button
                                            onClick={() => handleOpenRecordsModal(role)}
                                            className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200 ${canRun
                                                ? 'bg-white text-emerald-700 border border-emerald-200 hover:bg-emerald-50 hover:border-emerald-300'
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

            <AddTrialRecordModal
                isOpen={isAdding}
                onClose={() => setIsAdding(false)}
                onSubmit={onAddRecord}
                accounts={accounts}
                initialRole={selectedRole}
            />

            {viewRecordsRole && (
                <TrialRoleRecordsModal
                    isOpen={!!viewRecordsRole}
                    onClose={() => setViewRecordsRole(null)}
                    role={viewRecordsRole}
                    records={records}
                    onDeleteRecord={onDeleteRecord}
                />
            )}
        </div >
    );
};
