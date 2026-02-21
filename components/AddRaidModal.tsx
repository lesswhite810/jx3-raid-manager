import React, { useState, useEffect } from 'react';
import { Raid } from '../types';
import { X, Shield, Users, AlertCircle, FileText } from 'lucide-react';
import { isDuplicateRaid } from '../utils/raidUtils';

interface AddRaidModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSubmit: (raid: Raid) => void;
    existingRaids: Raid[];
}

export const AddRaidModal: React.FC<AddRaidModalProps> = ({
    isOpen,
    onClose,
    onSubmit,
    existingRaids
}) => {
    const [newRaid, setNewRaid] = useState<Partial<Raid>>({
        name: '',
        difficulty: '普通',
        playerCount: 25,
        isActive: true,
        version: '',
        notes: ''
    });
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (isOpen) {
            setNewRaid({
                name: '',
                difficulty: '普通',
                playerCount: 25,
                isActive: true,
                version: '',
                notes: ''
            });
            setError(null);
        }
    }, [isOpen]);

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();

        if (!newRaid.name || newRaid.name.trim() === '') {
            setError('副本名称不能为空');
            return;
        }

        if (newRaid.name.length > 50) {
            setError('副本名称不能超过50个字符');
            return;
        }

        const isDuplicate = isDuplicateRaid(
            existingRaids,
            newRaid.name.trim(),
            newRaid.playerCount || 25,
            newRaid.difficulty || '普通');

        if (isDuplicate) {
            setError('该副本已存在（相同名称、人数和难度）');
            return;
        }

        const raid: Raid = {
            name: newRaid.name.trim(),
            difficulty: newRaid.difficulty || '普通',
            playerCount: newRaid.playerCount || 25,
            version: newRaid.version?.trim() || '其他',
            notes: newRaid.notes,
            isActive: newRaid.isActive ?? true
        };

        onSubmit(raid);
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
                            <Shield className="w-5 h-5 text-primary" />
                            <h2 className="text-lg font-semibold text-main">新增副本</h2>
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
                                副本名称 <span className="text-red-500">*</span>
                            </label>
                            <div className="relative group">
                                <Shield className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <input
                                    type="text"
                                    required
                                    value={newRaid.name}
                                    onChange={e => {
                                        setNewRaid({ ...newRaid, name: e.target.value });
                                        if (error) setError(null);
                                    }}
                                    className="w-full pl-10 pr-4 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all placeholder:text-muted text-main"
                                    placeholder="例如：冷龙峰"
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-1.5">
                                <label className="block text-sm font-medium text-main ml-1">
                                    难度
                                </label>
                                <div className="relative">
                                    <select
                                        className="w-full p-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all text-main appearance-none cursor-pointer"
                                        value={newRaid.difficulty}
                                        onChange={e => setNewRaid({ ...newRaid, difficulty: e.target.value as any })}
                                    >
                                        <option value="普通">普通</option>
                                        <option value="英雄">英雄</option>
                                        <option value="挑战">挑战</option>
                                    </select>
                                </div>
                            </div>
                            <div className="space-y-1.5">
                                <label className="block text-sm font-medium text-main ml-1">
                                    人数
                                </label>
                                <div className="relative group">
                                    <Users className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted z-10" />
                                    <select
                                        className="w-full pl-10 pr-4 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all text-main appearance-none cursor-pointer"
                                        value={newRaid.playerCount}
                                        onChange={e => setNewRaid({ ...newRaid, playerCount: Number(e.target.value) as 10 | 25 })}
                                    >
                                        <option value={10}>10人</option>
                                        <option value={25}>25人</option>
                                    </select>
                                </div>
                            </div>
                        </div>

                        <div className="space-y-1.5">
                            <label className="block text-sm font-medium text-main ml-1">
                                所属资料片版本
                            </label>
                            <div className="relative group">
                                <FileText className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <input
                                    type="text"
                                    value={newRaid.version || ''}
                                    onChange={e => setNewRaid({ ...newRaid, version: e.target.value })}
                                    className="w-full pl-10 pr-4 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all placeholder:text-muted text-main"
                                    placeholder="例如：丝路风雨（留空为 其他）"
                                />
                            </div>
                        </div>

                        <div className="space-y-1.5">
                            <label className="block text-sm font-medium text-main ml-1">
                                状态
                            </label>
                            <div className="grid grid-cols-2 gap-3">
                                <button
                                    type="button"
                                    onClick={() => setNewRaid({ ...newRaid, isActive: true })}
                                    className={`relative flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg border transition-all duration-200 text-sm font-medium ${newRaid.isActive
                                        ? 'bg-primary/10 border-primary text-primary'
                                        : 'bg-base border-base text-muted hover:bg-base/80'
                                        }`}
                                >
                                    启用
                                </button>
                                <button
                                    type="button"
                                    onClick={() => setNewRaid({ ...newRaid, isActive: false })}
                                    className={`relative flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg border transition-all duration-200 text-sm font-medium ${!newRaid.isActive
                                        ? 'bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 text-slate-900 dark:text-slate-100'
                                        : 'bg-base border-base text-muted hover:bg-base/80'
                                        }`}
                                >
                                    禁用
                                </button>
                            </div>
                        </div>

                        <div className="space-y-1.5">
                            <label className="block text-sm font-medium text-main ml-1">
                                备注
                            </label>
                            <div className="relative group">
                                <FileText className="absolute left-3 top-3 w-4 h-4 text-muted group-focus-within:text-primary transition-colors" />
                                <textarea
                                    value={newRaid.notes || ''}
                                    onChange={e => setNewRaid({ ...newRaid, notes: e.target.value })}
                                    className="w-full pl-10 pr-4 py-2.5 bg-base border border-base rounded-lg focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all resize-none text-main placeholder:text-muted"
                                    rows={2}
                                    placeholder="可选备注信息"
                                />
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
                                保存副本
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </>
    );
};
