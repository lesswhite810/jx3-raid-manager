import React, { useState, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { Account, BaizhanRecord } from '../types';
import { Save, X, AlertCircle, Calendar, Swords } from 'lucide-react';
import { generateUUID } from '../utils/uuid';
import { toast } from '../utils/toastManager';
import { db } from '../services/db';
import { getBossesByDifficulty, BAIZHAN_SKILL_COLORS, getBossById } from '../data/baizhanBosses';

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
    const [difficulty, setDifficulty] = useState<3 | 6 | 10>(3);
    const [selectedBossId, setSelectedBossId] = useState<string>('');
    const [skillColor, setSkillColor] = useState<string>('red');
    const [skillLevel, setSkillLevel] = useState<number>(10000);
    const [rewards, setRewards] = useState<string>('');
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
            setDifficulty(3);
            setSelectedBossId('');
            setSkillColor('red');
            setSkillLevel(10000);
            setRewards('');
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

    const availableBosses = getBossesByDifficulty(difficulty);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);

        if (!selectedRoleId) {
            setError('请选择角色');
            return;
        }
        if (!selectedBossId) {
            setError('请选择BOSS');
            return;
        }
        if (skillLevel < 0) {
            setError('精耐等级必须大于0');
            return;
        }

        setIsSubmitting(true);

        const role = allRoles.find(r => r.id === selectedRoleId);
        if (!role) {
            setError('找不到选中的角色');
            setIsSubmitting(false);
            return;
        }

        const boss = getBossById(selectedBossId);

        try {
            const record: BaizhanRecord = {
                id: generateUUID(),
                accountId: role.accountId,
                roleId: role.id,
                roleName: role.name,
                server: `${role.region} ${role.server}`,
                date: formatDateFromInput(recordDate),
                difficulty,
                bossId: selectedBossId,
                bossName: boss?.name || '未知BOSS',
                skillColor,
                skillLevel,
                rewards: rewards.trim() ? rewards.split('\n').filter(r => r.trim()) : undefined,
                type: 'baizhan',
                notes: notes.trim()
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
            <div className="bg-surface w-full max-w-2xl rounded-xl shadow-2xl border border-base overflow-hidden animate-in fade-in zoom-in duration-200 max-h-[95vh] flex flex-col">
                {/* Header */}
                <div className="flex items-center justify-between px-6 py-4 border-b border-base bg-base/50">
                    <div className="flex items-center gap-6">
                        <div className="flex items-center gap-2">
                            <div className="p-2 bg-purple-500/10 rounded-lg">
                                <Swords className="w-5 h-5 text-purple-500" />
                            </div>
                            <h2 className="text-lg font-bold text-main">记录百战</h2>
                        </div>

                        {/* Role Selector in Header */}
                        <div className="h-6 w-px bg-base mx-2"></div>
                        <div className="flex items-center gap-3">
                            {initialRole ? (
                                <div className="flex items-center gap-2 px-3 py-1.5 bg-base rounded-full border border-base/60 pr-4">
                                    <div className="w-6 h-6 rounded-full bg-primary/10 flex items-center justify-center text-primary font-bold text-xs">
                                        {initialRole.name.charAt(0)}
                                    </div>
                                    <div className="flex flex-col leading-none">
                                        <span className="font-bold text-main text-xs">{initialRole.name}</span>
                                        <span className="text-[10px] text-muted">{initialRole.server}</span>
                                    </div>
                                </div>
                            ) : (
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
                            )}
                        </div>
                    </div>

                    <button
                        onClick={onClose}
                        className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-base text-muted hover:text-main transition-colors"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Form */}
                <form onSubmit={handleSubmit} className="p-6 space-y-6 overflow-y-auto">
                    {error && (
                        <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm flex items-center gap-2">
                            <AlertCircle className="w-4 h-4" />
                            {error}
                        </div>
                    )}

                    {/* Difficulty & Boss Selection */}
                    <div className="bg-base/30 rounded-xl p-4 border border-base/50 space-y-4">
                        {/* Difficulty */}
                        <div>
                            <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-2">挑战难度</label>
                            <div className="flex gap-3">
                                {[3, 6, 10].map(d => (
                                    <button
                                        key={d}
                                        type="button"
                                        onClick={() => {
                                            setDifficulty(d as 3 | 6 | 10);
                                            setSelectedBossId(''); // Reset boss selection
                                        }}
                                        className={`flex-1 py-3 rounded-lg font-bold transition-all ${
                                            difficulty === d
                                                ? 'bg-purple-500 text-white shadow-lg shadow-purple-500/25'
                                                : 'bg-surface text-muted hover:bg-base hover:text-main'
                                        }`}
                                    >
                                        {d}人
                                    </button>
                                ))}
                            </div>
                        </div>

                        {/* Boss Selection */}
                        <div>
                            <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-2">挑战BOSS</label>
                            <div className="grid grid-cols-3 gap-2">
                                {availableBosses.map(boss => (
                                    <button
                                        key={boss.id}
                                        type="button"
                                        onClick={() => setSelectedBossId(boss.id)}
                                        className={`px-3 py-2 rounded-lg text-sm font-medium transition-all ${
                                            selectedBossId === boss.id
                                                ? 'bg-purple-500 text-white shadow-lg shadow-purple-500/25'
                                                : 'bg-surface text-muted hover:bg-base hover:text-main'
                                        }`}
                                    >
                                        {boss.name}
                                    </button>
                                ))}
                            </div>
                        </div>

                        {/* Date Picker */}
                        <div className="w-48">
                            <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-1.5">记录日期</label>
                            <div className="relative">
                                <Calendar className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted" />
                                <input
                                    type="datetime-local"
                                    value={recordDate}
                                    onChange={e => setRecordDate(e.target.value)}
                                    className="w-full pl-8 pr-3 py-2 rounded-lg bg-surface border border-base text-xs font-medium text-main focus:ring-2 focus:ring-purple-500/20 outline-none"
                                />
                            </div>
                        </div>
                    </div>

                    {/* Skill Color & Level */}
                    <div className="bg-base/30 rounded-xl p-4 border border-base/50">
                        <label className="block text-[10px] font-bold uppercase tracking-wider text-muted mb-2">百战技能</label>
                        <div className="flex gap-4">
                            {/* Color Selection */}
                            <div className="flex-1">
                                <div className="text-xs text-muted mb-2">技能颜色</div>
                                <div className="flex flex-wrap gap-2">
                                    {BAIZHAN_SKILL_COLORS.map(color => (
                                        <button
                                            key={color.value}
                                            type="button"
                                            onClick={() => setSkillColor(color.value)}
                                            className={`w-10 h-10 rounded-lg border-2 transition-all ${
                                                skillColor === color.value
                                                    ? 'border-purple-500 ring-2 ring-purple-500/20 scale-110'
                                                    : 'border-base hover:border-purple-300'
                                            }`}
                                            style={{ backgroundColor: color.color }}
                                            title={color.name}
                                        >
                                            {skillColor === color.value && (
                                                <div className="w-full h-full flex items-center justify-center">
                                                    <div className="w-3 h-3 rounded-full bg-white shadow-sm" />
                                                </div>
                                            )}
                                        </button>
                                    ))}
                                </div>
                            </div>

                            {/* Skill Level */}
                            <div className="flex-1">
                                <div className="text-xs text-muted mb-2">精耐等级</div>
                                <input
                                    type="number"
                                    min="0"
                                    step="100"
                                    value={skillLevel}
                                    onChange={e => setSkillLevel(parseInt(e.target.value) || 0)}
                                    className="w-full px-3 py-2 rounded-lg bg-surface border border-base text-sm font-medium text-main focus:ring-2 focus:ring-purple-500/20 outline-none"
                                    placeholder="输入精耐等级"
                                />
                            </div>
                        </div>
                    </div>

                    {/* Rewards */}
                    <div>
                        <label className="block text-xs font-medium text-muted mb-1.5">奖励（可选，每行一个）</label>
                        <textarea
                            value={rewards}
                            onChange={e => setRewards(e.target.value)}
                            className="w-full p-3 rounded-lg bg-base border border-base text-sm text-main focus:ring-2 focus:ring-purple-500/20 outline-none resize-none h-24"
                            placeholder="例如：
橙武
乌金
五行石"
                        />
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
                            disabled={isSubmitting}
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
