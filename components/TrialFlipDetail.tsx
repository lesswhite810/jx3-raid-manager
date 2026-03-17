import React, { useMemo, useState, useEffect } from 'react';
import { ArrowLeft } from 'lucide-react';
import { TrialPlaceRecord } from '../types';
import { db } from '../services/db';
import { calculateTrialFlipStats } from '../utils/trialFlipStats';

interface TrialFlipDetailProps {
  trialRecords: TrialPlaceRecord[];
  onBack: () => void;
}

export const TrialFlipDetail: React.FC<TrialFlipDetailProps> = ({ trialRecords, onBack }) => {
  const [equipments, setEquipments] = useState<any[]>([]);

  useEffect(() => {
    db.getEquipments().then((data: any[]) => {
      setEquipments(data.map(d => typeof d === 'string' ? JSON.parse(d) : d));
    }).catch(console.error);
  }, []);

  const findEquipmentById = (id: string | undefined) => {
    if (!id || !id.trim()) return null;
    return equipments.find(e => e.ID?.toString() === id) || null;
  };

  const safeTrialRecords = Array.isArray(trialRecords) ? trialRecords : [];
  const stats = useMemo(() => calculateTrialFlipStats(safeTrialRecords), [safeTrialRecords]);

  const tradableEquipCount = useMemo(() => {
    let count = 0;
    safeTrialRecords.forEach(record => {
      const equipId = (record as any)[`card${record.flippedIndex}`];
      if (!equipId) {
        return;
      }
      const equip = findEquipmentById(equipId);
      if (equip && (equip.BindType === 1 || equip.BindType === 2)) {
        count += 1;
      }
    });
    return count;
  }, [safeTrialRecords, equipments]);

  return (
    <div className="space-y-5">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-4">
          <button
            onClick={onBack}
            className="rounded-lg p-2 transition-colors hover:bg-base"
          >
            <ArrowLeft className="w-5 h-5 text-muted" />
          </button>
          <div>
            <h2 className="text-2xl font-bold text-main">试炼翻牌统计</h2>
            <p className="mt-1 text-sm text-muted">
              全部历史共 {stats.totalRecords} 条记录，统计 1-5 号位翻牌次数、翻中装备率和位置出现率
            </p>
          </div>
        </div>
        <div className="rounded-full border border-base bg-surface px-3 py-1 text-xs text-muted">
          全部历史
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 md:grid-cols-4">
        <div className="rounded-xl border border-base bg-surface p-5 shadow-sm">
          <div className="text-sm font-medium text-muted">总记录数</div>
          <div className="mt-2 text-3xl font-bold text-main">{stats.totalRecords}</div>
        </div>
        <div className="rounded-xl border border-base bg-surface p-5 shadow-sm">
          <div className="text-sm font-medium text-muted">可交易装备数</div>
          <div className="mt-2 text-3xl font-bold text-main">{tradableEquipCount}</div>
        </div>
        <div className="rounded-xl border border-base bg-surface p-5 shadow-sm">
          <div className="text-sm font-medium text-muted">翻牌装备率最高</div>
          <div className="mt-2 text-xl font-bold text-main">
            {stats.bestFlipPosition ? `${stats.bestFlipPosition.position}号位` : '-'}
          </div>
          <div className="mt-1 text-sm text-emerald-600">
            {stats.bestFlipPosition
              ? `${(stats.bestFlipPosition.flipEquipmentRate * 100).toFixed(1)}%`
              : '0%'}
          </div>
        </div>
        <div className="rounded-xl border border-base bg-surface p-5 shadow-sm">
          <div className="text-sm font-medium text-muted">位置出现率最高</div>
          <div className="mt-2 text-xl font-bold text-main">
            {stats.bestAppearancePosition ? `${stats.bestAppearancePosition.position}号位` : '-'}
          </div>
          <div className="mt-1 text-sm text-main">
            {stats.bestAppearancePosition
              ? `${(stats.bestAppearancePosition.appearanceRate * 100).toFixed(1)}%`
              : '0%'}
          </div>
        </div>
      </div>

      <div className="rounded-xl border border-base bg-surface p-5 shadow-sm">
        <div className="mb-4">
          <h3 className="text-lg font-semibold text-main">位置明细</h3>
          <p className="mt-1 text-sm text-muted">按位置统计翻牌次数、翻中装备次数与出现次数</p>
        </div>

        {stats.totalRecords === 0 ? (
          <div className="rounded-xl border border-dashed border-base px-4 py-10 text-center">
            <p className="text-muted">暂无试炼翻牌记录</p>
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
              {stats.positions.map(position => (
                <div
                  key={`trial-flip-detail-${position.position}`}
                  className="grid grid-cols-[88px_96px_110px_116px_110px_116px] border-t border-base text-sm text-main"
                >
                  <div className="px-4 py-3 font-medium">{position.position}号位</div>
                  <div className="px-4 py-3">{position.flipCount}</div>
                  <div className="px-4 py-3">{position.flippedEquipmentCount}</div>
                  <div className="px-4 py-3 font-medium text-emerald-600">{(position.flipEquipmentRate * 100).toFixed(1)}%</div>
                  <div className="px-4 py-3">{position.appearanceCount}</div>
                  <div className="px-4 py-3">{(position.appearanceRate * 100).toFixed(1)}%</div>
                </div>
              ))}
            </div>

            <div className="space-y-3 md:hidden">
              {stats.positions.map(position => (
                <div key={`trial-flip-detail-mobile-${position.position}`} className="rounded-xl border border-base p-4">
                  <div className="flex items-center justify-between mb-3">
                    <div className="font-medium text-main">{position.position}号位</div>
                    <div className="text-xs font-semibold text-emerald-600">
                      翻牌装备率 {(position.flipEquipmentRate * 100).toFixed(1)}%
                    </div>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
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
  );
};
