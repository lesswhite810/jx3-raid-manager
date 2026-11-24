import React from 'react';
import { Skull } from 'lucide-react';
import type { RaidRecord } from '../types';
import { getRecordBossNames } from '../utils/recordBossDisplay';

interface RecordBossTagsProps {
  record: Pick<RaidRecord, 'bossId' | 'bossName' | 'bossIds' | 'bossNames'>;
}

/**
 * 记录中「击败 BOSS」的标签组。
 *
 * 该副本没有 BOSS 配置、或历史记录里只剩 BOSS ID 时返回 null，不留下孤立的图标。
 * 标签沿用中性底色（非 rose），与设计规范里「red 系仅用于删除与错误」一致。
 */
export const RecordBossTags: React.FC<RecordBossTagsProps> = ({ record }) => {
  const bossNames = getRecordBossNames(record);

  if (bossNames.length === 0) return null;

  return (
    <>
      <span
        className="inline-flex items-center text-muted flex-shrink-0"
        title={`击败 BOSS（${bossNames.length}）`}
      >
        <Skull className="w-3.5 h-3.5" />
      </span>
      {bossNames.map(bossName => (
        <span
          key={bossName}
          className="inline-flex items-center px-1.5 py-0.5 bg-surface text-muted text-xs font-medium rounded border border-base flex-shrink-0"
          title={`击败 BOSS：${bossName}`}
        >
          {bossName}
        </span>
      ))}
    </>
  );
};
