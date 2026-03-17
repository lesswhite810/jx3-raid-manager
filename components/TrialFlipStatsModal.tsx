import React from 'react';
import { X } from 'lucide-react';
import { TrialFlipStatsSummary } from '../types';

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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 px-4 py-6">
      <div className="w-full max-w-4xl overflow-hidden rounded-2xl border border-base bg-surface shadow-xl">
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

        <div className="space-y-5 px-6 py-5">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <div className="rounded-xl border border-base bg-base/40 px-4 py-3">
              <div className="text-xs text-muted">总记录数</div>
              <div className="mt-1 text-2xl font-semibold text-main">{stats.totalRecords}</div>
            </div>
            <div className="rounded-xl border border-base bg-base/40 px-4 py-3">
              <div className="text-xs text-muted">翻牌装备率最高</div>
              <div className="mt-1 text-base font-semibold text-main">
                {stats.bestFlipPosition ? `${stats.bestFlipPosition.position}号位` : '-'}
              </div>
              <div className="mt-1 text-sm text-emerald-600">
                {stats.bestFlipPosition
                  ? `${(stats.bestFlipPosition.flipEquipmentRate * 100).toFixed(1)}%`
                  : '0%'}
              </div>
            </div>
            <div className="rounded-xl border border-base bg-base/40 px-4 py-3">
              <div className="text-xs text-muted">位置出现率最高</div>
              <div className="mt-1 text-base font-semibold text-main">
                {stats.bestAppearancePosition ? `${stats.bestAppearancePosition.position}号位` : '-'}
              </div>
              <div className="mt-1 text-sm text-main">
                {stats.bestAppearancePosition
                  ? `${(stats.bestAppearancePosition.appearanceRate * 100).toFixed(1)}%`
                  : '0%'}
              </div>
            </div>
          </div>

          {stats.totalRecords === 0 ? (
            <div className="rounded-xl border border-dashed border-base px-4 py-10 text-center">
              <p className="text-muted">暂无翻牌记录</p>
              <p className="mt-1 text-sm text-muted/70">新增试炼记录后会自动生成统计</p>
            </div>
          ) : (
            <>
              <div className="hidden overflow-hidden rounded-xl border border-base md:block">
                <div className="grid grid-cols-[88px_96px_110px_116px_110px_116px] bg-base/50 text-xs font-medium text-muted">
                  <div className="px-4 py-3">位置</div>
                  <div className="px-4 py-3">翻牌次数</div>
                  <div className="px-4 py-3">翻中装备</div>
                  <div className="px-4 py-3">翻牌装备率</div>
                  <div className="px-4 py-3">出现次数</div>
                  <div className="px-4 py-3">位置出现率</div>
                </div>
                {stats.positions.map((position) => (
                  <div
                    key={`trial-flip-modal-${position.position}`}
                    className="grid grid-cols-[88px_96px_110px_116px_110px_116px] border-t border-base text-sm text-main"
                  >
                    <div className="px-4 py-3 font-medium">{position.position}号位</div>
                    <div className="px-4 py-3">{position.flipCount}</div>
                    <div className="px-4 py-3">{position.flippedEquipmentCount}</div>
                    <div className="px-4 py-3 font-medium text-emerald-600">
                      {(position.flipEquipmentRate * 100).toFixed(1)}%
                    </div>
                    <div className="px-4 py-3">{position.appearanceCount}</div>
                    <div className="px-4 py-3">{(position.appearanceRate * 100).toFixed(1)}%</div>
                  </div>
                ))}
              </div>

              <div className="space-y-3 md:hidden">
                {stats.positions.map((position) => (
                  <div key={`trial-flip-modal-mobile-${position.position}`} className="rounded-xl border border-base p-4">
                    <div className="flex items-center justify-between">
                      <div className="font-medium text-main">{position.position}号位</div>
                      <div className="text-xs font-semibold text-emerald-600">
                        翻牌装备率 {(position.flipEquipmentRate * 100).toFixed(1)}%
                      </div>
                    </div>
                    <div className="mt-3 grid grid-cols-2 gap-3">
                      <div className="rounded-lg bg-base/40 px-3 py-2">
                        <div className="text-[11px] text-muted">翻牌次数</div>
                        <div className="mt-1 font-semibold text-main">{position.flipCount}</div>
                      </div>
                      <div className="rounded-lg bg-base/40 px-3 py-2">
                        <div className="text-[11px] text-muted">翻中装备</div>
                        <div className="mt-1 font-semibold text-main">{position.flippedEquipmentCount}</div>
                      </div>
                      <div className="rounded-lg bg-base/40 px-3 py-2">
                        <div className="text-[11px] text-muted">出现次数</div>
                        <div className="mt-1 font-semibold text-main">{position.appearanceCount}</div>
                      </div>
                      <div className="rounded-lg bg-base/40 px-3 py-2">
                        <div className="text-[11px] text-muted">位置出现率</div>
                        <div className="mt-1 font-semibold text-main">{(position.appearanceRate * 100).toFixed(1)}%</div>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
};
