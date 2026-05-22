import React from 'react';
import { TrialBossEquipmentStats } from '../types';

interface TrialBossEquipmentStatsSectionProps {
  stats: TrialBossEquipmentStats[];
}

const formatBossSequence = (bosses: [string, string, string]) => {
  return bosses.map((boss, index) => `${index + 1}.${boss}`).join('  ');
};

const formatPositionSummary = (stat: TrialBossEquipmentStats) => {
  const positions = stat.positions
    .filter(position => position.appearanceCount > 0)
    .sort((a, b) => b.appearanceCount - a.appearanceCount || a.position - b.position);

  if (positions.length === 0) {
    return '-';
  }

  return positions
    .map(position => `${position.position}号 ${position.appearanceCount}`)
    .join(' / ');
};

export const TrialBossEquipmentStatsSection: React.FC<TrialBossEquipmentStatsSectionProps> = ({ stats }) => {
  return (
    <div className="rounded-xl border border-base bg-surface p-5 shadow-sm">
      <div className="mb-4">
        <h3 className="text-lg font-semibold text-main">Boss 出装备统计</h3>
        <p className="mt-1 text-sm text-muted">按三关 Boss 顺序统计装备出现的位置</p>
      </div>

      {stats.length === 0 ? (
        <div className="rounded-xl border border-dashed border-base px-4 py-8 text-center">
          <p className="text-muted">暂无 Boss 出装备统计</p>
        </div>
      ) : (
        <>
          <div className="hidden overflow-hidden rounded-xl border border-base md:block">
            <div className="grid grid-cols-[minmax(220px,1fr)_80px_88px_110px_minmax(170px,1fr)] bg-base/50 text-xs font-medium text-muted">
              <div className="px-4 py-3">Boss 顺序</div>
              <div className="px-4 py-3">记录数</div>
              <div className="px-4 py-3">出装备</div>
              <div className="px-4 py-3">高发位置</div>
              <div className="px-4 py-3">位置分布</div>
            </div>
            {stats.map(stat => (
              <div
                key={`trial-boss-equipment-${stat.bossKey}`}
                className="grid grid-cols-[minmax(220px,1fr)_80px_88px_110px_minmax(170px,1fr)] border-t border-base text-sm text-main"
              >
                <div className="px-4 py-3 font-medium break-words">{formatBossSequence(stat.bosses)}</div>
                <div className="px-4 py-3">{stat.totalRecords}</div>
                <div className="px-4 py-3">{stat.equipmentCount}</div>
                <div className="px-4 py-3 font-medium text-emerald-600">
                  {stat.bestEquipmentPosition ? `${stat.bestEquipmentPosition.position}号位` : '-'}
                </div>
                <div className="px-4 py-3 text-muted break-words">{formatPositionSummary(stat)}</div>
              </div>
            ))}
          </div>

          <div className="space-y-3 md:hidden">
            {stats.map(stat => (
              <div key={`trial-boss-equipment-mobile-${stat.bossKey}`} className="rounded-xl border border-base p-4">
                <div className="font-medium text-main break-words">{formatBossSequence(stat.bosses)}</div>
                <div className="mt-3 grid grid-cols-3 gap-2">
                  <div className="rounded-lg bg-base/40 px-3 py-2">
                    <div className="text-[11px] text-muted">记录数</div>
                    <div className="mt-1 font-semibold text-main">{stat.totalRecords}</div>
                  </div>
                  <div className="rounded-lg bg-base/40 px-3 py-2">
                    <div className="text-[11px] text-muted">出装备</div>
                    <div className="mt-1 font-semibold text-main">{stat.equipmentCount}</div>
                  </div>
                  <div className="rounded-lg bg-base/40 px-3 py-2">
                    <div className="text-[11px] text-muted">高发位置</div>
                    <div className="mt-1 font-semibold text-emerald-600">
                      {stat.bestEquipmentPosition ? `${stat.bestEquipmentPosition.position}号` : '-'}
                    </div>
                  </div>
                </div>
                <div className="mt-3 rounded-lg bg-base/40 px-3 py-2 text-xs text-muted">
                  {formatPositionSummary(stat)}
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
};
