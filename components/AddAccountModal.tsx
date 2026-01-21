import React, { useState, useEffect } from 'react';
import { AccountType } from '../types';
import { X, User, Key, FileText, AlertCircle } from 'lucide-react';

interface AddAccountModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (data: { accountName: string; type: AccountType; password?: string; notes?: string }) => void;
    existingAccountNames: string[];
}

export const AddAccountModal: React.FC<AddAccountModalProps> = ({
    isOpen,
    onClose,
    onSubmit,
    existingAccountNames
}) => {
    const [accountName, setAccountName] = useState('');
    const [type, setType] = useState<AccountType>(AccountType.OWN);
    const [password, setPassword] = useState('');
    const [notes, setNotes] = useState('');
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (isOpen) {
            setAccountName('');
            setType(AccountType.OWN);
            setPassword('');
            setNotes('');
            setError(null);
        }
    }, [isOpen]);

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();

        if (!accountName.trim()) {
            setError('请输入账号名称');
            return;
        }

        if (existingAccountNames.includes(accountName.trim())) {
            setError('该账号名称已存在');
            return;
        }

        onSubmit({
            accountName: accountName.trim(),
            type,
            password: password.trim() || undefined,
            notes: notes.trim() || undefined
        });

        onClose();
    };

    if (!isOpen) return null;

    return (
        <>
            <div
                className="fixed inset-0 bg-slate-900/30 backdrop-blur-sm z-40 transition-opacity duration-300"
                onClick={onClose}
            />
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
                <div className="bg-white/80 dark:bg-slate-900/80 backdrop-blur-md border border-white/20 dark:border-slate-700/50 rounded-2xl shadow-2xl w-full max-w-md overflow-hidden pointer-events-auto animate-in fade-in zoom-in-95 duration-300 ring-1 ring-black/5">
                    <div className="px-6 py-5 flex items-center justify-between border-b border-slate-200/50 dark:border-slate-700/50">
                        <div className="flex items-center gap-2">
                            <div className="p-2 bg-indigo-500/10 rounded-lg">
                                <User className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
                            </div>
                            <h2 className="text-xl font-bold text-slate-800 dark:text-slate-100 tracking-tight">新增账号</h2>
                        </div>
                        <button
                            onClick={onClose}
                            className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 transition-all p-2 rounded-lg"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    <form onSubmit={handleSubmit} className="p-6 space-y-5">
                        {error && (
                            <div className="p-4 bg-red-50/50 dark:bg-red-900/10 border border-red-200/50 dark:border-red-800/30 rounded-xl flex items-center gap-3 text-red-600 dark:text-red-400 text-sm animate-in slide-in-from-top-2">
                                <AlertCircle className="w-5 h-5 shrink-0" />
                                <span className="font-medium">{error}</span>
                            </div>
                        )}

                        <div className="space-y-1.5">
                            <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 ml-1">
                                账号名称 <span className="text-red-500">*</span>
                            </label>
                            <div className="relative group">
                                <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400 group-focus-within:text-indigo-500 transition-colors" />
                                <input
                                    type="text"
                                    value={accountName}
                                    onChange={e => {
                                        setAccountName(e.target.value);
                                        if (error) setError(null);
                                    }}
                                    className="w-full pl-10 pr-4 py-3 bg-slate-50/50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-xl focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none transition-all placeholder:text-slate-400 text-slate-800 dark:text-slate-200 shadow-sm"
                                    placeholder="例如：ybb18829283218"
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div className="space-y-1.5">
                            <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 ml-1">
                                账号类型
                            </label>
                            <div className="grid grid-cols-2 gap-3">
                                <button
                                    type="button"
                                    onClick={() => setType(AccountType.OWN)}
                                    className={`relative flex items-center justify-center gap-2 py-3 px-4 rounded-xl border transition-all duration-200 overflow-hidden ${type === AccountType.OWN
                                        ? 'bg-indigo-50/50 dark:bg-indigo-900/20 border-indigo-200 dark:border-indigo-500/30 text-indigo-700 dark:text-indigo-300 font-semibold ring-1 ring-indigo-500/20 shadow-sm'
                                        : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-400 hover:border-indigo-300 dark:hover:border-slate-600 hover:bg-slate-50 dark:hover:bg-slate-700'
                                        }`}
                                >
                                    <User className="w-4 h-4" />
                                    <span>本人账号</span>
                                    {type === AccountType.OWN && (
                                        <div className="absolute w-1 h-1 bg-indigo-500 rounded-full bottom-1" />
                                    )}
                                </button>
                                <button
                                    type="button"
                                    onClick={() => setType(AccountType.CLIENT)}
                                    className={`relative flex items-center justify-center gap-2 py-3 px-4 rounded-xl border transition-all duration-200 overflow-hidden ${type === AccountType.CLIENT
                                        ? 'bg-emerald-50/50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-500/30 text-emerald-700 dark:text-emerald-300 font-semibold ring-1 ring-emerald-500/20 shadow-sm'
                                        : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-400 hover:border-emerald-300 dark:hover:border-slate-600 hover:bg-slate-50 dark:hover:bg-slate-700'
                                        }`}
                                >
                                    <User className="w-4 h-4" />
                                    <span>代清账号</span>
                                    {type === AccountType.CLIENT && (
                                        <div className="absolute w-1 h-1 bg-emerald-500 rounded-full bottom-1" />
                                    )}
                                </button>
                            </div>
                        </div>

                        <div className="pt-2 border-t border-slate-100 dark:border-slate-700/50">
                            <div className="space-y-4">
                                <div className="space-y-1.5">
                                    <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 ml-1">
                                        游戏密码 <span className="text-xs font-normal text-slate-400 ml-1">(本地加密存储)</span>
                                    </label>
                                    <div className="relative group">
                                        <Key className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400 group-focus-within:text-indigo-500 transition-colors" />
                                        <input
                                            type="text"
                                            value={password}
                                            onChange={e => setPassword(e.target.value)}
                                            className="w-full pl-10 pr-4 py-3 bg-slate-50/50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-xl focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none transition-all placeholder:text-slate-400 text-slate-800 dark:text-slate-200 shadow-sm"
                                            placeholder="可选"
                                        />
                                    </div>
                                </div>

                                <div className="space-y-1.5">
                                    <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 ml-1">
                                        备注信息
                                    </label>
                                    <div className="relative group">
                                        <FileText className="absolute left-3 top-3.5 w-4 h-4 text-slate-400 group-focus-within:text-indigo-500 transition-colors" />
                                        <textarea
                                            value={notes}
                                            onChange={e => setNotes(e.target.value)}
                                            className="w-full pl-10 pr-4 py-3 bg-slate-50/50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-xl focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none transition-all resize-none text-slate-800 dark:text-slate-200 shadow-sm"
                                            rows={2}
                                            placeholder="可选"
                                        />
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div className="flex gap-3 pt-4">
                            <button
                                type="button"
                                onClick={onClose}
                                className="flex-1 px-4 py-3 border border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-300 rounded-xl font-semibold hover:bg-slate-50 dark:hover:bg-slate-800 transition-all active:scale-[0.98]"
                            >
                                取消
                            </button>
                            <button
                                type="submit"
                                className="flex-1 px-4 py-3 bg-gradient-to-r from-indigo-600 to-indigo-500 hover:from-indigo-500 hover:to-indigo-400 text-white rounded-xl font-semibold shadow-lg shadow-indigo-500/20 hover:shadow-indigo-500/30 transition-all active:scale-[0.98] flex items-center justify-center gap-2"
                            >
                                <span className="relative">保存账号</span>
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </>
    );
};
