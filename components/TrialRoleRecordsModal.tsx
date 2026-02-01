import React, { useState, useMemo } from 'react';
import { TrialPlaceRecord } from '../types';
import { X, Calendar, Trophy, Search, CheckCircle, Trash2, AlertCircle, Loader2 } from 'lucide-react';

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

    return (
        <div className="fixed inset-0 bg-slate-900/60 flex items-center justify-center z-50 p-4">
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
                                                        <span className="text-muted block mb-0.5">精简</span>
                                                        <span className="font-mono font-bold text-amber-600">
                                                            {record.cards.jingJianIndices && record.cards.jingJianIndices.length > 0
                                                                ? record.cards.jingJianIndices.join(', ')
                                                                // @ts-ignore legacy support
                                                                : (record.cards.jingJianIndex ?? '-')}
                                                        </span>
                                                    </div>
                                                    <div>
                                                        <span className="text-muted block mb-0.5">翻牌</span>
                                                        <span className="font-mono font-bold text-blue-600">{record.cards.flippedIndex}</span>
                                                    </div>
                                                    {/* Hit Status */}
                                                    {((record.cards.jingJianIndices && record.cards.jingJianIndices.includes(record.cards.flippedIndex)) ||
                                                        // @ts-ignore legacy check
                                                        (record.cards.jingJianIndex === record.cards.flippedIndex)) && (
                                                            <div className="flex-1 flex justify-end">
                                                                <div className="flex items-center gap-1 text-emerald-600 font-bold bg-emerald-50 px-2 py-1 rounded">
                                                                    <CheckCircle className="w-3.5 h-3.5" />
                                                                    命中!
                                                                    {record.cards.droppedEquipment && (
                                                                        <span className="text-emerald-500 font-normal ml-1">
                                                                            - {record.cards.droppedEquipment}
                                                                        </span>
                                                                    )}
                                                                </div>
                                                            </div>
                                                        )}
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

            {/* Delete Confirmation */}
            {showConfirmDialog && (
                <div className="fixed inset-0 bg-slate-900/70 flex items-center justify-center z-[60] animate-in fade-in zoom-in-95">
                    <div className="bg-surface p-6 rounded-2xl shadow-2xl max-w-sm w-full mx-4">
                        <div className="flex items-center gap-3 mb-4">
                            <div className="w-12 h-12 bg-red-100 dark:bg-red-900/20 rounded-full flex items-center justify-center flex-shrink-0">
                                <AlertCircle className="w-6 h-6 text-red-600 dark:text-red-400" />
                            </div>
                            <div>
                                <h3 className="text-lg font-semibold text-main">确认删除</h3>
                                <p className="text-sm text-muted">此操作不可恢复</p>
                            </div>
                        </div>
                        <p className="text-sm text-muted mb-6">确定要删除这条试炼记录吗？</p>
                        <div className="flex justify-end gap-3">
                            <button
                                onClick={() => {
                                    setShowConfirmDialog(false);
                                    setRecordToDelete(null);
                                }}
                                className="px-4 py-2 border border-base text-main rounded-lg hover:bg-base transition-colors text-sm font-medium"
                            >
                                取消
                            </button>
                            <button
                                onClick={handleDeleteConfirm}
                                className="px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors text-sm font-medium"
                            >
                                确认删除
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
