import type { Boss, RaidRecord } from '../types';

/**
 * 待确认记录的 BOSS 选择值。
 *
 * 自动扫描的旧记录可能只有名称，手动旧记录则可能只有单个 bossId/bossName，
 * 因此编辑界面需要同时兼容两种表示。
 */
export interface PendingRecordBossSelection {
  bossIds: string[];
  bossNames: string[];
}

interface BossPair {
  id: string;
  name?: string;
}

interface UnmappedBossValues {
  pairs: BossPair[];
  namesWithoutId: string[];
}

/** 从多选字段读取选择；没有多选值时兼容单选旧字段。 */
export const getPendingRecordBossSelection = (
  record: Pick<RaidRecord, 'bossId' | 'bossName' | 'bossIds' | 'bossNames'>,
): PendingRecordBossSelection => {
  const bossIds = record.bossIds ?? [];
  const bossNames = record.bossNames ?? [];
  return {
    // 与 CD 计算保持一致：仅多选数组为空时继续读取单选旧字段。
    // 有值时不分别过滤两个数组，避免名称和 ID 的历史配对发生位移。
    bossIds: bossIds.length > 0 ? [...bossIds] : (record.bossId ? [record.bossId] : []),
    bossNames: bossNames.length > 0 ? [...bossNames] : (record.bossName ? [record.bossName] : []),
  };
};

/** 名称尚未映射为 ID 的自动扫描旧记录，也应在编辑界面显示为已选。 */
export const isPendingRecordBossSelected = (
  selection: PendingRecordBossSelection,
  boss: Boss,
): boolean => selection.bossIds.includes(boss.id) || selection.bossNames.includes(boss.name);

const getUnmappedBossPairs = (
  selection: PendingRecordBossSelection,
  knownIds: Set<string>,
  knownNames: Set<string>,
): UnmappedBossValues => {
  const pairCount = Math.max(selection.bossIds.length, selection.bossNames.length);
  const values: UnmappedBossValues = { pairs: [], namesWithoutId: [] };
  for (let index = 0; index < pairCount; index += 1) {
    const id = selection.bossIds[index];
    const name = selection.bossNames[index];
    // 一端已经是当前配置 BOSS 时，该对值代表同一个选择；在显式编辑后
    // 统一重建为标准 ID/名称，不能留下另一端的旧自定义 ID 继续占用 CD。
    if (knownIds.has(id ?? '') || knownNames.has(name ?? '')) continue;
    if (id) {
      values.pairs.push({ id, name: name || undefined });
    } else if (name) {
      // 没有 ID 的历史名称不能制造空 ID，否则会被 CD 统计当成一个击杀。
      values.namesWithoutId.push(name);
    }
  }
  return values;
};

/**
 * 切换一个配置内 BOSS，并将所有已选的配置内 BOSS 标准化为 ID/名称成对字段。
 * 不属于当前配置的历史/自定义值原样保留，避免一次无关的手动修正抹掉它们。
 */
export const togglePendingRecordBossSelection = (
  selection: PendingRecordBossSelection,
  boss: Boss,
  availableBosses: Boss[],
): PendingRecordBossSelection => {
  const knownIds = new Set(availableBosses.map(item => item.id));
  const knownNames = new Set(availableBosses.map(item => item.name));
  const wasSelected = isPendingRecordBossSelected(selection, boss);

  const selectedBosses = availableBosses.filter(item => {
    if (item.id === boss.id) return !wasSelected;
    return isPendingRecordBossSelected(selection, item);
  });
  const unmapped = getUnmappedBossPairs(selection, knownIds, knownNames);

  return {
    bossIds: [
      ...selectedBosses.map(item => item.id),
      // 只写真实 ID，避免空字符串在 CD 统计中被误计为已击杀的 BOSS。
      ...unmapped.pairs.map(pair => pair.id),
    ],
    bossNames: [
      ...selectedBosses.map(item => item.name),
      // 有 ID 但缺名称时保留展示占位，保证 RaidDetail 的按索引名称查找不偏移。
      ...unmapped.pairs.map(pair => pair.name ?? ''),
      // 无法映射的名称保留在有效 ID 对之后，不为它伪造 bossId。
      ...unmapped.namesWithoutId,
    ],
  };
};

/**
 * 编辑过 BOSS 后的确认字段。
 *
 * 同时写入多选 ID 与名称；单选旧字段同步为第一项，或在取消全部选择时写空，
 * 防止旧 bossId 继续参与 CD 计算。
 */
export const getPendingRecordBossConfirmFields = (
  selection: PendingRecordBossSelection,
): Pick<RaidRecord, 'bossId' | 'bossName' | 'bossIds' | 'bossNames'> => ({
  bossIds: selection.bossIds,
  bossNames: selection.bossNames,
  bossId: selection.bossIds[0] ?? '',
  bossName: selection.bossNames[0] ?? '',
});
