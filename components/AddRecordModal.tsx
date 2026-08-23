import React, { useState, useEffect, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { Raid, RaidRecord, ScrapsItem } from '../types';
import { X, Calendar, Coins, Sparkles, FileText, TrendingUp, TrendingDown, AlertCircle, Shirt, Crown, Package, Ghost, Anchor, Flag, BookOpen, Boxes, ScrollText, Swords, Check } from 'lucide-react';
import { generateUUID } from '../utils/uuid';
import { logOperation } from '../utils/cooldownManager';
import { getBaseServerName } from '../utils/serverUtils';
import { DateTimePicker } from './DateTimePicker';
import { computeScrapsValue, normalizeScrapsItems } from '../utils/scrapsUtils';
import { ScrapsItemsEditor } from './ScrapsItemsEditor';

interface RoleWithStatus {
  id: string;
  name: string;
  server: string;
  region: string;
  sect?: string;
  accountId: string;
  accountName: string;
  bossCooldowns?: any[]; // Using any[] for now to avoid circular dependency issues
}

interface AddRecordModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (record: Partial<RaidRecord>) => void;
  raid: Raid;
  role: RoleWithStatus;
  initialData?: RaidRecord;
}

/** 8 种特殊掉落项配置：图标颜色 + 文案，便于渲染卡片式复选框 */
const SPECIAL_DROP_ITEMS: Array<{
  id: string;
  key: keyof Pick<
    RaidRecord,
    'hasXuanjing' | 'hasMaJu' | 'hasPet' | 'hasPendant' | 'hasMount' | 'hasAppearance' | 'hasTitle' | 'hasSecretBook'
  >;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  iconColor: string;
}> = [
  { id: 'xuanjing',    key: 'hasXuanjing',   label: '玄晶', icon: Sparkles,  iconColor: 'text-ds-warning-strong' },
  { id: 'maju',        key: 'hasMaJu',       label: '马具', icon: Anchor,    iconColor: 'text-blue-500' },
  { id: 'pet',         key: 'hasPet',        label: '宠物', icon: Ghost,     iconColor: 'text-purple-500' },
  { id: 'pendant',     key: 'hasPendant',    label: '挂件', icon: Package,   iconColor: 'text-orange-500' },
  { id: 'mount',       key: 'hasMount',      label: '坐骑', icon: Flag,      iconColor: 'text-green-500' },
  { id: 'appearance',  key: 'hasAppearance', label: '外观', icon: Shirt,     iconColor: 'text-pink-500' },
  { id: 'title',       key: 'hasTitle',      label: '称号', icon: Crown,     iconColor: 'text-yellow-600' },
  { id: 'secretbook',  key: 'hasSecretBook', label: '秘籍', icon: BookOpen,  iconColor: 'text-cyan-600' },
];

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
  const [isScrapsBoss, setIsScrapsBoss] = useState(false);
  const [scrapsItems, setScrapsItems] = useState<ScrapsItem[]>([]);

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
      // 锁定背景滚动，避免弹窗时页面可上下滚动
      const originalOverflow = document.body.style.overflow;
      document.body.style.overflow = 'hidden';

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
        setIsScrapsBoss(initialData.isScrapsBoss ?? false);
        setScrapsItems(normalizeScrapsItems(initialData.scrapsItems));
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
        // 默认只选择未完成的boss
        setSelectedBossIds(
          role.bossCooldowns
            ? role.bossCooldowns
                .filter(boss => !boss.hasRecord)
                .map(boss => boss.bossId)
            : availableBosses.map(b => b.id)
        );
        setRecordDate(formatDateForInput(new Date()));
        setIsScrapsBoss(false);
        setScrapsItems([]);
      }
      setIsSubmitting(false);
      setErrorMessage(null);

      // 恢复背景滚动
      return () => {
        document.body.style.overflow = originalOverflow;
      };
    }
  }, [isOpen, initialData]);

  const constructRaidName = (): string => {
    return `${raid.playerCount}人${raid.difficulty}${raid.name}`;
  };

  /** 将 item.key 映射到对应的 setter，避免在 JSX 中写一长串条件分支 */
  const specialDropSetters: Record<typeof SPECIAL_DROP_ITEMS[number]['key'], (v: boolean) => void> = {
    hasXuanjing: setHasXuanjing,
    hasMaJu: setHasMaJu,
    hasPet: setHasPet,
    hasPendant: setHasPendant,
    hasMount: setHasMount,
    hasAppearance: setHasAppearance,
    hasTitle: setHasTitle,
    hasSecretBook: setHasSecretBook,
  };

  /** 将 item.key 映射到当前勾选状态 */
  const specialDropValues: Record<typeof SPECIAL_DROP_ITEMS[number]['key'], boolean> = {
    hasXuanjing,
    hasMaJu,
    hasPet,
    hasPendant,
    hasMount,
    hasAppearance,
    hasTitle,
    hasSecretBook,
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage(null);

    if (goldIncome < 0 || goldExpense < 0) {
      logOperation(role.id, role.name, constructRaidName(), false, '金额不能小于0');
      setErrorMessage('收入和支出不能小于0');
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
        server: role.server,
        transactionType: 'combined',
        bossIds: selectedBossIds.length > 0 ? selectedBossIds : undefined,
        bossNames: selectedBossIds.map(id => availableBosses.find(b => b.id === id)?.name).filter(Boolean) as string[] || undefined,
        isScrapsBoss,
        scrapsItems: scrapsItems.length > 0 ? scrapsItems : undefined,
        scrapsValue: scrapsItems.length > 0 ? computeScrapsValue(scrapsItems) : undefined,
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

  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  if (!isOpen) return null;

  const hasBosses = availableBosses.length > 0;

  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm overflow-hidden animate-in fade-in duration-200"
      onClick={handleBackdropClick}
      data-od-id="add-record-modal-backdrop"
    >
      <div
        className="bg-surface w-full max-w-2xl rounded-xl shadow-ds-modal border border-base overflow-hidden animate-in zoom-in-95 duration-200 max-h-[90vh] flex flex-col"
        onClick={e => e.stopPropagation()}
        data-od-id="add-record-modal"
      >
        {/* 标题区 */}
        <div
          className="flex items-center justify-between gap-3 px-6 py-4 border-b border-base bg-base/50 backdrop-blur-sm flex-shrink-0"
          data-od-id="add-record-modal-header"
        >
          <div className="flex items-center gap-3 min-w-0 flex-1">
            <div className="p-2 bg-ds-success-soft dark:bg-ds-success-soft/30 rounded-lg border border-ds-success-soft/50 dark:border-ds-success-soft/50 flex-shrink-0">
              <ScrollText className="w-4 h-4 text-ds-success-strong/80" />
            </div>
            <div className="min-w-0">
              <h2 className="text-base font-bold text-main truncate">
                {initialData ? '修改副本记录' : '添加副本记录'}
              </h2>
              <p className="text-xs text-muted mt-0.5 truncate">
                <span className="font-medium text-main">{role.name}·{getBaseServerName(role.server)}</span>
                <span className="mx-1.5 text-muted/40">·</span>
                <span>{role.accountName}</span>
                <span className="mx-1.5 text-muted/40">·</span>
                <span>{constructRaidName()}</span>
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-base text-muted hover:text-main transition-colors flex-shrink-0"
            title="关闭"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* 表单主体 */}
        <form onSubmit={handleSubmit} className="flex-1 flex flex-col overflow-hidden">
          <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5">

            {/* 记录日期与击败 BOSS */}
            <div
              className="grid grid-cols-1 sm:grid-cols-2 gap-4"
              data-od-id="add-record-modal-top-fields"
            >
              <div>
                <label className="flex items-center gap-1.5 text-xs font-medium text-muted mb-2">
                  <Calendar className="w-3.5 h-3.5 text-primary" />
                  记录日期
                </label>
                <DateTimePicker
                  value={recordDate}
                  onChange={setRecordDate}
                />
              </div>

              {hasBosses && (
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <label className="flex items-center gap-1.5 text-xs font-medium text-muted">
                      <Swords className="w-3.5 h-3.5 text-primary" />
                      击败 BOSS
                      <span className="text-[10px] font-normal text-muted/70">（可多选）</span>
                    </label>
                    {selectedBossIds.length > 0 && (
                      <span className="text-[10px] font-medium text-ds-success-strong dark:text-ds-success-strong bg-ds-success-soft dark:bg-ds-success-soft/30 px-2 py-0.5 rounded-full border border-ds-success-soft/60 dark:border-ds-success-soft/60">
                        已选 {selectedBossIds.length} 个
                      </span>
                    )}
                  </div>
                  <div className="flex flex-wrap gap-1.5 p-2 bg-base/60 rounded-lg border border-base max-h-24 overflow-y-auto">
                    {availableBosses.map((boss) => {
                      const isSelected = selectedBossIds.includes(boss.id);
                      return (
                        <button
                          key={boss.id}
                          type="button"
                          onClick={() => {
                            if (isSelected) {
                              setSelectedBossIds(selectedBossIds.filter(id => id !== boss.id));
                            } else {
                              setSelectedBossIds([...selectedBossIds, boss.id]);
                            }
                          }}
                          className={`px-2 py-1 rounded-md text-xs font-medium border transition-all ${
                            isSelected
                              ? 'bg-ds-success text-white border-ds-success hover:bg-ds-success'
                              : 'bg-surface text-muted border-base hover:border-ds-success hover:text-ds-success-strong dark:hover:text-ds-success-strong hover:bg-ds-success-soft/50'
                          }`}
                        >
                          {boss.name}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>

            {/* 错误提示 */}
            {errorMessage && (
              <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg flex items-center gap-2">
                <AlertCircle className="w-4 h-4 text-red-600 flex-shrink-0" />
                <p className="text-sm text-red-700 dark:text-red-400">{errorMessage}</p>
              </div>
            )}

            {/* 收支情况 */}
            <section data-od-id="add-record-modal-income-section">
              <header className="flex items-center gap-2 mb-3">
                <div className="w-1 h-4 bg-primary rounded"></div>
                <h3 className="text-sm font-semibold text-main">收支情况</h3>
              </header>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="flex items-center gap-1.5 text-xs font-medium text-muted mb-1.5">
                    <TrendingUp className="w-3.5 h-3.5 text-ds-success-strong" />
                    金币收入
                  </label>
                  <div className="relative">
                    <div className="absolute left-2 top-1/2 -translate-y-1/2 w-6 h-6 rounded-md bg-ds-success-soft dark:bg-ds-success-soft/40 flex items-center justify-center">
                      <span className="text-xs font-bold text-ds-success-strong dark:text-ds-success-strong">+</span>
                    </div>
                    <input
                      type="number"
                      min="0"
                      value={goldIncome || ''}
                      onChange={e => setGoldIncome(Number(e.target.value))}
                      placeholder="0"
                      className="w-full pl-11 pr-9 py-2 bg-ds-success-soft/40 dark:bg-ds-success-soft/10 border border-ds-success-soft dark:border-ds-success-soft rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-ds-success focus:border-ds-success transition-all font-mono text-[1rem]"
                    />
                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs font-medium text-muted">金</span>
                  </div>
                </div>

                <div>
                  <label className="flex items-center gap-1.5 text-xs font-medium text-muted mb-1.5">
                    <TrendingDown className="w-3.5 h-3.5 text-ds-warning-strong" />
                    金币支出
                  </label>
                  <div className="relative">
                    <div className="absolute left-2 top-1/2 -translate-y-1/2 w-6 h-6 rounded-md bg-ds-warning-soft dark:bg-ds-warning-soft/40 flex items-center justify-center">
                      <span className="text-xs font-bold text-ds-warning-strong dark:text-ds-warning-strong">−</span>
                    </div>
                    <input
                      type="number"
                      min="0"
                      value={goldExpense || ''}
                      onChange={e => setGoldExpense(Number(e.target.value))}
                      placeholder="0"
                      className="w-full pl-11 pr-9 py-2 bg-ds-warning-soft/40 dark:bg-ds-warning-soft/10 border border-ds-warning-soft dark:border-ds-warning-soft rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-ds-warning focus:border-ds-warning transition-all font-mono text-[1rem]"
                    />
                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs font-medium text-muted">金</span>
                  </div>
                </div>
              </div>
            </section>

            {/* 特殊掉落 */}
            <section data-od-id="add-record-modal-drops-section">
              <header className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  <div className="w-1 h-4 bg-primary rounded"></div>
                  <h3 className="text-sm font-semibold text-main">特殊掉落</h3>
                </div>
                <span className="text-[10px] text-muted">勾选对应稀有掉落</span>
              </header>
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                {SPECIAL_DROP_ITEMS.map(item => {
                  const Icon = item.icon;
                  const checked = specialDropValues[item.key];
                  const setter = specialDropSetters[item.key];
                  return (
                    <label
                      key={item.id}
                      htmlFor={item.id}
                      className={`flex items-center gap-2 px-2.5 py-2 rounded-lg border cursor-pointer transition-all select-none ${
                        checked
                          ? 'bg-ds-success-soft dark:bg-ds-success-soft/20 border-ds-success dark:border-ds-success'
                          : 'bg-surface border-base hover:border-ds-success hover:bg-ds-success-soft/40 dark:hover:bg-ds-success-soft/10'
                      }`}
                    >
                      <input
                        type="checkbox"
                        id={item.id}
                        checked={checked}
                        onChange={e => setter(e.target.checked)}
                        className="w-3.5 h-3.5 text-primary rounded border-base focus:ring-primary focus:ring-2 flex-shrink-0"
                      />
                      <Icon className={`w-3.5 h-3.5 flex-shrink-0 ${checked ? 'text-ds-success-strong' : item.iconColor}`} />
                      <span className={`text-sm ${checked ? 'text-main font-medium' : 'text-main'}`}>{item.label}</span>
                      {checked && <Check className="w-3 h-3 text-ds-success-strong ml-auto flex-shrink-0" />}
                    </label>
                  );
                })}
              </div>
            </section>

            {/* 散件老板 */}
            <section data-od-id="add-record-modal-scraps-section">
              <header className="flex items-center justify-between mb-3">
                <label htmlFor="scraps-boss" className="flex items-center gap-2 cursor-pointer select-none">
                  <div className="w-1 h-4 bg-primary rounded"></div>
                  <input
                    type="checkbox"
                    id="scraps-boss"
                    checked={isScrapsBoss}
                    onChange={e => setIsScrapsBoss(e.target.checked)}
                    className="w-3.5 h-3.5 text-ds-success-strong rounded border-base focus:ring-ds-success"
                  />
                  <Boxes className={`w-3.5 h-3.5 ${isScrapsBoss ? 'text-ds-success-strong' : 'text-muted'}`} />
                  <h3 className="text-sm font-semibold text-main">散件老板</h3>
                </label>
                <span className="text-[10px] text-muted">勾选后估价计入统计</span>
              </header>

              {isScrapsBoss && (
                <div className="mt-2 p-3 bg-base/30 rounded-lg border border-base">
                  <ScrapsItemsEditor
                    items={scrapsItems}
                    onChange={setScrapsItems}
                    editable
                    allowAddCustom
                    isScrapsBoss={isScrapsBoss}
                    listMaxHeightClass="none"
                  />
                </div>
              )}
            </section>

            {/* 备注 */}
            <section data-od-id="add-record-modal-notes-section">
              <label className="flex items-center gap-1.5 text-xs font-medium text-muted mb-2">
                <FileText className="w-3.5 h-3.5 text-primary" />
                备注
                <span className="text-[10px] font-normal text-muted/70">（可选）</span>
              </label>
              <textarea
                value={notes}
                onChange={e => setNotes(e.target.value)}
                placeholder="可补充特殊情况、分配方式等..."
                rows={2}
                className="w-full px-3 py-2 bg-surface border border-base rounded-lg text-main placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-primary/40 focus:border-primary transition-all resize-none text-sm"
              />
            </section>
          </div>

          {/* 底部操作栏 */}
          <div className="flex gap-3 px-6 py-4 border-t border-base bg-base/40 backdrop-blur-sm flex-shrink-0">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 py-2 rounded-lg border border-base text-main font-medium hover:bg-base transition-colors text-sm"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={isSubmitting || (goldIncome < 0 && goldExpense < 0)}
              className="flex-1 py-2 rounded-lg bg-primary text-white font-medium hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 text-sm"
            >
              {isSubmitting ? (
                <>
                  <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  <span>提交中...</span>
                </>
              ) : (
                <>
                  <Coins className="w-4 h-4" />
                  <span>{initialData ? '保存修改' : '确认添加'}</span>
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