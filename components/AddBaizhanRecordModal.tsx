import React, { useState, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { Account, BaizhanRecord } from '../types';
import { Save, X, AlertCircle, Swords, TrendingUp, TrendingDown, Coins } from 'lucide-react';
import { generateUUID } from '../utils/uuid';
import { toast } from '../utils/toastManager';
import { db } from '../services/db';
import { DateTimePicker } from './DateTimePicker';

interface RoleWithStatus {
    id: string;
    name: string;
    server: string;
    region: string;
    accountId: string;
    accountName: string;
    sect: string;
    equipmentScore?: number;
}

interface AddBaizhanRecordModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (record: BaizhanRecord) => void;
    accounts: Account[];
    initialRole?: RoleWithStatus;
}

export const AddBaizhanRecordModal: React.FC<AddBaizhanRecordModalProps> = ({
    isOpen,
    onClose,
    onSubmit,
    accounts,
    initialRole
}) => {
    const [selectedRoleId, setSelectedRoleId] = useState<string>(initialRole?.id || '');
    const [goldIncome, setGoldIncome] = useState<number>(0);
    const [goldExpense, setGoldExpense] = useState<number>(0);
    const [notes, setNotes] = useState<string>('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [recordDate, setRecordDate] = useState<string>('');

    const formatDateForInput = (date: Date | string): string => {
        const d = typeof date === 'string' ? new Date(date) : date;
        const year = d.getFullYear();
        const month = String(d.getMonth() + 1).padStart(2, '0');
        const day = String(d.getDate()).padStart(2, '0');
        const hours = String(d.getHours()).padStart(2, '0');
        const minutes = String(d.getMinutes()).padStart(2, '0');
        return `${year}-${month}-${day}T${hours}:${minutes}`;
    };

    const formatDateFromInput = (dateStr: string): string => {
        if (!dateStr) return new Date().toISOString();
        return new Date(dateStr).toISOString();
    };

    // Reset state when modal opens
    useEffect(() => {
        if (isOpen) {
            setSelectedRoleId(initialRole?.id || '');
            setGoldIncome(0);
            setGoldExpense(0);
            setNotes('');
            setError(null);
            setIsSubmitting(false);
            setRecordDate(formatDateForInput(new Date()));
        }
    }, [isOpen, initialRole]);

    // Lock background scroll
    useEffect(() => {
        if (isOpen) {
            const originalOverflow = document.body.style.overflow;
            document.body.style.overflow = 'hidden';
            return () => {
                document.body.style.overflow = originalOverflow;
            };
        }
    }, [isOpen]);

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



    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);

        if (!selectedRoleId) {
            setError('请选择角色');
            return;
        }
        if (goldIncome <= 0 && goldExpense <= 0) {
            setError('请填写收入或支出');
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
            const record: BaizhanRecord = {
                id: generateUUID(),
                accountId: role.accountId,
                roleId: role.id,
                roleName: role.name,
                server: `${role.region} ${role.server}`,
                date: formatDateFromInput(recordDate),
                goldIncome: goldIncome || 0,
                goldExpense: goldExpense || 0,
                type: 'baizhan',
                notes: notes.trim() || undefined
            };

            await db.addBaizhanRecord(record);
            onSubmit(record);
            toast.success('百战记录添加成功');
            onClose();
        } catch (err) {
            console.error(err);
            setError('添加失败，请重试');
        } finally {
            setIsSubmitting(false);
        }
    };

    if (!isOpen) return null;

    return createPortal(
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm overflow-hidden">
            <div className="bg-surface w-full max-w-md rounded-xl shadow-2xl border border-base overflow-hidden animate-in fade-in zoom-in duration-200 max-h-[95vh] flex flex-col">
                {/* Header */}
                <div className="flex items-center justify-between px-6 py-4 border-b border-base bg-base/50">
                    <div className="flex items-center gap-6">
                        <div className="flex items-center gap-2">
                            <div className="p-2 bg-purple-500/10 rounded-lg">
                                <Swords className="w-5 h-5 text-purple-500" />
                            </div>
                            <div>
                                <h2 className="text-lg font-bold text-main">记录百战</h2>
                                {initialRole && (
                                    <p className="text-xs text-muted mt-0.5">
                                        <span className="font-medium text-main">{initialRole.name}@{initialRole.server}</span>
                                    </p>
                                )}
                            </div>
                        </div>

                        {/* Role Selector in Header */}
                        {!initialRole && (
                            <>
                                <div className="h-6 w-px bg-base mx-2"></div>
                                <select
                                    className="pl-2 pr-8 py-1.5 rounded-lg bg-base border border-base text-sm focus:ring-2 focus:ring-purple-500/20 outline-none cursor-pointer hover:bg-base/80"
                                    value={selectedRoleId}
                                    onChange={e => setSelectedRoleId(e.target.value)}
                                >
                                    <option value="">选择挑战角色...</option>
                                    {allRoles.map(r => (
                                        <option key={r.id} value={r.id}>{r.name} @ {r.server}</option>
                                    ))}
                                </select>
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
                <form onSubmit={handleSubmit} className="p-6 space-y-5 overflow-y-auto">
                    {error && (
                        <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm flex items-center gap-2">
                            <AlertCircle className="w-4 h-4" />
                            {error}
                        </div>
                    )}

                    {/* Date Picker */}
                    <div>
                        <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-1.5">记录日期</label>
                        <div className="relative w-full">
                            <DateTimePicker
                                value={recordDate}
                                onChange={setRecordDate}
                            />
                        </div>
                    </div>

                    {/* Income & Expense */}
                    <div className="bg-base/30 rounded-xl p-4 border border-base/50 space-y-4">
                        <label className="block text-[10px] font-bold uppercase tracking-wider text-muted">收支记录</label>

                        <div className="grid grid-cols-2 gap-3">
                            {/* Income */}
                            <div>
                                <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                                    <TrendingUp className="w-4 h-4 text-emerald-600" />
                                    金币收入
                                </label>
                                <div className="relative">
                                    <Coins className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-emerald-500" />
                                    <input
                                        type="number"
                                        min="0"
                                        step="1"
                                        value={goldIncome || ''}
                                        onChange={e => setGoldIncome(parseInt(e.target.value) || 0)}
                                        placeholder="收入金额"
                                        className="w-full pl-9 pr-3 py-2.5 bg-surface border border-emerald-200 dark:border-emerald-800 rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-emerald-400 transition-all font-mono text-[1rem]"
                                    />
                                </div>
                            </div>

                            {/* Expense */}
                            <div>
                                <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                                    <TrendingDown className="w-4 h-4 text-amber-600" />
                                    金币支出
                                </label>
                                <div className="relative">
                                    <Coins className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-amber-500" />
                                    <input
                                        type="number"
                                        min="0"
                                        step="1"
                                        value={goldExpense || ''}
                                        onChange={e => setGoldExpense(parseInt(e.target.value) || 0)}
                                        placeholder="支出金额"
                                        className="w-full pl-9 pr-3 py-2.5 bg-surface border border-amber-200 dark:border-amber-800 rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-amber-400 transition-all font-mono text-[1rem]"
                                    />
                                </div>
                            </div>
                        </div>
                    </div>

                    {/* Notes */}
                    <div>
                        <label className="block text-xs font-medium text-muted mb-1.5">备注（可选）</label>
                        <textarea
                            value={notes}
                            onChange={e => setNotes(e.target.value)}
                            className="w-full p-3 rounded-lg bg-base border border-base text-sm text-main focus:ring-2 focus:ring-purple-500/20 outline-none resize-none h-20"
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
                            disabled={isSubmitting || (goldIncome <= 0 && goldExpense <= 0)}
                            className="flex-1 py-2.5 rounded-lg bg-purple-500 text-white font-bold shadow-lg shadow-purple-500/25 hover:bg-purple-600 hover:shadow-purple-500/40 active:scale-[0.98] transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
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
        </div>,
        document.body
    );
};
