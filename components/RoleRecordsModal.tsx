import React, { useState, useMemo, useCallback } from 'react';
import { RaidRecord, Raid } from '../types';
import { X, Search, Calendar, Sparkles, Trash2, CheckCircle, AlertCircle, Loader2, ArrowDownToLine, ArrowUpFromLine, Wallet, Info } from 'lucide-react';
import { formatGoldAmount } from '../utils/recordUtils';

interface RoleWithStatus {
  id: string;
  name: string;
  server: string;
  region: string;
  sect: string;
  accountId: string;
  accountName: string;
}

interface RoleRecordsModalProps {
  isOpen: boolean;
  onClose: () => void;
  role: RoleWithStatus;
  records: RaidRecord[];
  raid: Raid;
  setRecords?: React.Dispatch<React.SetStateAction<RaidRecord[]>>;
  currentUserId?: string;
  isAdmin?: boolean;
}

export const RoleRecordsModal: React.FC<RoleRecordsModalProps> = ({
  isOpen,
  onClose,
  role,
  records,
  raid,
  setRecords,
  currentUserId,
  isAdmin = false
}) => {
  const [deletingRecordId, setDeletingRecordId] = useState<string | null>(null);
  const [showConfirmDialog, setShowConfirmDialog] = useState(false);
  const [recordToDelete, setRecordToDelete] = useState<RaidRecord | null>(null);
  const [showSuccess, setShowSuccess] = useState(false);
  const [showError, setShowError] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');

  const isTenPerson = raid.playerCount === 10;
  const maxRecords = isTenPerson ? 2 : 1;

  const weekInfo = useMemo(() => {
    const now = new Date();
    const dayOfWeek = now.getDay();
    const mondayOffset = dayOfWeek === 0 ? -6 : 1 - dayOfWeek;
    const weekStart = new Date(now);
    weekStart.setDate(now.getDate() + mondayOffset);
    weekStart.setHours(0, 0, 0, 0);
    
    const weekEnd = new Date(weekStart);
    weekEnd.setDate(weekStart.getDate() + 6);
    weekEnd.setHours(23, 59, 59, 999);
    
    return {
      start: weekStart,
      end: weekEnd
    };
  }, []);

  const roleRecords = useMemo(() => {
    const safeRecords = Array.isArray(records) ? records : [];
    const weekStartTime = weekInfo.start.getTime();
    const weekEndTime = weekInfo.end.getTime();
    
    const difficultyText = raid.difficulty === 'HEROIC' ? '英雄' : raid.difficulty === 'CHALLENGE' ? '挑战' : '普通';
    
    return safeRecords
      .filter(r => {
        const matchesRole = r.roleId === role.id;
        const matchesName = r.raidName.includes(raid.name);
        const matchesDifficulty = r.raidName.includes(difficultyText);
        const matchesPlayerCount = r.raidName.includes(`${raid.playerCount}人`);
        const recordTime = new Date(r.date).getTime();
        const matchesTimeRange = recordTime >= weekStartTime && recordTime <= weekEndTime;
        
        return matchesRole && matchesName && matchesDifficulty && matchesPlayerCount && matchesTimeRange;
      })
      .sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
  }, [records, role.id, raid, weekInfo]);

  const totalIncome = roleRecords.reduce((sum, r) => sum + (r.goldIncome || 0), 0);
  const totalExpense = roleRecords.reduce((sum, r) => sum + (r.goldExpense || 0), 0);
  const xuanjingCount = roleRecords.filter(r => r.hasXuanjing).length;
  const totalNet = totalIncome - totalExpense;

  const canDeleteRecord = useCallback((record: RaidRecord): boolean => {
    if (isAdmin) return true;
    if (currentUserId && record.accountId === currentUserId) return true;
    return false;
  }, [isAdmin, currentUserId]);

  const formatDate = useCallback((dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleDateString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit'
    });
  }, []);

  const handleDeleteClick = useCallback((record: RaidRecord) => {
    if (!canDeleteRecord(record)) {
      setErrorMessage('您没有权限删除此记录');
      setShowError(true);
      setTimeout(() => setShowError(false), 3000);
      return;
    }
    setRecordToDelete(record);
    setShowConfirmDialog(true);
  }, [canDeleteRecord]);

  const handleDeleteConfirm = useCallback(async () => {
    if (!recordToDelete || !setRecords) {
      setShowConfirmDialog(false);
      return;
    }

    setDeletingRecordId(recordToDelete.id);
    setShowConfirmDialog(false);

    try {
      await new Promise(resolve => setTimeout(resolve, 500));

      setRecords(prev => prev.filter(r => r.id !== recordToDelete.id));
      
      setShowSuccess(true);
      setTimeout(() => setShowSuccess(false), 2000);

    } catch (error) {
      setErrorMessage('删除失败，请重试');
      setShowError(true);
      setTimeout(() => setShowError(false), 3000);
    } finally {
      setDeletingRecordId(null);
      setRecordToDelete(null);
    }
  }, [recordToDelete, setRecords]);

  const handleDeleteCancel = useCallback(() => {
    setShowConfirmDialog(false);
    setRecordToDelete(null);
  }, []);

  const handleClose = useCallback(() => {
    if (deletingRecordId) return;
    onClose();
  }, [deletingRecordId, onClose]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-slate-900/60 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-2xl shadow-2xl w-full max-w-lg max-h-[85vh] overflow-hidden flex flex-col animate-in">
        <div className="bg-gradient-to-r from-slate-800 to-slate-700 px-5 py-4 flex-shrink-0">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-slate-600/50 flex items-center justify-center">
                <span className="text-white font-bold text-lg">{role.name.charAt(0)}</span>
              </div>
              <div>
                <h2 className="text-lg font-semibold text-white">{role.name}</h2>
                <p className="text-slate-300 text-xs mt-0.5">{role.region} {role.server}</p>
              </div>
            </div>
            <button
              onClick={handleClose}
              className="text-slate-300 hover:text-white transition-colors p-2 rounded-lg hover:bg-slate-600/50 active:scale-95"
              aria-label="关闭"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        <div className="bg-slate-50 px-5 py-3 border-b border-slate-200 flex-shrink-0">
          <div className="flex items-center justify-between flex-wrap gap-3">
            <div className="flex items-center gap-4">
              <div className="text-center">
                <div className="text-xl font-bold text-slate-800">{roleRecords.length}/{maxRecords}</div>
                <div className="text-xs text-slate-500">记录</div>
              </div>
              <div className="h-8 w-px bg-slate-200" />
              <div className="text-center">
                <div className="text-xl font-bold text-emerald-600">{formatGoldAmount(totalIncome)}</div>
                <div className="text-xs text-slate-500">收入</div>
              </div>
              {totalExpense > 0 && (
                <>
                  <div className="h-8 w-px bg-slate-200" />
                  <div className="text-center">
                    <div className="text-xl font-bold text-amber-600">{formatGoldAmount(totalExpense)}</div>
                    <div className="text-xs text-slate-500">支出</div>
                  </div>
                </>
              )}
              <div className="h-8 w-px bg-slate-200" />
              <div className="text-center">
                <div className={`text-xl font-bold ${totalNet >= 0 ? 'text-emerald-600' : 'text-amber-600'}`}>{formatGoldAmount(totalNet)}</div>
                <div className="text-xs text-slate-500">净收入</div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              {isTenPerson ? (
                <span className="text-xs bg-amber-100 text-amber-700 px-2.5 py-1 rounded-full font-medium">10人</span>
              ) : (
                <span className="text-xs bg-emerald-100 text-emerald-700 px-2.5 py-1 rounded-full font-medium">25人</span>
              )}
              {role.sect && role.sect !== '未知' && (
                <span className="text-xs bg-slate-200 text-slate-700 px-2.5 py-1 rounded-full font-medium">{role.sect}</span>
              )}
              {xuanjingCount > 0 && (
                <span className="text-xs bg-amber-100 text-amber-700 px-2.5 py-1 rounded-full font-medium flex items-center gap-1">
                  <Sparkles className="w-3 h-3" />
                  {xuanjingCount}
                </span>
              )}
            </div>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-4 scroll-smooth">
          {roleRecords.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-slate-400">
              <div className="w-16 h-16 bg-slate-100 rounded-full flex items-center justify-center mb-4">
                <Search className="w-8 h-8 text-slate-300" />
              </div>
              <p className="text-base font-medium mb-2">暂无记录</p>
              <p className="text-sm">本周 {isTenPerson ? '最多2条' : '仅1条'} 记录</p>
            </div>
          ) : (
            <div className="space-y-3">
              {roleRecords.map((record, index) => (
                <div
                  key={record.id}
                  className={`p-4 rounded-xl border-2 transition-all duration-200 hover:shadow-md ${
                    deletingRecordId === record.id ? 'opacity-50' : ''
                  } ${record.hasXuanjing ? 'border-amber-200 bg-gradient-to-br from-amber-50 to-white' : 'border-slate-200 bg-white'}`}
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-3">
                        <span className="text-xs bg-slate-200 text-slate-700 px-2 py-0.5 rounded font-bold">
                          #{index + 1}
                        </span>
                        <span className="font-semibold text-slate-800">{record.raidName}</span>
                        {record.hasXuanjing && (
                          <span className="px-2 py-0.5 bg-amber-100 text-amber-700 text-xs rounded font-medium flex items-center gap-1">
                            <Sparkles className="w-3.5 h-3.5" />
                            玄晶
                          </span>
                        )}
                      </div>
                      
                      <div className="space-y-2">
                        <div className="flex items-center gap-2 text-xs text-slate-500">
                          <Calendar className="w-4 h-4 flex-shrink-0" />
                          <span className="font-medium">{formatDate(record.date)}</span>
                        </div>
                        
                        <div className="flex items-center gap-3 flex-wrap">
                          {record.goldIncome > 0 && (
                            <div className="flex items-center gap-1.5 px-2.5 py-1.5 bg-emerald-50 rounded-lg">
                              <ArrowDownToLine className="w-4 h-4 text-emerald-600 flex-shrink-0" />
                              <span className="text-sm font-semibold text-emerald-700">{formatGoldAmount(record.goldIncome)}金</span>
                            </div>
                          )}
                          {record.goldExpense && record.goldExpense > 0 && (
                            <div className="flex items-center gap-1.5 px-2.5 py-1.5 bg-amber-50 rounded-lg">
                              <ArrowUpFromLine className="w-4 h-4 text-amber-600 flex-shrink-0" />
                              <span className="text-sm font-semibold text-amber-700">{formatGoldAmount(record.goldExpense)}金</span>
                            </div>
                          )}
                        </div>
                        
                        {record.notes && (
                          <div className="flex items-start gap-2 text-xs text-slate-500">
                            <Info className="w-4 h-4 flex-shrink-0 mt-0.5" />
                            <p className="flex-1 break-words">{record.notes}</p>
                          </div>
                        )}
                      </div>
                    </div>
                    
                    <div className="flex flex-col items-center gap-2 flex-shrink-0">
                      <div className="flex items-center gap-1 px-3 py-2 bg-slate-100 rounded-xl">
                        <Wallet className="w-4 h-4 text-slate-600 flex-shrink-0" />
                        <span className={`text-base font-bold font-mono ${
                          (record.goldIncome || 0) - (record.goldExpense || 0) >= 0 ? 'text-emerald-600' : 'text-amber-600'
                        }`}>
                          {(record.goldIncome || 0) - (record.goldExpense || 0) >= 0 ? '+' : ''}
                          {formatGoldAmount((record.goldIncome || 0) - (record.goldExpense || 0))}金
                        </span>
                      </div>
                      
                      {setRecords && (
                        <button
                          onClick={() => handleDeleteClick(record)}
                          disabled={deletingRecordId === record.id}
                          className={`p-2 rounded-xl transition-all duration-200 ${
                            canDeleteRecord(record)
                              ? 'text-slate-400 hover:text-red-500 hover:bg-red-50 active:scale-95'
                              : 'text-slate-300 cursor-not-allowed'
                          }`}
                          title={canDeleteRecord(record) ? '删除记录' : '无权限'}
                          aria-label={canDeleteRecord(record) ? '删除记录' : '无权限'}
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
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="bg-slate-50 px-5 py-3 border-t border-slate-200 flex-shrink-0">
          <button
            onClick={handleClose}
            disabled={!!deletingRecordId}
            className="w-full py-2.5 border border-slate-300 text-slate-700 rounded-xl font-medium hover:bg-slate-100 active:bg-slate-200 transition-all duration-200 flex items-center justify-center gap-2 text-sm disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <X className="w-4 h-4" />
            关闭
          </button>
        </div>
      </div>

      {showConfirmDialog && recordToDelete && (
        <div className="fixed inset-0 bg-slate-900/70 flex items-center justify-center z-50 animate-in">
          <div className="bg-white p-6 rounded-2xl shadow-2xl max-w-sm w-full mx-4 animate-in">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-12 h-12 bg-red-100 rounded-full flex items-center justify-center flex-shrink-0">
                <AlertCircle className="w-6 h-6 text-red-600" />
              </div>
              <div className="flex-1 min-w-0">
                <h3 className="text-lg font-semibold text-slate-800">确认删除</h3>
                <p className="text-sm text-slate-500 mt-0.5">此操作不可恢复</p>
              </div>
            </div>
            <p className="text-sm text-slate-600 mb-5">确定要删除此记录吗？</p>
            <div className="flex justify-end gap-3">
              <button
                onClick={handleDeleteCancel}
                className="px-5 py-2.5 border border-slate-300 text-slate-700 rounded-xl font-medium hover:bg-slate-50 active:bg-slate-100 transition-all duration-200 text-sm"
              >
                取消
              </button>
              <button
                onClick={handleDeleteConfirm}
                className="px-5 py-2.5 bg-red-600 text-white rounded-xl font-medium hover:bg-red-700 active:bg-red-800 transition-all duration-200 flex items-center gap-2 text-sm"
              >
                <Trash2 className="w-4 h-4" />
                删除
              </button>
            </div>
          </div>
        </div>
      )}

      {showSuccess && (
        <div className="fixed bottom-8 left-1/2 -translate-x-1/2 bg-emerald-600 text-white px-6 py-3 rounded-xl shadow-lg flex items-center gap-2 text-sm font-medium animate-in">
          <CheckCircle className="w-5 h-5" />
          记录已删除
        </div>
      )}

      {showError && (
        <div className="fixed bottom-8 left-1/2 -translate-x-1/2 bg-red-600 text-white px-6 py-3 rounded-xl shadow-lg flex items-center gap-2 text-sm font-medium animate-in">
          <AlertCircle className="w-5 h-5" />
          {errorMessage}
        </div>
      )}
    </div>
  );
};
