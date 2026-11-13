import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, Loader2, Package, X } from 'lucide-react';
import { JX3Equip } from '../services/jx3BoxApi';
import { TrialDraw } from '../types';
import { TrialEquipmentCatalogState, TrialEquipmentPicker } from './TrialEquipmentPicker';

export type EquipmentCatalogState = TrialEquipmentCatalogState;

interface TrialDrawEquipmentEditorProps {
    draw: TrialDraw;
    equipments: JX3Equip[];
    catalogState: EquipmentCatalogState;
    catalogMessage: string | null;
    findEquipmentById: (id: string | undefined) => JX3Equip | null;
    /** 仅保存当前翻牌子记录的指定卡位，不能写入父通关记录。 */
    onUpdateEquipment: (cardIndex: number, equipmentId: string | null) => Promise<void> | void;
}

const CARD_KEYS = ['card1', 'card2', 'card3', 'card4', 'card5'] as const;

const getEquipmentId = (draw: TrialDraw, cardIndex: number): string => {
    const key = CARD_KEYS[cardIndex - 1];
    return key ? String(draw[key] || '').trim() : '';
};

export const TrialDrawEquipmentEditor: React.FC<TrialDrawEquipmentEditorProps> = ({
    draw, equipments, catalogState, catalogMessage, findEquipmentById, onUpdateEquipment,
}) => {
    const [isOpen, setIsOpen] = useState(false);
    const [selectedCard, setSelectedCard] = useState(draw.flippedIndex >= 1 && draw.flippedIndex <= 5 ? draw.flippedIndex : 1);
    const [savingCard, setSavingCard] = useState<number | null>(null);
    const [saveError, setSaveError] = useState<string | null>(null);
    const [lastSavedCard, setLastSavedCard] = useState<number | null>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);
    const closeButtonRef = useRef<HTMLButtonElement>(null);
    const dialogRef = useRef<HTMLDivElement>(null);
    const shouldRestoreFocusRef = useRef(false);
    const operationPendingRef = useRef(false);

    const cards = useMemo(() => CARD_KEYS.map((_, index) => {
        const cardIndex = index + 1;
        const equipId = getEquipmentId(draw, cardIndex);
        const equip = findEquipmentById(equipId);
        let label = '未记录';
        let tone = 'text-muted';
        if (equip) {
            label = equip.Name;
            tone = 'text-main';
        } else if (equipId) {
            label = catalogState === 'loading' ? '目录加载中…'
                : catalogState === 'error' ? `目录读取失败（ID ${equipId}）`
                    : catalogState === 'empty' ? `目录为空（ID ${equipId}）`
                        : `目录未找到（ID ${equipId}）`;
            tone = catalogState === 'loading' ? 'text-muted' : 'text-ds-warning-strong';
        }
        return { cardIndex, equipId, label, tone };
    }), [catalogState, draw, findEquipmentById]);

    const selectedCardEntry = cards[selectedCard - 1];
    const save = async (equipmentId: string | null) => {
        if (operationPendingRef.current) return;
        const cardIndex = selectedCard;
        operationPendingRef.current = true;
        setSavingCard(cardIndex);
        setSaveError(null);
        setLastSavedCard(null);
        try {
            await onUpdateEquipment(cardIndex, equipmentId);
            setLastSavedCard(cardIndex);
        } catch {
            setSaveError('保存装备失败，请重试');
        } finally {
            operationPendingRef.current = false;
            setSavingCard(null);
        }
    };

    const isSaving = savingCard !== null;
    const hasEquipment = cards.some(card => card.equipId);

    const openEditor = () => {
        shouldRestoreFocusRef.current = true;
        setSaveError(null);
        setLastSavedCard(null);
        setIsOpen(true);
    };

    const closeEditor = useCallback(() => {
        if (isSaving) return;
        setIsOpen(false);
    }, [isSaving]);

    useEffect(() => {
        if (!isOpen) {
            if (shouldRestoreFocusRef.current) {
                triggerRef.current?.focus();
                shouldRestoreFocusRef.current = false;
            }
            return;
        }

        const animationFrame = window.requestAnimationFrame(() => closeButtonRef.current?.focus());
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                event.preventDefault();
                closeEditor();
                return;
            }
            if (event.key !== 'Tab' || !dialogRef.current) return;

            const focusable = Array.from(dialogRef.current.querySelectorAll<HTMLElement>(
                'button:not([disabled]), input:not([disabled]), [href], select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
            ));
            if (focusable.length === 0) return;
            const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
            if (event.shiftKey && (currentIndex <= 0 || document.activeElement === dialogRef.current)) {
                event.preventDefault();
                focusable[focusable.length - 1].focus();
            } else if (!event.shiftKey && currentIndex === focusable.length - 1) {
                event.preventDefault();
                focusable[0].focus();
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => {
            window.cancelAnimationFrame(animationFrame);
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, [closeEditor, isOpen]);

    return (
        <div className="mt-2 border-t border-base/60 pt-2">
            <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-xs font-medium text-ds-fg-soft">装备：</span>
                {cards.filter(card => card.equipId).map(card => (
                    <span key={`${draw.id}-equipment-${card.cardIndex}`} title={`卡 ${card.cardIndex}：${card.label}`} className={`max-w-full truncate rounded-tag border border-base bg-surface px-1.5 py-0.5 text-[11px] ${card.cardIndex === draw.flippedIndex ? 'text-ds-success-strong' : card.tone}`}>
                        卡 {card.cardIndex}：{card.label}
                    </span>
                ))}
                {!hasEquipment && <span className="text-xs text-muted">本次无装备</span>}
                <button ref={triggerRef} type="button" onClick={openEditor} aria-haspopup="dialog" aria-label={hasEquipment ? '修改装备' : '补录装备'}
                    className="ml-auto inline-flex w-32 max-w-full items-center justify-center gap-1 whitespace-nowrap rounded-tag border border-ds-success-soft bg-ds-success-soft px-2 py-1.5 text-xs font-medium text-ds-success-strong transition-colors hover:bg-ds-success-soft/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong">
                    <Package className="h-3.5 w-3.5" />{hasEquipment ? '修改装备' : '补录装备'}
                </button>
            </div>
            {isOpen && createPortal(
                <div className="fixed inset-0 z-[110] flex items-center justify-center bg-ds-scrim p-4 backdrop-blur-sm" onClick={closeEditor}>
                    <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby={`trial-draw-equipment-title-${draw.id}`} className="flex h-[80vh] w-full max-w-5xl flex-col overflow-hidden rounded-card border border-base bg-surface shadow-ds-modal" onClick={event => event.stopPropagation()}>
                        <div className="flex items-center justify-between border-b border-base bg-base/50 px-4 py-3">
                            <h3 id={`trial-draw-equipment-title-${draw.id}`} className="flex items-center gap-2 text-[1rem] font-bold text-main">
                                <Package className="h-4 w-4 text-primary" />选择装备
                                <span className="rounded-tag border border-base bg-surface px-2 py-0.5 text-xs font-normal text-muted">#{selectedCard}</span>
                            </h3>
                            <button ref={closeButtonRef} type="button" disabled={isSaving} onClick={closeEditor} aria-label="关闭装备选择" className="rounded-tag p-1.5 text-muted transition-colors hover:bg-base hover:text-main disabled:cursor-not-allowed disabled:opacity-60">
                                <X className="h-5 w-5" />
                            </button>
                        </div>
                        <div className="shrink-0 border-b border-base bg-base/30 px-4 py-3">
                                <div className="rounded-card border border-base bg-surface p-2.5">
                                    <div className="flex flex-wrap items-center gap-1.5" role="group" aria-label="选择要补录的牌位">
                                        {cards.map(card => <button key={`${draw.id}-card-${card.cardIndex}`} type="button" disabled={isSaving}
                                            onClick={() => { setSelectedCard(card.cardIndex); setSaveError(null); setLastSavedCard(null); }} aria-pressed={selectedCard === card.cardIndex}
                                            title={`卡 ${card.cardIndex}：${card.label}`}
                                            className={`max-w-48 min-w-0 truncate rounded-tag border px-2 py-1 text-left text-[11px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong disabled:cursor-not-allowed disabled:opacity-60 ${selectedCard === card.cardIndex ? 'border-ds-success-soft bg-ds-success-soft text-ds-success-strong' : 'border-base bg-base/30 text-main hover:bg-base'}`}>
                                            <span className="font-medium">卡 {card.cardIndex}</span><span className="ml-1 text-muted">{card.label}</span>
                                        </button>)}
                                    </div>
                                    <div className="mt-2 flex min-h-5 items-center gap-1.5 text-xs text-muted" aria-live="polite">
                                        {savingCard === selectedCard && <><Loader2 className="h-3.5 w-3.5 animate-spin" />正在保存第 {selectedCard} 张牌位…</>}
                                        {!isSaving && lastSavedCard === selectedCard && <><Check className="h-3.5 w-3.5 text-ds-success-strong" />第 {selectedCard} 张牌位已保存</>}
                                        {!isSaving && lastSavedCard !== selectedCard && <span>当前第 {selectedCard} 张：{selectedCardEntry?.equipId ? '已记录装备，可重新选择或清空' : '尚未记录装备'}</span>}
                                    </div>
                                </div>
                        </div>
                        <div className="min-h-0 flex-1 overflow-y-auto bg-base/30 p-4">
                                <TrialEquipmentPicker
                                    equipments={equipments}
                                    catalogState={catalogState}
                                    catalogMessage={catalogMessage}
                                    selectedEquipmentId={selectedCardEntry?.equipId}
                                    onSelect={equipmentId => save(equipmentId)}
                                    onClear={() => save(null)}
                                    isSaving={isSaving}
                                />
                        </div>
                        <div className="shrink-0 border-t border-base bg-surface px-4 py-3">
                            {saveError && <div className="mb-2 rounded-card border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-600 dark:border-red-900/40 dark:bg-red-900/20 dark:text-red-300" role="alert">{saveError}</div>}
                            <div className="flex flex-wrap items-center gap-2">
                                <div className="ml-auto flex items-center gap-2">
                                    <button type="button" disabled={isSaving} onClick={closeEditor} className="rounded-tag bg-primary px-4 py-1.5 text-xs font-medium text-white transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ds-success-strong disabled:cursor-not-allowed disabled:opacity-60">
                                        完成
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>,
                document.body,
            )}
        </div>
    );
};
