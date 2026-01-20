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
                className="fixed inset-0 bg-slate-900/60 z-40"
                onClick={onClose}
            />
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
                <div className="bg-white rounded-xl shadow-2xl w-full max-w-md overflow-hidden pointer-events-auto animate-in fade-in zoom-in-95 duration-200">
                    <div className="bg-slate-800 px-6 py-4 flex items-center justify-between">
                        <h2 className="text-lg font-semibold text-white">新增账号</h2>
                        <button
                            onClick={onClose}
                            className="text-slate-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-slate-700"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    <form onSubmit={handleSubmit} className="p-6 space-y-4">
                        {error && (
                            <div className="p-3 bg-red-50 border border-red-200 rounded-lg flex items-center gap-2 text-red-700 text-sm">
                                <AlertCircle className="w-4 h-4" />
                                <span>{error}</span>
                            </div>
                        )}

                        <div>
                            <label className="block text-sm font-medium text-slate-700 mb-1.5 focus:text-indigo-600">
                                账号名称 <span className="text-red-500">*</span>
                            </label>
                            <div className="relative">
                                <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                                <input
                                    type="text"
                                    value={accountName}
                                    onChange={e => {
                                        setAccountName(e.target.value);
                                        if (error) setError(null);
                                    }}
                                    className="w-full pl-9 pr-3 py-2.5 border border-slate-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 outline-none transition-all placeholder:text-slate-400"
                                    placeholder="例如：ybb18829283218"
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div>
                            <label className="block text-sm font-medium text-slate-700 mb-1.5">
                                账号类型
                            </label>
                            <div className="grid grid-cols-2 gap-3">
                                <button
                                    type="button"
                                    onClick={() => setType(AccountType.OWN)}
                                    className={`flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg border transition-all ${type === AccountType.OWN
                                        ? 'bg-blue-50 border-blue-200 text-blue-700 font-medium ring-2 ring-blue-500/20'
                                        : 'border-slate-300 text-slate-600 hover:bg-slate-50'
                                        }`}
                                >
                                    <User className="w-4 h-4" />
                                    本人账号
                                </button>
                                <button
                                    type="button"
                                    onClick={() => setType(AccountType.CLIENT)}
                                    className={`flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg border transition-all ${type === AccountType.CLIENT
                                        ? 'bg-emerald-50 border-emerald-200 text-emerald-700 font-medium ring-2 ring-emerald-500/20'
                                        : 'border-slate-300 text-slate-600 hover:bg-slate-50'
                                        }`}
                                >
                                    <User className="w-4 h-4" />
                                    代清账号
                                </button>
                            </div>
                        </div>

                        <div className="pt-2 border-t border-slate-200">
                            <label className="block text-sm font-medium text-slate-700 mb-1.5">
                                游戏密码 <span className="text-xs font-normal text-slate-500">(本地存储)</span>
                            </label>
                            <div className="relative">
                                <Key className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                                <input
                                    type="text"
                                    value={password}
                                    onChange={e => setPassword(e.target.value)}
                                    className="w-full pl-9 pr-3 py-2.5 border border-slate-300 rounded-lg focus:ring-2 focus:ring-indigo-500 outline-none transition-all"
                                    placeholder="可选"
                                />
                            </div>
                        </div>

                        <div>
                            <label className="block text-sm font-medium text-slate-700 mb-1.5">
                                备注
                            </label>
                            <div className="relative">
                                <FileText className="absolute left-3 top-3 w-4 h-4 text-slate-400" />
                                <textarea
                                    value={notes}
                                    onChange={e => setNotes(e.target.value)}
                                    className="w-full pl-9 pr-3 py-2.5 border border-slate-300 rounded-lg focus:ring-2 focus:ring-indigo-500 outline-none transition-all resize-none"
                                    rows={2}
                                    placeholder="可选"
                                />
                            </div>
                        </div>

                        <div className="flex gap-3 pt-4">
                            <button
                                type="button"
                                onClick={onClose}
                                className="flex-1 px-4 py-2.5 border border-slate-300 text-slate-700 rounded-lg font-medium hover:bg-slate-50 transition-colors"
                            >
                                取消
                            </button>
                            <button
                                type="submit"
                                className="flex-1 px-4 py-2.5 bg-indigo-600 text-white rounded-lg font-medium hover:bg-indigo-700 transition-colors shadow-sm"
                            >
                                保存
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </>
    );
};
