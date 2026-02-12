import React from 'react';
import { BossCooldownInfo } from '../types';
import { Check, Clock } from 'lucide-react';

interface BossCooldownDisplayProps {
  bossCooldowns: BossCooldownInfo[];
  onBossClick?: (bossId: string, bossName: string) => void;
  compact?: boolean;
}

export const BossCooldownDisplay: React.FC<BossCooldownDisplayProps> = ({
  bossCooldowns,
  onBossClick,
  compact = false
}) => {
  if (!bossCooldowns || bossCooldowns.length === 0) {
    return null;
  }

  if (compact) {
    return (
      <div className="flex items-center gap-1 flex-wrap">
        {bossCooldowns.map((boss) => (
          <button
            key={boss.bossId}
            onClick={() => onBossClick?.(boss.bossId, boss.bossName)}
            className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-xs transition-all ${
              boss.hasRecord
                ? 'bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-400'
                : 'bg-emerald-50 text-emerald-600 dark:bg-emerald-900/30 dark:text-emerald-400 hover:bg-emerald-100'
            } ${onBossClick ? 'cursor-pointer' : 'cursor-default'}`}
            title={boss.hasRecord ? `${boss.bossName} - 已完成` : `${boss.bossName} - 可打`}
          >
            <span
              className={`w-2 h-2 rounded-full ${
                boss.hasRecord ? 'bg-gray-400' : 'bg-emerald-500 animate-pulse'
              }`}
            />
            <span>{boss.bossName}</span>
          </button>
        ))}
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2 text-xs text-muted mb-1">
        <span>BOSS进度:</span>
        <span className="text-main font-medium">
          {bossCooldowns.filter(b => b.hasRecord).length}/{bossCooldowns.length}
        </span>
      </div>
      <div className="grid grid-cols-3 sm:grid-cols-6 gap-2">
        {bossCooldowns.map((boss, index) => (
          <button
            key={boss.bossId}
            onClick={() => onBossClick?.(boss.bossId, boss.bossName)}
            className={`relative p-2 rounded-lg border transition-all text-center ${
              boss.hasRecord
                ? 'bg-gray-50 border-gray-200 dark:bg-gray-800/50 dark:border-gray-700'
                : 'bg-emerald-50 border-emerald-200 dark:bg-emerald-900/20 dark:border-emerald-800 hover:border-emerald-300 hover:shadow-sm'
            } ${onBossClick ? 'cursor-pointer' : 'cursor-default'}`}
            title={boss.hasRecord 
              ? `${boss.bossName} - 已完成 (${boss.lastRecordDate ? new Date(boss.lastRecordDate).toLocaleDateString('zh-CN') : ''})` 
              : `${boss.bossName} - 可打`
            }
          >
            <div className="flex flex-col items-center gap-1">
              <div
                className={`w-3 h-3 rounded-full flex items-center justify-center ${
                  boss.hasRecord
                    ? 'bg-gray-400'
                    : 'bg-emerald-500 animate-pulse'
                }`}
              >
                {boss.hasRecord && (
                  <Check className="w-2 h-2 text-white" />
                )}
              </div>
              <span
                className={`text-xs font-medium truncate max-w-full ${
                  boss.hasRecord
                    ? 'text-gray-500 dark:text-gray-400'
                    : 'text-emerald-600 dark:text-emerald-400'
                }`}
              >
                {boss.bossName}
              </span>
              {boss.hasRecord && boss.lastRecordDate && (
                <span className="text-[10px] text-gray-400 dark:text-gray-500 flex items-center gap-0.5">
                  <Clock className="w-2.5 h-2.5" />
                  {new Date(boss.lastRecordDate).toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' })}
                </span>
              )}
            </div>
            <div className="absolute -top-1 -left-1 w-4 h-4 rounded-full bg-surface border border-base text-[10px] text-muted flex items-center justify-center">
              {index + 1}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
};

interface BossCooldownSummaryProps {
  bossCooldowns: BossCooldownInfo[];
}

export const BossCooldownSummary: React.FC<BossCooldownSummaryProps> = ({
  bossCooldowns
}) => {
  if (!bossCooldowns || bossCooldowns.length === 0) {
    return null;
  }

  const completedCount = bossCooldowns.filter(b => b.hasRecord).length;
  const totalCount = bossCooldowns.length;
  const allComplete = completedCount === totalCount;

  return (
    <div className={`flex items-center gap-2 px-2 py-1 rounded-lg text-xs ${
      allComplete
        ? 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400'
        : 'bg-emerald-50 text-emerald-600 dark:bg-emerald-900/30 dark:text-emerald-400'
    }`}>
      <div className="flex items-center gap-1">
        {bossCooldowns.map((boss) => (
          <span
            key={boss.bossId}
            className={`w-2 h-2 rounded-full ${
              boss.hasRecord ? 'bg-gray-400' : 'bg-emerald-500'
            }`}
            title={boss.bossName}
          />
        ))}
      </div>
      <span>{completedCount}/{totalCount}</span>
    </div>
  );
};
