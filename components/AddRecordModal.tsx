import React, { useState, useEffect } from 'react';
import { Raid, RaidRecord } from '../types';
import { X, Calendar, Coins, Sparkles, FileText, ArrowDownToLine, ArrowUpFromLine, Wallet, AlertCircle } from 'lucide-react';
import { generateUUID } from '../utils/uuid';
import { logOperation } from '../utils/cooldownManager';

interface RoleWithStatus {
  id: string;
  name: string;
  server: string;
  region: string;
  sect: string;
  accountId: string;
  accountName: string;
}

interface AddRecordModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (record: Partial<RaidRecord>) => void;
  raid: Raid;
  role: RoleWithStatus;
}

export const AddRecordModal: React.FC<AddRecordModalProps> = ({
  isOpen,
  onClose,
  onSubmit,
  raid,
  role
}) => {
  const [goldIncome, setGoldIncome] = useState<number>(0);
  const [goldExpense, setGoldExpense] = useState<number>(0);
  const [hasXuanjing, setHasXuanjing] = useState(false);
  const [notes, setNotes] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      setGoldIncome(0);
      setGoldExpense(0);
      setHasXuanjing(false);
      setNotes('');
      setIsSubmitting(false);
      setErrorMessage(null);
    }
  }, [isOpen]);

  const constructRaidName = (): string => {
    const difficultyLabel = raid.difficulty === 'NORMAL' ? '普通' :
      raid.difficulty === 'HEROIC' ? '英雄' :
        '挑战';
    return `${raid.playerCount}人${difficultyLabel}${raid.name}`;
  };

  const formatDate = (): string => {
    const now = new Date();
    return now.toLocaleDateString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit'
    });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage(null);

    if (goldIncome <= 0 && goldExpense <= 0) {
      logOperation(role.id, role.name, constructRaidName(), false, '未输入金额');
      setErrorMessage('请输入收入或支出金额');
      return;
    }

    setIsSubmitting(true);

    try {
      await new Promise(resolve => setTimeout(resolve, 300));

      const record: Partial<RaidRecord> = {
        id: generateUUID(),
        accountId: role.accountId,
        roleId: role.id,
        raidName: constructRaidName(),
        date: new Date().toISOString(),
        goldIncome: goldIncome || 0,
        goldExpense: goldExpense > 0 ? goldExpense : undefined,
        hasXuanjing: hasXuanjing,
        notes: notes.trim() || undefined,
        roleName: role.name,
        server: `${role.region} ${role.server}`,
        transactionType: 'combined'
      };

      onSubmit(record);
      logOperation(
        role.id,
        role.name,
        constructRaidName(),
        true,
        undefined,
        `收入: ${goldIncome}, 支出: ${goldExpense}, 玄晶: ${hasXuanjing}`
      );

      onClose();
    } catch (error) {
      const msg = '添加记录失败，请重试';
      logOperation(role.id, role.name, constructRaidName(), false, msg);
      setErrorMessage(msg);
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <>
      <div
        className="fixed inset-0 bg-slate-900/60 z-40"
        onClick={(e) => {
          if (e.target === e.currentTarget) {
            onClose();
          }
        }}
      />
      <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
        <div className={`bg-surface rounded-xl shadow-2xl w-full max-w-md overflow-hidden pointer-events-auto transition-all duration-300`}>
          <div className="px-6 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm">
            <div>
              <h2 className="text-lg font-bold text-main">添加副本记录</h2>
              <p className="text-muted text-xs mt-0.5">{constructRaidName()}</p>
            </div>
            <button
              onClick={onClose}
              className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50"
            >
              <X className="w-5 h-5" />
            </button>
          </div>

          <form onSubmit={handleSubmit} className="p-5 space-y-4">
            <div className="flex items-center justify-between p-3 bg-base rounded-lg border border-base">
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 bg-surface rounded-full flex items-center justify-center border border-base">
                  <span className="text-muted font-bold text-sm">{role.name.charAt(0)}</span>
                </div>
                <div>
                  <p className="font-medium text-main text-sm">{role.name}</p>
                  <p className="text-xs text-muted">{role.region} {role.server}</p>
                </div>
              </div>
              {role.sect && role.sect !== '未知' && (
                <span className="text-xs bg-surface text-muted border border-base px-2 py-0.5 rounded-full">{role.sect}</span>
              )}
            </div>

            <div className="flex items-center justify-between text-xs">
              <div className="flex items-center gap-2 text-muted">
                <Calendar className="w-3.5 h-3.5" />
                {formatDate()}
              </div>
            </div>

            {errorMessage && (
              <div className="p-3 bg-red-50 border border-red-200 rounded-lg">
                <p className="text-sm text-red-700 dark:text-red-400 flex items-center gap-2">
                  <AlertCircle className="w-4 h-4 flex-shrink-0" />
                  {errorMessage}
                </p>
              </div>
            )}

            <div className="space-y-3">
              <div>
                <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                  <ArrowDownToLine className="w-4 h-4 text-emerald-600" />
                  金币收入
                </label>
                <div className="relative">
                  <Coins className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-emerald-500" />
                  <input
                    type="number"
                    min="0"
                    value={goldIncome || ''}
                    onChange={e => setGoldIncome(Number(e.target.value))}
                    placeholder="收入金额"
                    className="w-full pl-9 pr-3 py-2.5 bg-surface border border-emerald-200 dark:border-emerald-800 rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-emerald-400 transition-all font-mono text-base"
                  />
                </div>
              </div>

              <div>
                <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                  <ArrowUpFromLine className="w-4 h-4 text-amber-600" />
                  金币支出
                </label>
                <div className="relative">
                  <Coins className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-amber-500" />
                  <input
                    type="number"
                    min="0"
                    value={goldExpense || ''}
                    onChange={e => setGoldExpense(Number(e.target.value))}
                    placeholder="支出金额"
                    className="w-full pl-9 pr-3 py-2.5 bg-surface border border-amber-200 dark:border-amber-800 rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-amber-400 transition-all font-mono text-base"
                  />
                </div>
              </div>

              {(goldIncome > 0 || goldExpense > 0) && (
                <div className="flex items-center gap-2 p-3 bg-base rounded-lg border border-base">
                  <Wallet className="w-4 h-4 text-muted" />
                  <span className="text-sm text-muted">净收入:</span>
                  <span className={`text-lg font-bold font-mono ${goldIncome - goldExpense >= 0 ? 'text-emerald-600' : 'text-amber-600'}`}>
                    {goldIncome - goldExpense >= 0 ? '+' : ''}{(goldIncome - goldExpense).toLocaleString()}金
                  </span>
                </div>
              )}
            </div>

            <div className="flex items-center gap-3 p-3 bg-base rounded-lg border border-base">
              <input
                type="checkbox"
                checked={hasXuanjing}
                onChange={e => setHasXuanjing(e.target.checked)}
                id="xuanjing"
                className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
              />
              <label htmlFor="xuanjing" className="flex items-center gap-2 cursor-pointer text-sm text-main">
                <Sparkles className="w-4 h-4 text-muted" />
                <span>出玄晶</span>
              </label>
            </div>

            <div>
              <label className="block text-sm font-medium text-main mb-1.5">
                <FileText className="w-4 h-4 inline mr-1" />
                备注
              </label>
              <textarea
                value={notes}
                onChange={e => setNotes(e.target.value)}
                placeholder="可选"
                rows={2}
                className="w-full px-3 py-2.5 bg-surface border border-base rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-base transition-all resize-none text-sm"
              />
            </div>

            <div className="flex gap-2.5 pt-2">
              <button
                type="button"
                onClick={onClose}
                className="flex-1 px-4 py-2.5 border border-base text-main rounded-lg font-medium hover:bg-base transition-colors text-sm"
              >
                取消
              </button>
              <button
                type="submit"
                disabled={isSubmitting || (goldIncome <= 0 && goldExpense <= 0)}
                className="flex-1 px-4 py-2.5 bg-primary text-white rounded-lg font-medium hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 text-sm"
              >
                {isSubmitting ? (
                  <>
                    <div className="w-4 h-4 border-2 border-white/40 border-t-white rounded-full animate-spin" />
                    <span>提交中...</span>
                  </>
                ) : (
                  <>
                    <Coins className="w-4 h-4" />
                    <span>确认</span>
                  </>
                )}
              </button>
            </div>
          </form>
        </div>
      </div>
    </>
  );
};
