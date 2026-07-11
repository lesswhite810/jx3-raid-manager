import React, { useState, useMemo, useCallback } from 'react';
import { createPortal } from 'react-dom';
import {
  Clock, Check, X, Loader2, AlertTriangle, Package,
  TrendingUp, TrendingDown, Coins, Sparkles, Anchor, Ghost, Package as PackageIcon,
  Flag, Shirt, Crown, BookOpen, FileText, Pencil, Skull, Calendar,
} from 'lucide-react';
import { RaidRecord, Account } from '../types';
import { dropScannerService } from '../services/dropScanner';
import { getLastMonday, getNextMonday, getTenPersonCycle } from '../utils/cooldownManager';
import { getBaseServerName } from '../utils/serverUtils';
import { formatGoldAmount } from '../utils/recordUtils';
import { toast } from '../utils/toastManager';

interface PendingRecordsPanelProps {
  records: RaidRecord[];
  accounts: Account[];
  onRefreshRecords?: () => void;
}

interface RoleInfo {
  name: string;
  server: string;
  accountName: string;
}

/** 确认时编辑表单的数据结构（RaidRecord 部分字段） */
interface EditFormData {
  goldIncome: number;
  goldExpense: number;
  hasXuanjing: boolean;
  hasMaJu: boolean;
  hasPet: boolean;
  hasPendant: boolean;
  hasMount: boolean;
  hasAppearance: boolean;
  hasTitle: boolean;
  hasSecretBook: boolean;
  notes: string;
}

/**
 * 待确认记录面板
 *
 * 以按钮 + Modal 形式展示所有自动扫描产生的 pending 记录。
 * 每条记录显示角色、副本、时间、BOSS 进度、掉落物、收入（来自 chatlog）。
 * 确认时打开编辑弹窗，允许修改收支金额、标记位、备注。
 */
export const PendingRecordsPanel: React.FC<PendingRecordsPanelProps> = ({
  records,
  accounts,
  onRefreshRecords,
}) => {
  const [pendingActionId, setPendingActionId] = useState<string | null>(null);
  const [cdConflictRecord, setCdConflictRecord] = useState<RaidRecord | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingRecord, setEditingRecord] = useState<RaidRecord | null>(null);
  const [editForm, setEditForm] = useState<EditFormData | null>(null);
  const [isScanningThisWeek, setIsScanningThisWeek] = useState(false);

  // 筛选所有待确认记录（含 scanning 副本进行中和 pending 可确认）
  // scanning 记录显示为锁定状态，不可确认/拒绝；pending 记录可正常确认/拒绝
  const pendingRecords = useMemo(() => {
    return records.filter(r =>
      r.source === 'auto' && (r.status === 'pending' || r.status === 'scanning')
    );
  }, [records]);

  // 构建 roleId -> role 信息映射
  const roleMap = useMemo(() => {
    const map = new Map<string, RoleInfo>();
    accounts.forEach(acc => {
      acc.roles.forEach(role => {
        map.set(role.id, {
          name: role.name,
          server: role.server,
          accountName: acc.accountName,
        });
      });
    });
    return map;
  }, [accounts]);

  const formatDate = useCallback((date: string | number) => {
    const d = new Date(date);
    return d.toLocaleDateString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  }, []);

  /**
   * 检查 CD 冲突：同一角色、同一副本名、同一 CD 窗口内是否已存在手动记录
   */
  const checkCdConflict = useCallback((record: RaidRecord): boolean => {
    const recordDate = new Date(record.date);
    const isTenPerson = record.raidName?.includes('10人');

    let windowStart: Date;
    let windowEnd: Date;
    if (isTenPerson) {
      const cycle = getTenPersonCycle(recordDate);
      windowStart = cycle.start;
      windowEnd = cycle.end;
    } else {
      windowStart = getLastMonday(recordDate);
      windowEnd = getNextMonday(recordDate);
    }

    return records.some(r => {
      if (r.source !== 'manual') return false;
      if (r.roleId !== record.roleId) return false;
      if (r.raidName !== record.raidName) return false;
      if (r.status === 'rejected') return false;
      const rDate = new Date(r.date);
      return rDate >= windowStart && rDate < windowEnd;
    });
  }, [records]);

  /** 打开编辑弹窗，预填充自动扫描数据 */
  const openEditModal = useCallback((record: RaidRecord) => {
    if (checkCdConflict(record)) {
      setCdConflictRecord(record);
      return;
    }
    doOpenEditModal(record);
  }, [checkCdConflict]);

  /** 实际打开编辑弹窗（跳过 CD 冲突检查） */
  const doOpenEditModal = useCallback((record: RaidRecord) => {
    setEditingRecord(record);
    // 掉落物中包含"玄晶"时默认勾选玄晶
    const dropsHasXuanjing = record.drops?.some(d => d.includes('玄晶')) ?? false;
    setEditForm({
      goldIncome: record.goldIncome || 0,
      goldExpense: record.goldExpense || 0,
      hasXuanjing: record.hasXuanjing || dropsHasXuanjing,
      hasMaJu: record.hasMaJu || false,
      hasPet: record.hasPet || false,
      hasPendant: record.hasPendant || false,
      hasMount: record.hasMount || false,
      hasAppearance: record.hasAppearance || false,
      hasTitle: record.hasTitle || false,
      hasSecretBook: record.hasSecretBook || false,
      notes: record.notes || '',
    });
  }, []);

  const closeEditModal = useCallback(() => {
    setEditingRecord(null);
    setEditForm(null);
  }, []);

  /** 提交编辑表单，调用 confirmRecord 并传入编辑数据 */
  const handleSubmitEdit = useCallback(async () => {
    if (!editingRecord || !editForm) return;
    setPendingActionId(editingRecord.id);
    try {
      await dropScannerService.confirmRecord(editingRecord.id, {
        goldIncome: editForm.goldIncome,
        goldExpense: editForm.goldExpense,
        hasXuanjing: editForm.hasXuanjing,
        hasMaJu: editForm.hasMaJu,
        hasPet: editForm.hasPet,
        hasPendant: editForm.hasPendant,
        hasMount: editForm.hasMount,
        hasAppearance: editForm.hasAppearance,
        hasTitle: editForm.hasTitle,
        hasSecretBook: editForm.hasSecretBook,
        notes: editForm.notes.trim() || undefined,
      });
      onRefreshRecords?.();
      toast.success('记录已确认');
      closeEditModal();
    } catch (error) {
      console.error('确认记录失败:', error);
      toast.error('确认失败，请重试');
    } finally {
      setPendingActionId(null);
    }
  }, [editingRecord, editForm, onRefreshRecords, closeEditModal]);

  /** CD 冲突时点击"继续编辑"，关闭冲突提示后打开编辑弹窗 */
  const continueToEdit = useCallback(() => {
    if (!cdConflictRecord) return;
    const record = cdConflictRecord;
    setCdConflictRecord(null);
    doOpenEditModal(record);
  }, [cdConflictRecord, doOpenEditModal]);

  const handleReject = useCallback(async (record: RaidRecord) => {
    setPendingActionId(record.id);
    try {
      await dropScannerService.rejectRecord(record.id);
      onRefreshRecords?.();
      toast.success('记录已拒绝');
    } catch (error) {
      console.error('拒绝记录失败:', error);
      toast.error('拒绝失败，请重试');
    } finally {
      setPendingActionId(null);
    }
  }, [onRefreshRecords]);

  /**
   * 扫描本周（周一7:00到下周一7:00）所有账号数据
   * 离线扫描，不依赖 JX3 运行状态，扫描所有账号
   */
  const handleScanThisWeek = useCallback(async () => {
    if (isScanningThisWeek) return;
    setIsScanningThisWeek(true);
    try {
      const now = new Date();
      const startMs = getLastMonday(now).getTime();
      const endMs = getNextMonday(now).getTime();
      const results = await dropScannerService.scanRaidsInRange(startMs, endMs);
      const totalScanned = results.reduce((sum, r) => sum + (r.instanceCount ?? 0), 0);
      const failedCount = results.filter(r => !r.success).length;
      if (totalScanned > 0) {
        toast.success(`扫描完成，处理 ${totalScanned} 个副本实例`);
        onRefreshRecords?.();
      } else if (failedCount > 0) {
        toast.warning(`扫描完成但有 ${failedCount} 个账号失败`);
      } else {
        toast.info('本周暂无副本数据');
      }
    } catch (error) {
      console.error('扫描本周失败:', error);
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`扫描本周失败: ${message}`);
    } finally {
      setIsScanningThisWeek(false);
    }
  }, [isScanningThisWeek, onRefreshRecords]);

  if (pendingRecords.length === 0 && !isScanningThisWeek) {
    // 没有待确认记录时，仅显示"扫描本周"按钮
    return (
      <button
        onClick={handleScanThisWeek}
        disabled={isScanningThisWeek}
        className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-emerald-200 dark:border-emerald-800 bg-emerald-50 dark:bg-emerald-900/20 text-emerald-700 dark:text-emerald-400 hover:bg-emerald-100 dark:hover:bg-emerald-900/30 transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
        title="扫描本周一7:00到下周一7:00所有账号的副本数据"
      >
        {isScanningThisWeek ? (
          <Loader2 className="w-4 h-4 animate-spin" />
        ) : (
          <Calendar className="w-4 h-4" />
        )}
        <span>{isScanningThisWeek ? '扫描中...' : '扫描本周'}</span>
      </button>
    );
  }

  return (
    <>
      <div className="flex items-center gap-2">
        {/* 扫描本周按钮 */}
        <button
          onClick={handleScanThisWeek}
          disabled={isScanningThisWeek}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-emerald-200 dark:border-emerald-800 bg-emerald-50 dark:bg-emerald-900/20 text-emerald-700 dark:text-emerald-400 hover:bg-emerald-100 dark:hover:bg-emerald-900/30 transition-colors text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          title="扫描本周一7:00到下周一7:00所有账号的副本数据"
        >
          {isScanningThisWeek ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : (
            <Calendar className="w-4 h-4" />
          )}
          <span>{isScanningThisWeek ? '扫描中...' : '扫描本周'}</span>
        </button>

        {/* 待确认按钮：带数字徽章 */}
        <button
          onClick={() => setIsModalOpen(true)}
          className="relative flex items-center gap-2 px-3 py-1.5 rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-400 hover:bg-amber-100 dark:hover:bg-amber-900/30 transition-colors text-sm font-medium"
          title="查看自动扫描的待确认记录"
        >
          <Clock className="w-4 h-4" />
          <span>待确认</span>
          <span className="inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 rounded-full bg-amber-500 text-white text-xs font-bold">
            {pendingRecords.length}
          </span>
        </button>
      </div>

      {/* 记录列表 Modal */}
      {isModalOpen && createPortal(
        <div
          className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm flex items-center justify-center z-[100] p-4"
          onClick={() => setIsModalOpen(false)}
        >
          <div
            className="bg-surface rounded-2xl shadow-2xl w-full max-w-2xl max-h-[85vh] overflow-hidden flex flex-col animate-in"
            onClick={e => e.stopPropagation()}
          >
            {/* 标题栏 */}
            <div className="px-6 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm flex-shrink-0">
              <div className="flex items-center gap-2">
                <Clock className="w-5 h-5 text-amber-600 dark:text-amber-400" />
                <h2 className="text-lg font-bold text-main">
                  待确认记录
                  <span className="ml-2 text-amber-600 dark:text-amber-400">({pendingRecords.length})</span>
                </h2>
              </div>
              <button
                onClick={() => setIsModalOpen(false)}
                className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50"
                aria-label="关闭"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* 记录列表 */}
            <div className="flex-1 overflow-y-auto p-4 space-y-3">
              {pendingRecords.map(record => {
                const roleInfo = roleMap.get(record.roleId);
                const roleName = roleInfo?.name ?? record.roleName ?? '未知角色';
                const server = roleInfo?.server ?? record.server ?? '';
                const income = record.goldIncome || 0;
                const bosses = record.bossNames?.filter(n => n) ?? [];
                const isScanning = record.status === 'scanning';

                return (
                  <div
                    key={record.id}
                    className={`p-3 rounded-lg border transition-opacity ${
                      isScanning
                        ? 'border-slate-200 dark:border-slate-700 bg-slate-50/30 dark:bg-slate-800/5'
                        : 'border-amber-200 dark:border-amber-800/50 bg-amber-50/30 dark:bg-amber-900/5'
                    } ${pendingActionId === record.id ? 'opacity-50' : ''}`}
                  >
                    {/* 第一行：角色 · 副本 · 时间 · 状态标签 */}
                    <div className="flex items-center gap-2 flex-wrap mb-2">
                      <span className="text-sm font-semibold text-main">{roleName}</span>
                      {server && (
                        <span className="text-muted text-xs">·{getBaseServerName(server)}</span>
                      )}
                      <span className="text-sm text-main font-medium">{record.raidName}</span>
                      {isScanning && (
                        <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 border border-slate-200 dark:border-slate-700">
                          <Loader2 className="w-3 h-3 animate-spin" />
                          扫描中
                        </span>
                      )}
                      <span className="text-xs text-muted ml-auto">{formatDate(record.date)}</span>
                    </div>

                    {/* 第二行：收支 */}
                    <div className="flex items-center gap-4 mb-2 text-sm">
                      <span className="flex items-center gap-1">
                        <TrendingUp className="w-3.5 h-3.5 text-emerald-600" />
                        <span className="text-muted text-xs">收入</span>
                        <span className="font-mono font-semibold text-emerald-700 dark:text-emerald-400">
                          {formatGoldAmount(income)}
                        </span>
                      </span>
                      <span className="flex items-center gap-1">
                        <TrendingDown className="w-3.5 h-3.5 text-amber-600" />
                        <span className="text-muted text-xs">支出</span>
                        <span className="font-mono font-semibold text-amber-700 dark:text-amber-400">
                          {formatGoldAmount(record.goldExpense || 0)}
                        </span>
                      </span>
                      {record.drops && record.drops.length > 0 && (
                        <span className="flex items-center gap-1 ml-auto">
                          <Package className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />
                          <span className="text-muted text-xs">掉落</span>
                          <span className="font-mono font-semibold text-main">{record.drops.length}</span>
                        </span>
                      )}
                    </div>

                    {/* 第三行：BOSS 进度 */}
                    {bosses.length > 0 && (
                      <div className="flex items-center gap-1.5 flex-wrap mb-2">
                        <Skull className="w-3.5 h-3.5 text-rose-500 flex-shrink-0" />
                        {bosses.map((boss, idx) => (
                          <span
                            key={`${boss}-${idx}`}
                            className="px-1.5 py-0.5 rounded text-[11px] bg-rose-50 dark:bg-rose-900/20 text-rose-700 dark:text-rose-400 border border-rose-200 dark:border-rose-800"
                          >
                            {boss}
                          </span>
                        ))}
                      </div>
                    )}

                    {/* 第四行：按钮（scanning 状态禁用确认/拒绝） */}
                    {isScanning ? (
                      <div className="flex items-center gap-2 text-xs text-muted">
                        <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        <span>副本进行中，等待扫描完成后可确认</span>
                      </div>
                    ) : (
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() => openEditModal(record)}
                          disabled={pendingActionId === record.id}
                          className="flex items-center gap-1.5 px-3 py-1 rounded-lg text-sm font-medium text-white bg-primary hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                          {pendingActionId === record.id ? (
                            <Loader2 className="w-3.5 h-3.5 animate-spin" />
                          ) : (
                            <Pencil className="w-3.5 h-3.5" />
                          )}
                          编辑并确认
                        </button>
                        <button
                          onClick={() => handleReject(record)}
                          disabled={pendingActionId === record.id}
                          className="flex items-center gap-1.5 px-3 py-1 rounded-lg text-sm font-medium text-muted hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 border border-base hover:border-red-200 dark:hover:border-red-800 transition-colors disabled:opacity-50"
                        >
                          <X className="w-3.5 h-3.5" />
                          拒绝
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </div>,
        document.body
      )}

      {/* 编辑弹窗 */}
      {editingRecord && editForm && createPortal(
        <div
          className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[110] flex items-center justify-center p-4"
          onClick={closeEditModal}
        >
          <div
            className="bg-surface rounded-xl shadow-2xl w-full max-w-md overflow-hidden animate-in"
            onClick={e => e.stopPropagation()}
          >
            {/* 标题栏 */}
            <div className="px-6 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm">
              <div>
                <h2 className="text-lg font-bold text-main">确认并编辑记录</h2>
                <p className="text-muted text-xs mt-0.5">
                  <span className="font-medium text-main">
                    {roleMap.get(editingRecord.roleId)?.name ?? editingRecord.roleName}
                  </span>
                  <span className="mx-1.5 text-muted/40">·</span>
                  {editingRecord.raidName}
                </p>
              </div>
              <button
                onClick={closeEditModal}
                className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* 表单 */}
            <div className="p-5 space-y-4 max-h-[70vh] overflow-y-auto">
              {/* 收支输入 */}
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                    <TrendingUp className="w-4 h-4 text-emerald-600" />
                    金币收入
                  </label>
                  <div className="relative">
                    <Coins className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-emerald-500" />
                    <input
                      type="number"
                      min="0"
                      value={editForm.goldIncome || ''}
                      onChange={e => setEditForm({ ...editForm, goldIncome: Number(e.target.value) })}
                      placeholder="收入金额"
                      className="w-full pl-9 pr-3 py-2.5 bg-surface border border-emerald-200 dark:border-emerald-800 rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-emerald-400 transition-all font-mono text-[1rem]"
                    />
                  </div>
                </div>
                <div>
                  <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                    <TrendingDown className="w-4 h-4 text-amber-600" />
                    金币支出
                  </label>
                  <div className="relative">
                    <Coins className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-amber-500" />
                    <input
                      type="number"
                      min="0"
                      value={editForm.goldExpense || ''}
                      onChange={e => setEditForm({ ...editForm, goldExpense: Number(e.target.value) })}
                      placeholder="支出金额"
                      className="w-full pl-9 pr-3 py-2.5 bg-surface border border-amber-200 dark:border-amber-800 rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-amber-400 transition-all font-mono text-[1rem]"
                    />
                  </div>
                </div>
              </div>

              {/* 标记位 */}
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 p-3 bg-base rounded-lg border border-base">
                {[
                  { key: 'hasXuanjing' as const, label: '玄晶', icon: Sparkles, color: 'text-amber-500' },
                  { key: 'hasMaJu' as const, label: '马具', icon: Anchor, color: 'text-blue-500' },
                  { key: 'hasPet' as const, label: '宠物', icon: Ghost, color: 'text-purple-500' },
                  { key: 'hasPendant' as const, label: '挂件', icon: PackageIcon, color: 'text-orange-500' },
                  { key: 'hasMount' as const, label: '坐骑', icon: Flag, color: 'text-green-500' },
                  { key: 'hasAppearance' as const, label: '外观', icon: Shirt, color: 'text-pink-500' },
                  { key: 'hasTitle' as const, label: '称号', icon: Crown, color: 'text-yellow-600' },
                  { key: 'hasSecretBook' as const, label: '秘籍', icon: BookOpen, color: 'text-cyan-600' },
                ].map(({ key, label, icon: Icon, color }) => (
                  <div key={key} className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={editForm[key]}
                      onChange={e => setEditForm({ ...editForm, [key]: e.target.checked })}
                      id={`edit-${key}`}
                      className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                    />
                    <label htmlFor={`edit-${key}`} className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                      <Icon className={`w-3.5 h-3.5 ${color}`} />
                      <span>{label}</span>
                    </label>
                  </div>
                ))}
              </div>

              {/* 备注 */}
              <div>
                <label className="block text-sm font-medium text-main mb-1.5">
                  <FileText className="w-4 h-4 inline mr-1" />
                  备注
                </label>
                <textarea
                  value={editForm.notes}
                  onChange={e => setEditForm({ ...editForm, notes: e.target.value })}
                  placeholder="可选"
                  rows={2}
                  className="w-full px-3 py-2.5 bg-surface border border-base rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-base transition-all resize-none text-sm"
                />
              </div>

              {/* 操作按钮 */}
              <div className="flex gap-2.5 pt-2">
                <button
                  onClick={closeEditModal}
                  className="flex-1 px-4 py-2.5 border border-base text-main rounded-lg font-medium hover:bg-base transition-colors text-sm"
                >
                  取消
                </button>
                <button
                  onClick={handleSubmitEdit}
                  disabled={pendingActionId === editingRecord.id}
                  className="flex-1 px-4 py-2.5 bg-primary text-white rounded-lg font-medium hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 text-sm"
                >
                  {pendingActionId === editingRecord.id ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin" />
                      <span>提交中...</span>
                    </>
                  ) : (
                    <>
                      <Check className="w-4 h-4" />
                      <span>确认</span>
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        </div>,
        document.body
      )}

      {/* CD 冲突确认弹窗 */}
      {cdConflictRecord && createPortal(
        <div
          className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm flex items-center justify-center z-[120] p-4"
          onClick={() => setCdConflictRecord(null)}
        >
          <div
            className="bg-surface rounded-2xl shadow-2xl w-full max-w-sm p-6 animate-in"
            onClick={e => e.stopPropagation()}
          >
            <div className="flex items-center gap-3 mb-4">
              <div className="w-10 h-10 rounded-full bg-amber-100 dark:bg-amber-900/30 flex items-center justify-center flex-shrink-0">
                <AlertTriangle className="w-5 h-5 text-amber-600 dark:text-amber-400" />
              </div>
              <h3 className="text-lg font-bold text-main">CD 冲突提醒</h3>
            </div>
            <p className="text-sm text-muted mb-2 leading-relaxed">
              角色 <span className="font-medium text-main">
                {roleMap.get(cdConflictRecord.roleId)?.name ?? cdConflictRecord.roleName}
              </span> 在当前 CD 周期内已存在
              <span className="font-medium text-main">「{cdConflictRecord.raidName}」</span>
              的手动记录。
            </p>
            <p className="text-sm text-muted mb-6 leading-relaxed">
              确认此自动记录后，两条记录将共存并共享 CD。是否继续？
            </p>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setCdConflictRecord(null)}
                className="px-4 py-2 text-sm font-medium text-muted hover:text-main rounded-lg hover:bg-base transition-colors"
              >
                取消
              </button>
              <button
                onClick={continueToEdit}
                className="px-4 py-2 text-sm font-medium text-white bg-amber-600 hover:bg-amber-700 rounded-lg transition-colors flex items-center gap-1.5"
              >
                <Pencil className="w-3.5 h-3.5" />
                继续编辑
              </button>
            </div>
          </div>
        </div>,
        document.body
      )}
    </>
  );
};
