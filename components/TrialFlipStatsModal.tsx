import React from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { TrialFlipStatsSummary } from '../types';
import { TrialBossEquipmentStatsSection } from './TrialBossEquipmentStatsSection';

interface TrialFlipStatsModalProps {
  isOpen: boolean;
  onClose: () => void;
  roleName: string;
  server: string;
  stats: TrialFlipStatsSummary;
}

export const TrialFlipStatsModal: React.FC<TrialFlipStatsModalProps> = ({
  isOpen,
  onClose,
  roleName,
  server,
  stats
}) => {
  if (!isOpen) {
    return null;
  }

  const formatEquipmentRate = (rate: number, denominator: number): string => {
    if (denominator > 0) {
      return `${(rate * 100).toFixed(1)}%`;
    }
    return stats.drawCount > 0 && stats.equipmentReviewedRecords === 0 ? '待补录' : '—';
  };

  const formatEquipmentCount = (count: number, denominator: number): string => {
    if (denominator > 0) {
      return `${count} / ${denominator}`;
    }
    return stats.drawCount > 0 && stats.equipmentReviewedRecords === 0 ? '待补录' : '—';
  };

  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-ds-scrim backdrop-blur-sm overflow-hidden"
      onClick={onClose}
    >
      <div
        className="w-full max-w-4xl overflow-hidden rounded-2xl border border-base bg-surface shadow-ds-modal animate-in fade-in zoom-in-95 duration-200"
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4 border-b border-base px-6 py-5">
          <div>
            <h3 className="text-xl font-semibold text-main">翻牌统计</h3>
            <p className="mt-1 text-sm text-muted">
              {roleName}@{server} · 全部历史
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-2 text-muted transition-colors hover:bg-base hover:text-main"
            title="关闭"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="space-y-5 px-6 py-5 max-h-[80vh] overflow-y-auto">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <div className="rounded-xl border border-base bg-base/40 px-4 py-3">
              <div className="text-xs text-muted">通关记录</div>
              <div className="mt-1 text-2xl font-semibold text-main">{stats.totalRecords}</div>
              <div className="mt-1 text-xs text-muted">
                可靠翻牌 {stats.drawCount} 次，已识别牌位 {stats.knownPositionDrawCount} 次
              </div>
              <div className="mt-1 text-xs text-muted">
                概率统计使用已确认牌位的 {stats.equipmentReviewedRecords} 次翻牌
              </div>
            </div>
            <div className="rounded-xl border border-base bg-base/40 px-4 py-3">
              <div className="text-xs text-muted">翻牌装备率最高</div>
              <div className="mt-1 text-base font-semibold text-main">
                {stats.bestFlipPosition ? `${stats.bestFlipPosition.position}号位` : '-'}
              </div>
              <div className="mt-1 text-sm text-ds-success-strong">
                {stats.bestFlipPosition
                  ? `${formatEquipmentRate(
                    stats.bestFlipPosition.flipEquipmentRate,
                    stats.bestFlipPosition.equipmentReviewCount,
                  )}（${stats.bestFlipPosition.flippedEquipmentCount}/${stats.bestFlipPosition.equipmentReviewCount}）`
                  : formatEquipmentRate(0, 0)}
              </div>
            </div>
            <div className="rounded-xl border border-base bg-base/40 px-4 py-3">
              <div className="text-xs text-muted">位置出现率最高</div>
              <div className="mt-1 text-base font-semibold text-main">
                {stats.bestAppearancePosition ? `${stats.bestAppearancePosition.position}号位` : '-'}
              </div>
              <div className="mt-1 text-sm text-main">
                {stats.bestAppearancePosition
                  ? `${formatEquipmentRate(
                    stats.bestAppearancePosition.appearanceRate,
                    stats.equipmentReviewedRecords,
                  )}（${stats.bestAppearancePosition.appearanceCount}/${stats.equipmentReviewedRecords}）`
                  : formatEquipmentRate(0, 0)}
              </div>
            </div>
          </div>

          {stats.drawCount === 0 ? (
            <div className="rounded-xl border border-dashed border-base px-4 py-10 text-center">
              <p className="text-muted">{stats.totalRecords === 0 ? '暂无通关记录' : '尚未记录到可靠翻牌'}</p>
              <p className="mt-1 text-sm text-muted/70">
                {stats.totalRecords === 0 ? '通关后会自动汇总翻牌数据' : '已记录的通关不会重复计入，等待入包或牌位识别结果'}
              </p>
            </div>
          ) : (
            <>
              <div className="hidden overflow-hidden rounded-xl border border-base md:block">
                <div className="grid grid-cols-[88px_96px_110px_116px_110px_116px] bg-base/50 text-xs font-medium text-muted">
                  <div className="px-4 py-3">位置</div>
                  <div className="px-4 py-3">翻牌次数</div>
                  <div className="px-4 py-3">翻中装备/已确认</div>
                  <div className="px-4 py-3">翻牌装备率</div>
                  <div className="px-4 py-3">出现次数/已确认</div>
                  <div className="px-4 py-3">位置出现率</div>
                </div>
                {stats.positions.map((position) => (
                  <div
                    key={`trial-flip-modal-${position.position}`}
                    className="grid grid-cols-[88px_96px_110px_116px_110px_116px] border-t border-base text-sm text-main"
                  >
                    <div className="px-4 py-3 font-medium">{position.position}号位</div>
                    <div className="px-4 py-3">{position.flipCount}</div>
                    <div className="px-4 py-3">{formatEquipmentCount(position.flippedEquipmentCount, position.equipmentReviewCount)}</div>
                    <div className="px-4 py-3 font-medium text-ds-success">
                      {formatEquipmentRate(position.flipEquipmentRate, position.equipmentReviewCount)}
                    </div>
                    <div className="px-4 py-3">{formatEquipmentCount(position.appearanceCount, stats.equipmentReviewedRecords)}</div>
                    <div className="px-4 py-3">{formatEquipmentRate(position.appearanceRate, stats.equipmentReviewedRecords)}</div>
                  </div>
                ))}
              </div>

              <div className="space-y-3 md:hidden">
                {stats.positions.map((position) => (
                  <div key={`trial-flip-modal-mobile-${position.position}`} className="rounded-xl border border-base p-4">
                    <div className="flex items-center justify-between">
                      <div className="font-medium text-main">{position.position}号位</div>
                      <div className="text-xs font-semibold text-ds-success-strong">
                        翻牌装备率 {formatEquipmentRate(position.flipEquipmentRate, position.equipmentReviewCount)}
                      </div>
                    </div>
                    <div className="mt-3 grid grid-cols-2 gap-3">
                      <div className="rounded-lg bg-base/40 px-3 py-2">
                        <div className="text-[11px] text-muted">翻牌次数</div>
                        <div className="mt-1 font-semibold text-main">{position.flipCount}</div>
                      </div>
                      <div className="rounded-lg bg-base/40 px-3 py-2">
                        <div className="text-[11px] text-muted">翻中装备/已确认</div>
                        <div className="mt-1 font-semibold text-main">{formatEquipmentCount(position.flippedEquipmentCount, position.equipmentReviewCount)}</div>
                      </div>
                      <div className="rounded-lg bg-base/40 px-3 py-2">
                        <div className="text-[11px] text-muted">出现次数/已确认</div>
                        <div className="mt-1 font-semibold text-main">{formatEquipmentCount(position.appearanceCount, stats.equipmentReviewedRecords)}</div>
                      </div>
                      <div className="rounded-lg bg-base/40 px-3 py-2">
                        <div className="text-[11px] text-muted">位置出现率</div>
                        <div className="mt-1 font-semibold text-main">{formatEquipmentRate(position.appearanceRate, stats.equipmentReviewedRecords)}</div>
                      </div>
                    </div>
                  </div>
                ))}
              </div>

              <TrialBossEquipmentStatsSection
                stats={stats.bossEquipmentStats}
                equipmentReviewedRecords={stats.equipmentReviewedRecords}
              />
            </>
          )}
        </div>
      </div>
    </div>,
    document.body
  );
};
