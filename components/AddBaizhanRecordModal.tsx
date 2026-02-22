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
    initialData?: BaizhanRecord;
}

export const AddBaizhanRecordModal: React.FC<AddBaizhanRecordModalProps> = ({
    isOpen,
    onClose,
    onSubmit,
    accounts,
    initialRole,
    initialData
}) => {
    const [selectedRoleId, setSelectedRoleId] = useState<string>(initialData?.roleId || initialRole?.id || '');
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
            if (initialData) {
                setSelectedRoleId(initialData.roleId);
                setGoldIncome(initialData.goldIncome);
                setGoldExpense(initialData.goldExpense || 0);
                setNotes(initialData.notes || '');
                setRecordDate(formatDateForInput(initialData.date));
            } else {
                setSelectedRoleId(initialRole?.id || '');
                setGoldIncome(0);
                setGoldExpense(0);
                setNotes('');
                setRecordDate(formatDateForInput(new Date()));
            }
            setError(null);
            setIsSubmitting(false);
        }
    }, [isOpen, initialRole, initialData]);

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
                id: initialData ? initialData.id : generateUUID(),
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
            toast.success(initialData ? '百战记录修改成功' : '百战记录添加成功');
            onClose();
        } catch (err) {
            console.error(err);
            setError(initialData ? '修改失败，请重试' : '添加失败，请重试');
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
                            <div className="p-2 bg-slate-100 dark:bg-slate-800/50 rounded-xl border border-slate-200/50 dark:border-slate-700/50">
                                <Swords className="w-5 h-5 text-purple-500/80" />
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
                        {!initialRole && !initialData && (
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
                        {initialData && !initialRole && (
                            <>
                                <div className="h-6 w-px bg-base mx-2"></div>
                                <div className="text-sm font-medium text-main">
                                    {initialData.roleName} @ {initialData.server}
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
                <form onSubmit={handleSubmit} className="p-6 space-y-5 overflow-y-auto">
                    {error && (
                        <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm flex items-center gap-2">
                            <AlertCircle className="w-4 h-4" />
                            {error}
                        </div>
                    )}

                    {/* Date Picker */}
                    <div>
                        <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4 text-primary"><rect width="18" height="18" x="3" y="4" rx="2" ry="2" /><line x1="16" x2="16" y1="2" y2="6" /><line x1="8" x2="8" y1="2" y2="6" /><line x1="3" x2="21" y1="10" y2="10" /></svg>
                            记录日期
                        </label>
                        <div className="relative w-full">
                            <DateTimePicker
                                value={recordDate}
                                onChange={setRecordDate}
                            />
                        </div>
                    </div>

                    {/* Income & Expense */}
                    <div className="space-y-3">

                        <div className="grid grid-cols-2 gap-3">
                            {/* Income */}
                            <div>
                                <label className="flex items-center gap-1.5 text-xs font-bold text-slate-700 dark:text-slate-300 mb-1.5">
                                    <TrendingUp className="w-4 h-4 text-emerald-500/80" />
                                    金币收入
                                </label>
                                <div className="relative">
                                    <Coins className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-emerald-500/60" />
                                    <input
                                        type="number"
                                        min="0"
                                        step="1"
                                        value={goldIncome || ''}
                                        onChange={e => setGoldIncome(parseInt(e.target.value) || 0)}
                                        placeholder="收入金额"
                                        className="w-full pl-9 pr-3 py-2.5 bg-slate-50 dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-xl text-main placeholder:text-muted/50 focus:outline-none focus:ring-2 focus:ring-emerald-500/30 transition-all font-mono text-[1rem]"
                                    />
                                </div>
                            </div>

                            {/* Expense */}
                            <div>
                                <label className="flex items-center gap-1.5 text-xs font-bold text-slate-700 dark:text-slate-300 mb-1.5">
                                    <TrendingDown className="w-4 h-4 text-amber-500/80" />
                                    金币支出
                                </label>
                                <div className="relative">
                                    <Coins className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-amber-500/60" />
                                    <input
                                        type="number"
                                        min="0"
                                        step="1"
                                        value={goldExpense || ''}
                                        onChange={e => setGoldExpense(parseInt(e.target.value) || 0)}
                                        placeholder="支出金额"
                                        className="w-full pl-9 pr-3 py-2.5 bg-slate-50 dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-xl text-main placeholder:text-muted/50 focus:outline-none focus:ring-2 focus:ring-amber-500/30 transition-all font-mono text-[1rem]"
                                    />
                                </div>
                            </div>
                        </div>
                    </div>


                    {/* Notes */}
                    <div>
                        <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-4 h-4 text-primary"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>
                            备注（可选）
                        </label>
                        <textarea
                            value={notes}
                            onChange={e => setNotes(e.target.value)}
                            className="w-full p-3 rounded-lg bg-base border border-base text-sm text-main focus:ring-2 focus:ring-primary/30 outline-none resize-none h-20"
                            placeholder="记录一些细节..."
                        />
                    </div>

                    {/* Actions */}
                    <div className="flex gap-3 pt-4 border-t border-base mt-2">
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
                            className="flex-1 py-2.5 rounded-lg bg-primary text-white font-bold shadow-lg shadow-primary/25 hover:bg-primary-hover hover:shadow-primary/40 active:scale-[0.98] transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                        >
                            {isSubmitting ? (
                                <>
                                    <span className="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                                    保存中...
                                </>
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
