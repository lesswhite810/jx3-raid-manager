import type { RaidRecord } from '../types';
import { getPendingRecordBossSelection } from './pendingRecordBossSelection';

/** 记录里可用于展示的 BOSS 字段：多选优先，兼容单选旧字段。 */
export type RecordBossFields = Pick<RaidRecord, 'bossId' | 'bossName' | 'bossIds' | 'bossNames'>;

/**
 * 取出记录中击败的 BOSS 名称。
 *
 * 自动扫描记录同时写入 bossIds/bossNames；手动旧记录可能只有 bossId/bossName，
 * 因此复用待确认面板的兼容逻辑，保证两处显示口径一致。历史数据里「有 ID 无名称」
 * 会留下空字符串，展示前过滤掉，避免出现空白标签。
 */
export const getRecordBossNames = (record: RecordBossFields): string[] => {
  const selection = getPendingRecordBossSelection(record);
  return selection.bossNames
    .map(name => name.trim())
    .filter(name => name.length > 0);
};
