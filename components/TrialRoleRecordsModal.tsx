import React, { useState, useMemo, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { TrialPlaceRecord } from '../types';
import { X, Calendar, Trophy, Search, CheckCircle, Trash2, AlertCircle, Loader2 } from 'lucide-react';
import { db } from '../services/db';
import { JX3Equip } from '../services/jx3BoxApi';

interface RoleDisplayData {
    id: string;
    name: string;
    server: string;
    sect?: string;
    accountName: string;
}

interface TrialRoleRecordsModalProps {
    isOpen: boolean;
    onClose: () => void;
    role: RoleDisplayData;
    records: TrialPlaceRecord[];
    onDeleteRecord?: (recordId: string) => void;
}

// 格式化属性的辅助函数
const getFormattedAttributes = (item: JX3Equip) => {
    const attrs: { label: string; color?: string }[] = [];
    if (item.attributes && Array.isArray(item.attributes)) {
        item.attributes.forEach((attr: any) => {
            let name = item.AttributeTypes?.[attr.type];
            if (!name && attr.label) {
                name = attr.label.replace(/提高.*$/, '').replace(/[0-9]+$/, '');
            }
            if (name) {
                name = name.replace(/等级$|值$/, '').replace(/^外功|^内功/, '');
                if (name === '会心效果') name = '会效';
                if (name === '治疗成效') name = '治疗';
                attrs.push({
                    label: name,
                    color: (attr.color && attr.color.toLowerCase() !== '#ffffff' && attr.color.toLowerCase() !== 'white')
                        ? attr.color : undefined
                });
            }
        });
    }
    return attrs;
};

// 获取绑定类型标签
const getBindTypeLabel = (val: any) => {
    switch (Number(val)) {
        case 1: return '不绑定';
        case 2: return '装绑';
        case 3: return '拾绑';
        default: return null;
    }
};

export const TrialRoleRecordsModal: React.FC<TrialRoleRecordsModalProps> = ({
    isOpen,
    onClose,
    role,
    records,
    onDeleteRecord
}) => {
    const [deletingRecordId, setDeletingRecordId] = useState<string | null>(null);
    const [recordToDelete, setRecordToDelete] = useState<TrialPlaceRecord | null>(null);
    const [showConfirmDialog, setShowConfirmDialog] = useState(false);
    const [equipments, setEquipments] = useState<JX3Equip[]>([]);

    // 加载装备数据
    useEffect(() => {
        if (isOpen) {
            db.getEquipments().then((data: any[]) => {
                setEquipments(data.map(d => typeof d === 'string' ? JSON.parse(d) : d));
            }).catch(console.error);
        }
    }, [isOpen]);

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

    // 根据 ID 查找装备
    const findEquipmentById = (id: string | undefined): JX3Equip | null => {
        if (!id || !id.trim()) return null;
        return equipments.find(e => e.ID?.toString() === id) || null;
    };

    // Sort records specific to this role
    const roleRecords = useMemo(() => {
        return records
            .filter(r => r.roleId === role.id)
            .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
    }, [records, role.id]);

    const maxLayer = roleRecords.length > 0 ? Math.max(...roleRecords.map(r => r.layer)) : 0;

    const handleDeleteClick = (record: TrialPlaceRecord) => {
        setRecordToDelete(record);
        setShowConfirmDialog(true);
    };

    const handleDeleteConfirm = async () => {
        if (!recordToDelete || !onDeleteRecord) return;

        setDeletingRecordId(recordToDelete.id);
        setShowConfirmDialog(false); // Close dialog immediately

        // Simulate async delay if needed or just call
        // In a real app we might wait for promise, but here we just trigger
        onDeleteRecord(recordToDelete.id);

        // Reset local state
        setDeletingRecordId(null);
        setRecordToDelete(null);
    };

    if (!isOpen) return null;

    return createPortal(
        <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm flex items-center justify-center z-[100] p-4 overflow-hidden">
            <div className="bg-surface rounded-2xl shadow-2xl w-full max-w-lg max-h-[85vh] overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200">
                {/* Header */}
                <div className="px-5 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm flex-shrink-0">
                    <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center text-primary font-bold text-lg">
                            {role.name.charAt(0)}
                        </div>
                        <div>
                            <h2 className="text-lg font-bold text-main">{role.name}</h2>
                            <p className="text-muted text-xs mt-0.5">@{role.server} {role.sect ? `· ${role.sect}` : ''}</p>
                        </div>
                    </div>
                    <button
                        onClick={onClose}
                        className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50 active:scale-95"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Stats Summary */}
                <div className="bg-base px-5 py-3 border-b border-base flex-shrink-0">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-6">
                            <div className="text-center">
                                <div className="text-xl font-bold text-main">{roleRecords.length}</div>
                                <div className="text-xs text-muted">总记录</div>
                            </div>
                            <div className="h-8 w-px bg-base border-r border-base/50" />
                            <div className="text-center">
                                <div className="text-xl font-bold text-amber-600">{maxLayer > 0 ? maxLayer : '-'}</div>
                                <div className="text-xs text-muted">最高层数</div>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Records List */}
                <div className="flex-1 overflow-y-auto p-4 scroll-smooth">
                    {roleRecords.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-16 text-muted">
                            <div className="w-16 h-16 bg-base rounded-full flex items-center justify-center mb-4">
                                <Search className="w-8 h-8 text-muted/50" />
                            </div>
                            <p className="text-base font-medium mb-2">暂无记录</p>
                        </div>
                    ) : (
                        <div className="space-y-3">
                            {roleRecords.map((record, index) => (
                                <div
                                    key={record.id}
                                    className={`p-4 rounded-xl border-2 transition-all duration-200 bg-surface border-base hover:shadow-sm`}
                                >
                                    <div className="flex items-start justify-between gap-4">
                                        <div className="flex-1 min-w-0">
                                            {/* Header Line */}
                                            <div className="flex items-center gap-2 mb-2">
                                                <span className="text-xs bg-base text-muted px-2 py-0.5 rounded font-bold">
                                                    #{roleRecords.length - index}
                                                </span>
                                                <div className="font-bold text-main flex items-center gap-2">
                                                    <Trophy className="w-4 h-4 text-amber-500" />
                                                    {record.layer}层
                                                </div>
                                                <div className="text-xs text-muted flex items-center gap-1 ml-auto">
                                                    <Calendar className="w-3.5 h-3.5" />
                                                    {new Date(record.date).toLocaleDateString()}
                                                </div>
                                            </div>

                                            {/* Content */}
                                            <div className="space-y-2 pl-1">
                                                <div className="text-sm text-main">
                                                    <span className="text-muted text-xs mr-2">Boss:</span>
                                                    {record.bosses.join(', ')}
                                                </div>

                                                {/* Cards Info */}
                                                <div className="bg-base/50 rounded-lg p-2 text-xs flex items-center gap-4 flex-wrap">
                                                    <div>
                                                        <span className="text-muted block mb-0.5">翻牌</span>
                                                        <span className="font-mono font-bold text-blue-600">{record.flippedIndex}</span>
                                                    </div>

                                                    {/* Dropped Equipment (Only show when there's a valid equip ID) */}
                                                    {(() => {
                                                        const equipId = (record as any)[`card${record.flippedIndex}`];
                                                        const equip = findEquipmentById(equipId);
                                                        if (!equip) return null;

                                                        const attrs = getFormattedAttributes(equip);
                                                        const iconUrl = equip.IconID ? `https://icon.jx3box.com/icon/${equip.IconID}.png` : null;
                                                        const bindLabel = getBindTypeLabel(equip.BindType);

                                                        return (
                                                            <div className="flex-1 flex justify-end">
                                                                <div className="flex items-center gap-2 text-emerald-600 bg-emerald-50 dark:bg-emerald-900/30 px-2 py-1.5 rounded border border-emerald-200 dark:border-emerald-800">
                                                                    <CheckCircle className="w-3.5 h-3.5 flex-shrink-0" />
                                                                    {iconUrl && (
                                                                        <div className="w-7 h-7 rounded border border-gray-600 bg-[#1a1a2e] overflow-hidden flex-shrink-0">
                                                                            <img src={iconUrl} alt="" className="w-full h-full object-cover" />
                                                                        </div>
                                                                    )}
                                                                    <div className="flex flex-col min-w-0">
                                                                        <span className="font-medium text-emerald-700 dark:text-emerald-400 truncate text-sm">
                                                                            {equip.Name}
                                                                        </span>
                                                                        <div className="flex items-center gap-1.5 text-xs text-muted">
                                                                            {bindLabel && <span className="text-amber-600">{bindLabel}</span>}
                                                                            <span>品级 {equip.Level}</span>
                                                                        </div>
                                                                        {attrs.length > 0 && (
                                                                            <div className="flex flex-wrap gap-1 mt-0.5">
                                                                                {attrs.slice(0, 4).map((a, i) => (
                                                                                    <span
                                                                                        key={i}
                                                                                        className="px-1 py-0.5 rounded bg-base/80 text-[10px]"
                                                                                        style={a.color ? { color: a.color } : undefined}
                                                                                    >
                                                                                        {a.label}
                                                                                    </span>
                                                                                ))}
                                                                            </div>
                                                                        )}
                                                                    </div>
                                                                </div>
                                                            </div>
                                                        );
                                                    })()}
                                                </div>
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
                            ))}
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
                                        确定要删除这条 <span className="font-semibold text-main">{recordToDelete?.layer}层</span> 的试炼记录吗？
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
                                        <span>Boss: {recordToDelete.bosses.join(', ')}</span>
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
