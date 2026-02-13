import React, { useState, useMemo, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { BaizhanRecord } from '../types';
import { X, Calendar, Swords, CheckCircle, Trash2, AlertCircle, Loader2 } from 'lucide-react';
import { getBossById, BAIZHAN_SKILL_COLORS } from '../data/baizhanBosses';

interface RoleDisplayData {
    id: string;
    name: string;
    server: string;
    sect?: string;
    accountName: string;
}

interface BaizhanRoleRecordsModalProps {
    isOpen: boolean;
    onClose: () => void;
    role: RoleDisplayData;
    records: BaizhanRecord[];
    onDeleteRecord?: (recordId: string) => void;
}

export const BaizhanRoleRecordsModal: React.FC<BaizhanRoleRecordsModalProps> = ({
    isOpen,
    onClose,
    role,
    records,
    onDeleteRecord
}) => {
    const [deletingRecordId, setDeletingRecordId] = useState<string | null>(null);
    const [recordToDelete, setRecordToDelete] = useState<BaizhanRecord | null>(null);
    const [showConfirmDialog, setShowConfirmDialog] = useState(false);

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

    // Sort records specific to this role
    const roleRecords = useMemo(() => {
        return records
            .filter(r => r.roleId === role.id)
            .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
    }, [records, role.id]);

    // Statistics
    const stats = useMemo(() => {
        const totalRecords = roleRecords.length;

        // Calculate highest tier
        let highestTier = 0;
        roleRecords.forEach(r => {
            const boss = getBossById(r.bossId);
            if (boss && boss.tier > highestTier) {
                highestTier = boss.tier;
            }
        });

        // Count by difficulty
        const countByDifficulty: Record<string, number> = { '3': 0, '6': 0, '10': 0 };
        roleRecords.forEach(r => {
            countByDifficulty[r.difficulty.toString()]++;
        });

        return { totalRecords, highestTier, countByDifficulty };
    }, [roleRecords]);

    const handleDeleteClick = (record: BaizhanRecord) => {
        setRecordToDelete(record);
        setShowConfirmDialog(true);
    };

    const handleDeleteConfirm = async () => {
        if (!recordToDelete || !onDeleteRecord) return;

        setDeletingRecordId(recordToDelete.id);
        setShowConfirmDialog(false);

        onDeleteRecord(recordToDelete.id);

        // Reset local state
        setDeletingRecordId(null);
        setRecordToDelete(null);
    };

    const getDifficultyLabel = (difficulty: 3 | 6 | 10) => {
        return `${difficulty}人`;
    };

    const getSkillColorDisplay = (colorValue: string) => {
        const colorConfig = BAIZHAN_SKILL_COLORS.find(c => c.value === colorValue);
        return colorConfig || null;
    };

    if (!isOpen) return null;

    return createPortal(
        <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm flex items-center justify-center z-[100] p-4 overflow-hidden">
            <div className="bg-surface rounded-2xl shadow-2xl w-full max-w-lg max-h-[85vh] overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200">
                {/* Header */}
                <div className="px-5 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm flex-shrink-0">
                    <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-full bg-purple-500/10 flex items-center justify-center text-purple-500 font-bold text-lg">
                            {role.name.charAt(0)}
                        </div>
                        <div>
                            <h2 className="text-lg font-bold text-main">{role.name}</h2>
                            <p className="text-muted text-xs mt-0.5">@{role.server} {role.sect ? `· ${role.sect}` : ''}</p>
                        </div>
                    </div>
                    <button
                        onClick={onClose}
                        className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-surface/50 active:scale-95"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Stats Summary */}
                <div className="bg-base px-5 py-3 border-b border-base flex-shrink-0">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-6">
                            <div className="text-center">
                                <div className="text-xl font-bold text-main">{stats.totalRecords}</div>
                                <div className="text-xs text-muted">总记录</div>
                            </div>
                            <div className="h-8 w-px bg-base border-r border-base/50" />
                            <div className="text-center">
                                <div className="text-xl font-bold text-amber-600">{stats.highestTier > 0 ? stats.highestTier : '-'}</div>
                                <div className="text-xs text-muted">最高阶数</div>
                            </div>
                            <div className="h-8 w-px bg-base border-r border-base/50" />
                            <div className="flex items-center gap-2 text-xs">
                                <span className="text-muted">3人: <span className="font-bold text-main">{stats.countByDifficulty['3']}</span></span>
                                <span className="text-muted">6人: <span className="font-bold text-main">{stats.countByDifficulty['6']}</span></span>
                                <span className="text-muted">10人: <span className="font-bold text-main">{stats.countByDifficulty['10']}</span></span>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Records List */}
                <div className="flex-1 overflow-y-auto p-4 scroll-smooth">
                    {roleRecords.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-16 text-muted">
                            <div className="w-16 h-16 bg-base rounded-full flex items-center justify-center mb-4">
                                <Swords className="w-8 h-8 text-muted/50" />
                            </div>
                            <p className="text-base font-medium mb-2">暂无百战记录</p>
                        </div>
                    ) : (
                        <div className="space-y-3">
                            {roleRecords.map((record, index) => {
                                const boss = getBossById(record.bossId);
                                const skillColor = getSkillColorDisplay(record.skillColor);
                                const rewards = record.rewards || [];

                                return (
                                    <div
                                        key={record.id}
                                        className="p-4 rounded-xl border-2 transition-all duration-200 bg-surface border-base hover:shadow-sm"
                                    >
                                        <div className="flex items-start justify-between gap-4">
                                            <div className="flex-1 min-w-0">
                                                {/* Header Line */}
                                                <div className="flex items-center gap-2 mb-2">
                                                    <span className="text-xs bg-base text-muted px-2 py-0.5 rounded font-bold">
                                                        #{roleRecords.length - index}
                                                    </span>
                                                    <div className="font-bold text-main flex items-center gap-2">
                                                        <Swords className="w-4 h-4 text-purple-500" />
                                                        {boss?.name || record.bossName}
                                                    </div>
                                                    <span className={`text-xs px-2 py-0.5 rounded font-medium ${record.difficulty === 3 ? 'bg-green-100 text-green-700' : record.difficulty === 6 ? 'bg-blue-100 text-blue-700' : 'bg-purple-100 text-purple-700'}`}>
                                                        {getDifficultyLabel(record.difficulty)}
                                                    </span>
                                                    <div className="text-xs text-muted flex items-center gap-1 ml-auto">
                                                        <Calendar className="w-3.5 h-3.5" />
                                                        {new Date(record.date).toLocaleDateString()}
                                                    </div>
                                                </div>

                                                {/* Content */}
                                                <div className="space-y-2 pl-1">
                                                    {/* Tier & Skill Info */}
                                                    <div className="flex items-center gap-4 text-sm">
                                                        {boss && (
                                                            <div className="text-muted">
                                                                <span className="text-xs mr-1">阶数:</span>
                                                                <span className="font-medium text-main">{boss.tier}阶</span>
                                                            </div>
                                                        )}
                                                        <div className="flex items-center gap-1">
                                                            <span className="text-xs text-muted mr-1">技能:</span>
                                                            <div
                                                                className="w-4 h-4 rounded border border-base/30"
                                                                style={{ backgroundColor: skillColor?.color || '#999' }}
                                                                title={skillColor?.name || record.skillColor}
                                                            />
                                                            <span className="font-medium text-main">{record.skillLevel.toLocaleString()}</span>
                                                        </div>
                                                    </div>

                                                    {/* Rewards */}
                                                    {rewards.length > 0 && (
                                                        <div className="bg-emerald-50 dark:bg-emerald-900/30 rounded-lg p-2 border border-emerald-200 dark:border-emerald-800">
                                                            <div className="text-xs text-emerald-700 dark:text-emerald-400 font-medium mb-1 flex items-center gap-1">
                                                                <CheckCircle className="w-3 h-3" />
                                                                获得奖励
                                                            </div>
                                                            <div className="flex flex-wrap gap-1">
                                                                {rewards.map((reward, i) => (
                                                                    <span
                                                                        key={i}
                                                                        className="text-xs px-2 py-0.5 bg-emerald-100 dark:bg-emerald-900/50 text-emerald-800 dark:text-emerald-300 rounded"
                                                                    >
                                                                        {reward}
                                                                    </span>
                                                                ))}
                                                            </div>
                                                        </div>
                                                    )}

                                                    {/* Notes */}
                                                    {record.notes && (
                                                        <div className="text-xs text-muted">
                                                            <span className="font-medium">备注: </span>{record.notes}
                                                        </div>
                                                    )}
                                                </div>
                                            </div>

                                            {/* Actions */}
                                            {onDeleteRecord && (
                                                <button
                                                    onClick={() => handleDeleteClick(record)}
                                                    disabled={deletingRecordId === record.id}
                                                    className="p-2 text-muted hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors active:scale-95"
                                                    title="删除记录"
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
                                );
                            })}
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
                    className="fixed inset-0 bg-slate-900/70 backdrop-blur-sm flex items-center justify-center z-[110] animate-in fade-in duration-200"
                    onClick={() => {
                        setShowConfirmDialog(false);
                        setRecordToDelete(null);
                    }}
                >
                    <div
                        className="bg-surface rounded-2xl shadow-2xl max-w-sm w-full mx-4 border border-base animate-in zoom-in-95 duration-200"
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
                                        确定要删除这条 <span className="font-semibold text-main">{recordToDelete?.bossName}</span> 的百战记录吗？
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
                                        <span>{recordToDelete.bossName}</span>
                                    </div>
                                </div>
                            </div>
                        )}

                        {/* Warning */}
                        <div className="px-5 pb-4">
                            <p className="text-xs text-amber-600 dark:text-amber-400 flex items-center gap-1.5">
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
                                className="px-4 py-2.5 bg-red-600 hover:bg-red-700 active:bg-red-800 text-white rounded-xl transition-all duration-200 text-sm font-medium flex items-center gap-2 cursor-pointer shadow-sm"
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
