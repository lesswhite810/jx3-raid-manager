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
        difficulty: 'NORMAL',
        playerCount: 25,
        isActive: true,
        notes: ''
    });
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (isOpen) {
            setNewRaid({
                name: '',
                difficulty: 'NORMAL',
                playerCount: 25,
                isActive: true,
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
            newRaid.difficulty || 'NORMAL'
        );

        if (isDuplicate) {
            setError('该副本已存在（相同名称、人数和难度）');
            return;
        }

        const raid: Raid = {
            name: newRaid.name.trim(),
            difficulty: newRaid.difficulty || 'NORMAL',
            playerCount: newRaid.playerCount || 25,
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
                className="fixed inset-0 bg-slate-900/30 backdrop-blur-sm z-40 transition-opacity duration-300"
                onClick={onClose}
            />
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
                <div className="bg-white/80 dark:bg-slate-900/80 backdrop-blur-md border border-white/20 dark:border-slate-700/50 rounded-2xl shadow-2xl w-full max-w-md overflow-hidden pointer-events-auto animate-in fade-in zoom-in-95 duration-300 ring-1 ring-black/5">
                    <div className="px-6 py-5 flex items-center justify-between border-b border-slate-200/50 dark:border-slate-700/50">
                        <div className="flex items-center gap-2">
                            <div className="p-2 bg-indigo-500/10 rounded-lg">
                                <Shield className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
                            </div>
                            <h2 className="text-xl font-bold text-slate-800 dark:text-slate-100 tracking-tight">新增副本</h2>
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
                                副本名称 <span className="text-red-500">*</span>
                            </label>
                            <div className="relative group">
                                <Shield className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400 group-focus-within:text-indigo-500 transition-colors" />
                                <input
                                    type="text"
                                    required
                                    value={newRaid.name}
                                    onChange={e => {
                                        setNewRaid({ ...newRaid, name: e.target.value });
                                        if (error) setError(null);
                                    }}
                                    className="w-full pl-10 pr-4 py-3 bg-slate-50/50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-xl focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none transition-all placeholder:text-slate-400 text-slate-800 dark:text-slate-200 shadow-sm"
                                    placeholder="例如：冷龙峰"
                                    autoFocus
                                />
                            </div>
                        </div>

                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-1.5">
                                <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 ml-1">
                                    难度
                                </label>
                                <div className="relative">
                                    <select
                                        className="w-full p-3 bg-slate-50/50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-xl focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none transition-all text-slate-800 dark:text-slate-200"
                                        value={newRaid.difficulty}
                                        onChange={e => setNewRaid({ ...newRaid, difficulty: e.target.value as any })}
                                    >
                                        <option value="NORMAL">普通</option>
                                        <option value="HEROIC">英雄</option>
                                        <option value="CHALLENGE">挑战</option>
                                    </select>
                                </div>
                            </div>
                            <div className="space-y-1.5">
                                <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 ml-1">
                                    人数
                                </label>
                                <div className="relative group">
                                    <Users className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400 z-10" />
                                    <select
                                        className="w-full pl-10 pr-4 py-3 bg-slate-50/50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-xl focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none transition-all text-slate-800 dark:text-slate-200 appearance-none"
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
                            <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 ml-1">
                                状态
                            </label>
                            <div className="grid grid-cols-2 gap-3">
                                <button
                                    type="button"
                                    onClick={() => setNewRaid({ ...newRaid, isActive: true })}
                                    className={`relative flex items-center justify-center gap-2 py-3 px-4 rounded-xl border transition-all duration-200 ${newRaid.isActive
                                        ? 'bg-emerald-50/50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-500/30 text-emerald-700 dark:text-emerald-300 font-semibold ring-1 ring-emerald-500/20 shadow-sm'
                                        : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-700'
                                        }`}
                                >
                                    启用
                                    {newRaid.isActive && <div className="absolute w-1.5 h-1.5 bg-emerald-500 rounded-full bottom-1.5" />}
                                </button>
                                <button
                                    type="button"
                                    onClick={() => setNewRaid({ ...newRaid, isActive: false })}
                                    className={`relative flex items-center justify-center gap-2 py-3 px-4 rounded-xl border transition-all duration-200 ${!newRaid.isActive
                                        ? 'bg-slate-100 dark:bg-slate-700 border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 font-semibold shadow-inner'
                                        : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-700'
                                        }`}
                                >
                                    禁用
                                </button>
                            </div>
                        </div>

                        <div className="space-y-1.5">
                            <label className="block text-sm font-semibold text-slate-700 dark:text-slate-300 ml-1">
                                备注
                            </label>
                            <div className="relative group">
                                <FileText className="absolute left-3 top-3.5 w-4 h-4 text-slate-400 group-focus-within:text-indigo-500 transition-colors" />
                                <textarea
                                    value={newRaid.notes || ''}
                                    onChange={e => setNewRaid({ ...newRaid, notes: e.target.value })}
                                    className="w-full pl-10 pr-4 py-3 bg-slate-50/50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-xl focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none transition-all resize-none text-slate-800 dark:text-slate-200 shadow-sm"
                                    rows={2}
                                    placeholder="可选备注信息"
                                />
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
                                <span className="relative">保存副本</span>
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </>
    );
};
