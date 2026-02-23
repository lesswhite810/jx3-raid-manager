import React, { useState, useEffect, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { Raid, RaidRecord } from '../types';
import { X, Calendar, Coins, Sparkles, FileText, TrendingUp, TrendingDown, AlertCircle, Shirt, Crown, Package, Ghost, Anchor, Flag, BookOpen } from 'lucide-react';
import { generateUUID } from '../utils/uuid';
import { logOperation } from '../utils/cooldownManager';
import { DateTimePicker } from './DateTimePicker';

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
  initialData?: RaidRecord;
}

export const AddRecordModal: React.FC<AddRecordModalProps> = ({
  isOpen,
  onClose,
  onSubmit,
  raid,
  role,
  initialData
}) => {
  const [goldIncome, setGoldIncome] = useState<number>(0);
  const [goldExpense, setGoldExpense] = useState<number>(0);
  const [hasXuanjing, setHasXuanjing] = useState(false);
  const [hasMaJu, setHasMaJu] = useState(false);
  const [hasPet, setHasPet] = useState(false);
  const [hasPendant, setHasPendant] = useState(false);
  const [hasMount, setHasMount] = useState(false);
  const [hasAppearance, setHasAppearance] = useState(false);
  const [hasTitle, setHasTitle] = useState(false);
  const [hasSecretBook, setHasSecretBook] = useState(false);
  const [notes, setNotes] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [selectedBossIds, setSelectedBossIds] = useState<string[]>([]);
  const [recordDate, setRecordDate] = useState<string>('');

  const availableBosses = useMemo(() => {
    return raid.bosses || [];
  }, [raid]);

  // 将日期转换为输入框显示用的字符串，支持时间戳和ISO字符串
  const formatDateForInput = (date: Date | string | number): string => {
    const d = new Date(date);
    const year = d.getFullYear();
    const month = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    const hours = String(d.getHours()).padStart(2, '0');
    const minutes = String(d.getMinutes()).padStart(2, '0');
    return `${year}-${month}-${day}T${hours}:${minutes}`;
  };

  // 将日期字符串转换为时间戳存储
  const formatDateFromInput = (dateStr: string): number => {
    if (!dateStr) return Date.now();
    return new Date(dateStr).getTime();
  };

  useEffect(() => {
    if (isOpen) {
      if (initialData) {
        setGoldIncome(initialData.goldIncome || 0);
        setGoldExpense(initialData.goldExpense || 0);
        setHasXuanjing(initialData.hasXuanjing || false);
        setHasMaJu(initialData.hasMaJu || false);
        setHasPet(initialData.hasPet || false);
        setHasPendant(initialData.hasPendant || false);
        setHasMount(initialData.hasMount || false);
        setHasAppearance(initialData.hasAppearance || false);
        setHasTitle(initialData.hasTitle || false);
        setHasSecretBook(initialData.hasSecretBook || false);
        setNotes(initialData.notes || '');
        setSelectedBossIds(initialData.bossIds || (initialData.bossId ? [initialData.bossId] : []));
        setRecordDate(formatDateForInput(initialData.date || new Date()));
      } else {
        setGoldIncome(0);
        setGoldExpense(0);
        setHasXuanjing(false);
        setHasMaJu(false);
        setHasPet(false);
        setHasPendant(false);
        setHasMount(false);
        setHasAppearance(false);
        setHasTitle(false);
        setHasSecretBook(false);
        setNotes('');
        setSelectedBossIds(availableBosses.map(b => b.id));
        setRecordDate(formatDateForInput(new Date()));
      }
      setIsSubmitting(false);
      setErrorMessage(null);
    }
  }, [isOpen, initialData]);

  const constructRaidName = (): string => {
    return `${raid.playerCount}人${raid.difficulty}${raid.name}`;
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
        id: initialData ? initialData.id : generateUUID(),
        accountId: role.accountId,
        roleId: role.id,
        raidName: initialData ? initialData.raidName : constructRaidName(),
        date: formatDateFromInput(recordDate),
        goldIncome: goldIncome || 0,
        goldExpense: goldExpense > 0 ? goldExpense : undefined,
        hasXuanjing,
        hasMaJu,
        hasPet,
        hasPendant,
        hasMount,
        hasAppearance,
        hasTitle,
        hasSecretBook,
        notes: notes.trim() || undefined,
        roleName: role.name,
        server: `${role.region} ${role.server}`,
        transactionType: 'combined',
        bossIds: selectedBossIds.length > 0 ? selectedBossIds : undefined,
        bossNames: selectedBossIds.map(id => availableBosses.find(b => b.id === id)?.name).filter(Boolean) as string[] || undefined,
      };

      onSubmit(record);
      logOperation(
        role.id,
        role.name,
        constructRaidName(),
        true,
        undefined,
        `收入: ${goldIncome}, 支出: ${goldExpense}, 玄晶: ${hasXuanjing}, 马具: ${hasMaJu}, 宠物: ${hasPet}, 挂件: ${hasPendant}, 坐骑: ${hasMount}, 外观: ${hasAppearance}, 称号: ${hasTitle}, 秘籍: ${hasSecretBook}`
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

  return createPortal(
    <>
      <div
        className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm z-[100]"
        onClick={(e) => {
          if (e.target === e.currentTarget) {
            onClose();
          }
        }}
      />
      <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 pointer-events-none">
        <div className={`bg-surface rounded-xl shadow-2xl w-full max-w-md overflow-hidden pointer-events-auto transition-all duration-300`}>
          <div className="px-6 py-4 border-b border-base flex items-center justify-between bg-surface/50 backdrop-blur-sm">
            <div>
              <h2 className="text-lg font-bold text-main">{initialData ? '修改副本记录' : '添加副本记录'}</h2>
              <p className="text-muted text-xs mt-0.5">
                <span className="font-medium text-main">{role.name}@{role.server}</span>
                <span className="mx-1.5 text-muted/40">·</span>
                {constructRaidName()}
              </p>
            </div>
            <button
              onClick={onClose}
              className="text-muted hover:text-main transition-colors p-2 rounded-lg hover:bg-base/50"
            >
              <X className="w-5 h-5" />
            </button>
          </div>

          <form onSubmit={handleSubmit} className="p-5 space-y-4">

            <div>
              <label className="flex items-center gap-2 text-sm font-medium text-main mb-1.5">
                <Calendar className="w-4 h-4 text-primary" />
                记录日期
              </label>
              <DateTimePicker
                value={recordDate}
                onChange={setRecordDate}
              />
            </div>

            {availableBosses.length > 0 && (
              <div>
                <label className="block text-sm font-medium text-main mb-1.5">
                  击败BOSS（可多选）
                </label>
                <div className="grid grid-cols-3 gap-2 p-3 bg-base rounded-lg border border-base">
                  {availableBosses.map((boss) => {
                    const isSelected = selectedBossIds.includes(boss.id);
                    return (
                      <label
                        key={boss.id}
                        className={`flex items-center gap-2 px-3 py-2 rounded-lg cursor-pointer transition-all ${isSelected
                          ? 'bg-primary text-white'
                          : 'bg-surface text-muted border border-base hover:border-primary hover:text-primary'
                          }`}
                      >
                        <input
                          type="checkbox"
                          checked={isSelected}
                          onChange={(e) => {
                            if (e.target.checked) {
                              setSelectedBossIds([...selectedBossIds, boss.id]);
                            } else {
                              setSelectedBossIds(selectedBossIds.filter(id => id !== boss.id));
                            }
                          }}
                          className="sr-only"
                        />
                        <span className="text-sm font-medium">{boss.name}</span>
                      </label>
                    );
                  })}
                </div>
                {selectedBossIds.length > 0 && (
                  <p className="text-xs text-muted mt-1">已选择 {selectedBossIds.length} 个BOSS</p>
                )}
              </div>
            )}

            {errorMessage && (
              <div className="p-3 bg-red-50 border border-red-200 rounded-lg">
                <p className="text-sm text-red-700 dark:text-red-400 flex items-center gap-2">
                  <AlertCircle className="w-4 h-4 flex-shrink-0" />
                  {errorMessage}
                </p>
              </div>
            )}

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
                    value={goldIncome || ''}
                    onChange={e => setGoldIncome(Number(e.target.value))}
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
                    value={goldExpense || ''}
                    onChange={e => setGoldExpense(Number(e.target.value))}
                    placeholder="支出金额"
                    className="w-full pl-9 pr-3 py-2.5 bg-surface border border-amber-200 dark:border-amber-800 rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-amber-400 transition-all font-mono text-[1rem]"
                  />
                </div>
              </div>
            </div>

            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 p-3 bg-base rounded-lg border border-base">
              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={hasXuanjing}
                  onChange={e => setHasXuanjing(e.target.checked)}
                  id="xuanjing"
                  className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                />
                <label htmlFor="xuanjing" className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                  <Sparkles className="w-3.5 h-3.5 text-amber-500" />
                  <span>玄晶</span>
                </label>
              </div>

              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={hasMaJu}
                  onChange={e => setHasMaJu(e.target.checked)}
                  id="maju"
                  className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                />
                <label htmlFor="maju" className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                  <Anchor className="w-3.5 h-3.5 text-blue-500" />
                  <span>马具</span>
                </label>
              </div>

              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={hasPet}
                  onChange={e => setHasPet(e.target.checked)}
                  id="pet"
                  className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                />
                <label htmlFor="pet" className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                  <Ghost className="w-3.5 h-3.5 text-purple-500" />
                  <span>宠物</span>
                </label>
              </div>

              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={hasPendant}
                  onChange={e => setHasPendant(e.target.checked)}
                  id="pendant"
                  className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                />
                <label htmlFor="pendant" className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                  <Package className="w-3.5 h-3.5 text-orange-500" />
                  <span>挂件</span>
                </label>
              </div>

              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={hasMount}
                  onChange={e => setHasMount(e.target.checked)}
                  id="mount"
                  className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                />
                <label htmlFor="mount" className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                  <Flag className="w-3.5 h-3.5 text-green-500" />
                  <span>坐骑</span>
                </label>
              </div>

              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={hasAppearance}
                  onChange={e => setHasAppearance(e.target.checked)}
                  id="appearance"
                  className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                />
                <label htmlFor="appearance" className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                  <Shirt className="w-3.5 h-3.5 text-pink-500" />
                  <span>外观</span>
                </label>
              </div>

              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={hasTitle}
                  onChange={e => setHasTitle(e.target.checked)}
                  id="title"
                  className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                />
                <label htmlFor="title" className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                  <Crown className="w-3.5 h-3.5 text-yellow-600" />
                  <span>称号</span>
                </label>
              </div>

              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={hasSecretBook}
                  onChange={e => setHasSecretBook(e.target.checked)}
                  id="secretbook"
                  className="w-4 h-4 text-primary rounded border-base focus:ring-primary"
                />
                <label htmlFor="secretbook" className="flex items-center gap-1.5 cursor-pointer text-sm text-main select-none">
                  <BookOpen className="w-3.5 h-3.5 text-cyan-600" />
                  <span>秘籍</span>
                </label>
              </div>
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
    </>,
    document.body
  );
};
