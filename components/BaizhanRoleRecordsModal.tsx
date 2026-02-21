import React, { useState, useMemo, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { BaizhanRecord } from '../types';
import { X, Calendar, Swords, Trash2, AlertCircle, Loader2, TrendingUp, TrendingDown, Wallet, Info } from 'lucide-react';

// 格式化金币显示（对齐 RoleRecordsModal 风格）
const formatGoldAmount = (amount: number): string => {
    if (amount >= 10000) {
        return `${(amount / 10000).toFixed(1)}w`;
    }
    return amount.toLocaleString();
};

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

    // 角色记录列表（日期降序）
    const roleRecords = useMemo(() => {
        return records
            .filter(r => r.roleId === role.id)
            .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
    }, [records, role.id]);

    // 汇总统计
    const stats = useMemo(() => {
        const totalIncome = roleRecords.reduce((sum, r) => sum + (r.goldIncome || 0), 0);
        const totalExpense = roleRecords.reduce((sum, r) => sum + (r.goldExpense || 0), 0);
        const netGold = totalIncome - totalExpense;
        return { totalRecords: roleRecords.length, totalIncome, totalExpense, netGold };
    }, [roleRecords]);

    const formatDate = useCallback((dateString: string) => {
        const date = new Date(dateString);
        return date.toLocaleDateString('zh-CN', {
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit'
        });
    }, []);

    const handleDeleteClick = useCallback((record: BaizhanRecord) => {
        setRecordToDelete(record);
        setShowConfirmDialog(true);
    }, []);

    const handleDeleteConfirm = useCallback(async () => {
        if (!recordToDelete || !onDeleteRecord) return;

        setDeletingRecordId(recordToDelete.id);
        setShowConfirmDialog(false);
        onDeleteRecord(recordToDelete.id);
        setDeletingRecordId(null);
        setRecordToDelete(null);
    }, [recordToDelete, onDeleteRecord]);

    const handleDeleteCancel = useCallback(() => {
        setShowConfirmDialog(false);
        setRecordToDelete(null);
    }, []);

    const handleClose = useCallback(() => {
        if (deletingRecordId) return;
        onClose();
    }, [deletingRecordId, onClose]);

    if (!isOpen) return null;

    return createPortal(
        <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm flex items-center justify-center z-[100] p-4 overflow-hidden">
            <div className="bg-surface rounded-2xl shadow-2xl w-full max-w-lg max-h-[85vh] overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200">
                {/* Header - 对齐团本风格 */}
                <div className="px-6 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm flex-shrink-0">
                    <div>
                        <h2 className="text-lg font-bold text-main">百战记录详情</h2>
                        <p className="text-muted text-xs mt-0.5">
                            <span className="font-medium text-main">{role.name}@{role.server}</span>
                            {role.sect && (
                                <>
                                    <span className="mx-1.5 text-muted/40">·</span>
                                    {role.sect}
                                </>
                            )}
                        </p>
                    </div>
                    <button
                        onClick={handleClose}
                        className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50 active:scale-95"
                        aria-label="关闭"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Stats - 对齐团本行内图标+数字风格 */}
                <div className="bg-base px-5 py-3 border-b border-base flex-shrink-0">
                    <div className="flex items-start justify-between gap-3">
                        <div className="flex flex-col gap-2">
                            <div className="flex items-center gap-4">
                                <div className="flex items-center gap-1.5" title="总收入">
                                    <TrendingUp className="w-4 h-4 text-emerald-600 dark:text-emerald-500 flex-shrink-0" />
                                    <span className="text-[1rem] font-bold text-emerald-600 dark:text-emerald-500">{formatGoldAmount(stats.totalIncome)}</span>
                                </div>
                                <div className="flex items-center gap-1.5" title="总支出">
                                    <TrendingDown className="w-4 h-4 text-amber-600 dark:text-amber-500 flex-shrink-0" />
                                    <span className="text-[1rem] font-bold text-amber-600 dark:text-amber-500">{formatGoldAmount(stats.totalExpense)}</span>
                                </div>
                                <div className="flex items-center gap-1.5" title="净收入">
                                    <Wallet className="w-4 h-4 text-muted flex-shrink-0" />
                                    <span className={`text-[1rem] font-bold ${stats.netGold >= 0 ? 'text-emerald-600 dark:text-emerald-500' : 'text-amber-600 dark:text-amber-500'}`}>
                                        {formatGoldAmount(stats.netGold)}
                                    </span>
                                </div>
                            </div>
                        </div>

                        <div className="flex flex-col flex-shrink-0">
                            <div className="flex items-center justify-center bg-surface border border-base px-3 py-1.5 rounded-md shadow-sm">
                                <span className="text-sm font-bold text-main">{stats.totalRecords} <span className="text-muted text-xs font-normal">次</span></span>
                            </div>
                        </div>
                    </div>
                </div>

                {/* 记录列表 - 对齐团本卡片风格 */}
                <div className="flex-1 overflow-y-auto p-4 scroll-smooth">
                    {roleRecords.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-16 text-muted">
                            <div className="w-16 h-16 bg-base rounded-full flex items-center justify-center mb-4">
                                <Swords className="w-8 h-8 text-muted/50" />
                            </div>
                            <p className="text-[1rem] font-medium mb-2">暂无百战记录</p>
                        </div>
                    ) : (
                        <div className="space-y-3">
                            {roleRecords.map((record) => {
                                const income = record.goldIncome || 0;
                                const expense = record.goldExpense || 0;

                                return (
                                    <div
                                        key={record.id}
                                        className={`p-4 rounded-xl border-2 border-base bg-surface transition-all duration-200 hover:shadow-md ${deletingRecordId === record.id ? 'opacity-50' : ''}`}
                                    >
                                        <div className="flex items-start justify-between gap-4">
                                            <div className="flex-1 min-w-0">
                                                {/* 编号 + 日期 + 收支数字 - 行内标签风格 */}
                                                <div className="flex items-center gap-3 mb-2 flex-wrap">

                                                    <div className="flex items-center gap-1.5 text-xs text-muted">
                                                        <Calendar className="w-3.5 h-3.5 flex-shrink-0" />
                                                        <span className="font-medium">{formatDate(record.date)}</span>
                                                    </div>

                                                    {income > 0 && (
                                                        <div className="flex items-center gap-1" title="收入">
                                                            <TrendingUp className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-500 flex-shrink-0" />
                                                            <span className="text-sm font-semibold text-emerald-600 dark:text-emerald-500">{formatGoldAmount(income)}</span>
                                                        </div>
                                                    )}
                                                    {expense > 0 && (
                                                        <div className="flex items-center gap-1" title="支出">
                                                            <TrendingDown className="w-3.5 h-3.5 text-amber-600 dark:text-amber-500 flex-shrink-0" />
                                                            <span className="text-sm font-semibold text-amber-600 dark:text-amber-500">{formatGoldAmount(expense)}</span>
                                                        </div>
                                                    )}
                                                </div>

                                                {/* 备注 - Info 图标 + 背景卡片风格 */}
                                                {record.notes && (
                                                    <div className="flex items-start gap-1.5 text-xs text-muted bg-base/50 p-2 rounded-lg">
                                                        <Info className="w-3.5 h-3.5 flex-shrink-0 mt-0.5 text-muted/70" />
                                                        <p className="flex-1 break-words leading-relaxed">{record.notes}</p>
                                                    </div>
                                                )}
                                            </div>

                                            {/* 删除按钮 */}
                                            {onDeleteRecord && (
                                                <div className="flex flex-row items-center gap-1.5 flex-shrink-0">
                                                    <button
                                                        onClick={() => handleDeleteClick(record)}
                                                        disabled={deletingRecordId === record.id}
                                                        className="p-1.5 rounded-lg text-muted hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 active:scale-95 transition-all duration-200"
                                                        title="删除记录"
                                                    >
                                                        {deletingRecordId === record.id ? (
                                                            <Loader2 className="w-4 h-4 animate-spin" />
                                                        ) : (
                                                            <Trash2 className="w-4 h-4" />
                                                        )}
                                                    </button>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </div>

                {/* Footer - 对齐团本风格 */}
                <div className="bg-base px-5 py-3 border-t border-base flex-shrink-0">
                    <button
                        onClick={handleClose}
                        disabled={!!deletingRecordId}
                        className="w-full py-2.5 border border-base text-main rounded-xl font-medium hover:bg-surface active:bg-base transition-all duration-200 flex items-center justify-center gap-2 text-sm disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        <X className="w-4 h-4" />
                        关闭
                    </button>
                </div>
            </div>

            {/* 删除确认弹窗 - 对齐团本风格 */}
            {showConfirmDialog && recordToDelete && (
                <div className="fixed inset-0 bg-slate-900/70 flex items-center justify-center z-[110] animate-in fade-in duration-200">
                    <div className="bg-surface p-6 rounded-2xl shadow-2xl max-w-sm w-full mx-4 animate-in zoom-in-95 duration-200">
                        <div className="flex items-center gap-3 mb-4">
                            <div className="w-12 h-12 bg-red-100 dark:bg-red-900/20 rounded-full flex items-center justify-center flex-shrink-0">
                                <AlertCircle className="w-6 h-6 text-red-600 dark:text-red-400" />
                            </div>
                            <div className="flex-1 min-w-0">
                                <h3 className="text-lg font-semibold text-main">确认删除</h3>
                                <p className="text-sm text-muted mt-0.5">此操作不可恢复</p>
                            </div>
                        </div>
                        <p className="text-sm text-muted mb-5">确定要删除此百战记录吗？</p>
                        <div className="flex justify-end gap-3">
                            <button
                                onClick={handleDeleteCancel}
                                className="px-5 py-2.5 border border-base text-main rounded-xl font-medium hover:bg-base transition-all duration-200 text-sm cursor-pointer"
                            >
                                取消
                            </button>
                            <button
                                onClick={handleDeleteConfirm}
                                className="px-5 py-2.5 bg-red-600 text-white rounded-xl font-medium hover:bg-red-700 active:bg-red-800 transition-all duration-200 flex items-center gap-2 text-sm cursor-pointer"
                            >
                                <Trash2 className="w-4 h-4" />
                                删除
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>,
        document.body
    );
};
