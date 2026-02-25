import React, { useState, useMemo, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { RaidRecord, Raid } from '../types';
import { X, Search, Calendar, Sparkles, Trash2, CheckCircle, AlertCircle, Loader2, TrendingUp, TrendingDown, Wallet, Info, Anchor, Ghost, Package, Shirt, Crown, Flag, Pencil, BookOpen } from 'lucide-react';
import { formatGoldAmount } from '../utils/recordUtils';
import { getLastMonday, getNextMonday } from '../utils/cooldownManager';
import { calculateBossCooldowns } from '../utils/bossCooldownManager';
import { BossCooldownSummary } from './BossCooldownDisplay';
import { db } from '../services/db';

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
  onEditRecord?: (record: RaidRecord) => void;
  onRefreshRecords?: () => void;
}

export const RoleRecordsModal: React.FC<RoleRecordsModalProps> = ({
  isOpen,
  onClose,
  role,
  records,
  raid,
  setRecords,
  currentUserId,
  isAdmin = false,
  onEditRecord,
  onRefreshRecords
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
    // 统计周期起点：周一 00:00
    const weekStart = getLastMonday(now);
    const weekEnd = getNextMonday(now);
    return { start: weekStart, end: weekEnd };
  }, []);

  const roleRecords = useMemo(() => {
    const safeRecords = Array.isArray(records) ? records : [];
    const weekStartTime = weekInfo.start.getTime();
    const weekEndTime = weekInfo.end.getTime();

    const difficultyText = raid.difficulty;

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

  const totalIncome = roleRecords.reduce((sum, r) => sum + (Number(r.goldIncome) || 0), 0);
  const totalExpense = roleRecords.reduce((sum, r) => sum + (Number(r.goldExpense) || 0), 0);
  const xuanjingCount = roleRecords.filter(r => r.hasXuanjing).length;
  const totalNet = totalIncome - totalExpense;

  const bossCooldowns = useMemo(() => {
    return calculateBossCooldowns(raid, roleRecords.flatMap(r => {
      // 支持多选BOSS：为每个bossId创建一个记录
      if (r.bossIds && r.bossIds.length > 0) {
        return r.bossIds.map(bossId => ({
          id: r.id,
          raidRecordId: r.id,
          bossId: bossId,
          bossName: r.bossNames?.[r.bossIds?.indexOf(bossId) || 0] || '',
          date: r.date,
          roleId: r.roleId,
          accountId: r.accountId
        }));
      }
      // 向后兼容单选BOSS
      return [{
        id: r.id,
        raidRecordId: r.id,
        bossId: r.bossId || '',
        bossName: r.bossName || '',
        date: r.date,
        roleId: r.roleId,
        accountId: r.accountId
      }];
    }), role.id);
  }, [raid, roleRecords, role.id]);

  const canDeleteRecord = useCallback((record: RaidRecord): boolean => {
    if (isAdmin) return true;
    if (currentUserId && record.accountId === currentUserId) return true;
    return false;
  }, [isAdmin, currentUserId]);

  const formatDate = useCallback((dateString: string | number) => {
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

      try {
        await db.deleteRecord(recordToDelete.id);
        onRefreshRecords?.();
      } catch (error) {
        console.error('删除记录失败:', error);
        throw error;
      }

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

  return createPortal(
    <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm flex items-center justify-center z-[100] p-4 overflow-hidden">
      <div className="bg-surface rounded-2xl shadow-2xl w-full max-w-lg max-h-[85vh] overflow-hidden flex flex-col animate-in">
        <div className="px-6 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm flex-shrink-0">
          <div>
            <h2 className="text-lg font-bold text-main">副本记录详情</h2>
            <p className="text-muted text-xs mt-0.5">
              <span className="font-medium text-main">{role.name}@{role.server}</span>
              <span className="mx-1.5 text-muted/40">·</span>
              {isTenPerson ? '10人' : '25人'}{raid.difficulty}{raid.name}
            </p>
          </div>
          <button
            onClick={handleClose}
            className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50 active:scale-95"
            aria-label="关闭"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="bg-base px-5 py-3 border-b border-base flex-shrink-0">
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-center gap-4 flex-wrap">
              <div className="flex items-center gap-1.5" title="总收入">
                <TrendingUp className="w-4 h-4 text-emerald-600 dark:text-emerald-500 flex-shrink-0" />
                <span className="text-[1rem] font-bold text-emerald-600 dark:text-emerald-500">{formatGoldAmount(totalIncome || 0)}</span>
              </div>
              <div className="flex items-center gap-1.5" title="总支出">
                <TrendingDown className="w-4 h-4 text-amber-600 dark:text-amber-500 flex-shrink-0" />
                <span className="text-[1rem] font-bold text-amber-600 dark:text-amber-500">{formatGoldAmount(totalExpense || 0)}</span>
              </div>
              <div className="flex items-center gap-1.5" title="净收入">
                <Wallet className="w-4 h-4 text-muted flex-shrink-0" />
                <span className={`text-[1rem] font-bold ${totalNet >= 0 ? 'text-emerald-600 dark:text-emerald-500' : 'text-amber-600 dark:text-amber-500'}`}>{formatGoldAmount(totalNet || 0)}</span>
              </div>
              {xuanjingCount > 0 && (
                <span className="text-xs bg-amber-100 text-amber-700 px-2 py-0.5 rounded-md font-medium flex items-center gap-1 shadow-sm">
                  <Sparkles className="w-3 h-3" />
                  {xuanjingCount}
                </span>
              )}
            </div>

            <div className="flex flex-col flex-shrink-0">
              {bossCooldowns && bossCooldowns.length > 0 ? (
                <div className="flex items-center justify-center p-1">
                  <BossCooldownSummary bossCooldowns={bossCooldowns} transparent />
                </div>
              ) : (
                <div className="flex items-center justify-center bg-surface border border-base px-3 py-1.5 rounded-md shadow-sm">
                  <span className="text-sm font-bold text-main">{roleRecords.length} <span className="text-muted text-xs font-normal mx-0.5">/</span> {maxRecords}</span>
                </div>
              )}
            </div>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-4 scroll-smooth">
          {roleRecords.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-muted">
              <div className="w-16 h-16 bg-base rounded-full flex items-center justify-center mb-4">
                <Search className="w-8 h-8 text-muted/50" />
              </div>
              <p className="text-[1rem] font-medium mb-2">暂无记录</p>
            </div>
          ) : (
            <div className="space-y-3">
              {roleRecords.map((record) => (
                <div
                  key={record.id}
                  className={`p-4 rounded-xl border-2 transition-all duration-200 hover:shadow-md ${deletingRecordId === record.id ? 'opacity-50' : ''
                    } ${record.hasXuanjing ? 'border-amber-200 dark:border-amber-800 bg-gradient-to-br from-amber-50 to-white dark:from-amber-900/20 dark:to-surface' : 'border-base bg-surface'}`}
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-3 mb-2 flex-wrap">

                        <div className="flex items-center gap-1.5 text-xs text-muted">
                          <Calendar className="w-3.5 h-3.5 flex-shrink-0" />
                          <span className="font-medium">{formatDate(record.date)}</span>
                        </div>

                        {record.goldIncome > 0 && (
                          <div className="flex items-center gap-1" title="收入">
                            <TrendingUp className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-500 flex-shrink-0" />
                            <span className="text-sm font-semibold text-emerald-600 dark:text-emerald-500">{formatGoldAmount(record.goldIncome)}</span>
                          </div>
                        )}
                        {record.goldExpense && record.goldExpense > 0 ? (
                          <div className="flex items-center gap-1" title="支出">
                            <TrendingDown className="w-3.5 h-3.5 text-amber-600 dark:text-amber-500 flex-shrink-0" />
                            <span className="text-sm font-semibold text-amber-600 dark:text-amber-500">{formatGoldAmount(record.goldExpense)}</span>
                          </div>
                        ) : null}
                      </div>

                      <div className="flex flex-col gap-2">
                        {(record.hasXuanjing || record.hasMaJu || record.hasPet || record.hasPendant || record.hasMount || record.hasAppearance || record.hasTitle || record.hasSecretBook) && (
                          <div className="flex items-center gap-2 flex-wrap">
                            {record.hasXuanjing && (
                              <span className="px-2 py-0.5 bg-amber-100 text-amber-700 text-xs rounded font-medium flex items-center gap-1">
                                <Sparkles className="w-3 h-3" />玄晶
                              </span>
                            )}
                            {record.hasMaJu && (
                              <span className="px-2 py-0.5 bg-blue-100 text-blue-700 text-xs rounded font-medium flex items-center gap-1">
                                <Anchor className="w-3 h-3" />马具
                              </span>
                            )}
                            {record.hasPet && (
                              <span className="px-2 py-0.5 bg-purple-100 text-purple-700 text-xs rounded font-medium flex items-center gap-1">
                                <Ghost className="w-3 h-3" />宠物
                              </span>
                            )}
                            {record.hasPendant && (
                              <span className="px-2 py-0.5 bg-orange-100 text-orange-700 text-xs rounded font-medium flex items-center gap-1">
                                <Package className="w-3 h-3" />挂件
                              </span>
                            )}
                            {record.hasMount && (
                              <span className="px-2 py-0.5 bg-green-100 text-green-700 text-xs rounded font-medium flex items-center gap-1">
                                <Flag className="w-3 h-3" />坐骑
                              </span>
                            )}
                            {record.hasAppearance && (
                              <span className="px-2 py-0.5 bg-pink-100 text-pink-700 text-xs rounded font-medium flex items-center gap-1">
                                <Shirt className="w-3 h-3" />外观
                              </span>
                            )}
                            {record.hasTitle && (
                              <span className="px-2 py-0.5 bg-yellow-100 text-yellow-700 text-xs rounded font-medium flex items-center gap-1">
                                <Crown className="w-3 h-3" />称号
                              </span>
                            )}
                            {record.hasSecretBook && (
                              <span className="px-2 py-0.5 bg-cyan-100 text-cyan-700 text-xs rounded font-medium flex items-center gap-1">
                                <BookOpen className="w-3 h-3" />秘籍
                              </span>
                            )}
                          </div>
                        )}

                        {record.notes && (
                          <div className="flex items-start gap-1.5 text-xs text-muted bg-base/50 p-2 rounded-lg">
                            <Info className="w-3.5 h-3.5 flex-shrink-0 mt-0.5 text-muted/70" />
                            <p className="flex-1 break-words leading-relaxed">{record.notes}</p>
                          </div>
                        )}
                      </div>
                    </div>

                    <div className="flex flex-row items-center gap-1.5 flex-shrink-0">
                      {setRecords && (
                        <>
                          <button
                            onClick={() => onEditRecord?.(record)}
                            className="p-1.5 rounded-lg text-muted hover:text-primary hover:bg-base active:scale-95 transition-all duration-200"
                            title="修改记录"
                          >
                            <Pencil className="w-4 h-4" />
                          </button>
                          <button
                            onClick={() => handleDeleteClick(record)}
                            disabled={deletingRecordId === record.id}
                            className={`p-1.5 rounded-lg transition-all duration-200 ${canDeleteRecord(record)
                              ? 'text-muted hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 active:scale-95'
                              : 'text-muted/50 cursor-not-allowed'
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
                        </>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="bg-base px-5 py-3 border-t border-base flex-shrink-0">
          <button
            onClick={handleClose}
            disabled={!!deletingRecordId}
            className="w-full py-2.5 border border-base text-main rounded-xl font-medium hover:bg-surface active:bg-base transition-all duration-200 flex items-center justify-center gap-2 text-sm disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <X className="w-4 h-4" />
            关闭
          </button>
        </div>
      </div>

      {showConfirmDialog && recordToDelete && (
        <div className="fixed inset-0 bg-slate-900/70 flex items-center justify-center z-[110] animate-in fade-in duration-200">
          <div className="bg-surface p-6 rounded-2xl shadow-2xl max-w-sm w-full mx-4 animate-in">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-12 h-12 bg-red-100 dark:bg-red-900/20 rounded-full flex items-center justify-center flex-shrink-0">
                <AlertCircle className="w-6 h-6 text-red-600 dark:text-red-400" />
              </div>
              <div className="flex-1 min-w-0">
                <h3 className="text-lg font-semibold text-main">确认删除</h3>
                <p className="text-sm text-muted mt-0.5">此操作不可恢复</p>
              </div>
            </div>
            <p className="text-sm text-muted mb-5">确定要删除此记录吗？</p>
            <div className="flex justify-end gap-3">
              <button
                onClick={handleDeleteCancel}
                className="px-5 py-2.5 border border-base text-main rounded-xl font-medium hover:bg-base transition-all duration-200 text-sm"
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
    </div>,
    document.body
  );
};
