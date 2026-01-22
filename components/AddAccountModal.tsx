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
                className="fixed inset-0 bg-black/40 z-40 transition-opacity duration-200"
                onClick={onClose}
            />
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
                <div className="bg-surface border border-base rounded-xl shadow-xl w-full max-w-md overflow-hidden pointer-events-auto animate-in fade-in zoom-in-95 duration-200">
                    <div className="px-6 py-4 flex items-center justify-between border-b border-base">
                        <div className="flex items-center gap-2">
                            <User className="w-5 h-5 text-primary" />
                            <h2 className="text-lg font-semibold text-main">新增账号</h2>
                        </div>
                        <button
                            onClick={onClose}
                            className="text-muted hover:text-main hover:bg-base p-1.5 rounded-lg transition-colors"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    <form onSubmit={handleSubmit} className="p-6 space-y-5">
                        {error && (
                            <div className="p-3 bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-lg flex items-center gap-2 text-red-600 dark:text-red-400 text-sm">
                                <AlertCircle className="w-4 h-4 shrink-0" />
                                <span>{error}</span>
                            </div>
                        )}

                        <div className="space-y-1.5">
                            <label className="block text-sm font-medium text-main ml-1">
                                账号名称 <span className="text-red-500">*</span>
                            </label>
                            <div className="relative group">
                                <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <input
                                    type="text"
                                    value={accountName}
                                    onChange={e => {
                                        setAccountName(e.target.value);
                                        if (error) setError(null);
                                    }}
                                    className="w-full pl-10 pr-4 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all placeholder:text-muted text-main"
                                    placeholder="例如：123456"
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div className="space-y-1.5">
                            <label className="block text-sm font-medium text-main ml-1">
                                账号类型
                            </label>
                            <div className="grid grid-cols-2 gap-3">
                                <button
                                    type="button"
                                    onClick={() => setType(AccountType.OWN)}
                                    className={`relative flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg border transition-all duration-200 text-sm font-medium ${type === AccountType.OWN
                                        ? 'bg-primary/10 border-primary text-primary'
                                        : 'bg-base border-base text-muted hover:bg-base/80'
                                        }`}
                                >
                                    <User className="w-4 h-4" />
                                    <span>本人账号</span>
                                </button>
                                <button
                                    type="button"
                                    onClick={() => setType(AccountType.CLIENT)}
                                    className={`relative flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg border transition-all duration-200 text-sm font-medium ${type === AccountType.CLIENT
                                        ? 'bg-emerald-50 text-emerald-600 border-emerald-200 dark:bg-emerald-900/20 dark:border-emerald-500/30'
                                        : 'bg-base border-base text-muted hover:bg-base/80'
                                        }`}
                                >
                                    <User className="w-4 h-4" />
                                    <span>代清账号</span>
                                </button>
                            </div>
                        </div>

                        <div className="pt-2 border-t border-border">
                            <div className="space-y-4">
                                <div className="space-y-1.5">
                                    <label className="block text-sm font-medium text-main ml-1">
                                        游戏密码 <span className="text-xs font-normal text-muted ml-1">(本地加密存储)</span>
                                    </label>
                                    <div className="relative group">
                                        <Key className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                        <input
                                            type="text"
                                            value={password}
                                            onChange={e => setPassword(e.target.value)}
                                            className="w-full pl-10 pr-4 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all placeholder:text-muted text-main"
                                            placeholder="可选"
                                        />
                                    </div>
                                </div>

                                <div className="space-y-1.5">
                                    <label className="block text-sm font-medium text-main ml-1">
                                        备注信息
                                    </label>
                                    <div className="relative group">
                                        <FileText className="absolute left-3 top-3 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                        <textarea
                                            value={notes}
                                            onChange={e => setNotes(e.target.value)}
                                            className="w-full pl-10 pr-4 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all resize-none placeholder:text-muted text-main"
                                            rows={2}
                                            placeholder="可选"
                                        />
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div className="flex gap-3 pt-2">
                            <button
                                type="button"
                                onClick={onClose}
                                className="flex-1 px-4 py-2.5 border border-base text-main rounded-lg font-medium hover:bg-base transition-colors active:scale-[0.98]"
                            >
                                取消
                            </button>
                            <button
                                type="submit"
                                className="flex-1 px-4 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium shadow-sm transition-all active:scale-[0.98]"
                            >
                                保存账号
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </>
    );
};
