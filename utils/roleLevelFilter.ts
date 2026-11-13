import type { GameVersion } from '../types';

/**
 * 判断角色在「隐藏非满级角色」开关下是否应显示。
 *
 * 规则（与后端扫描/入库无关，仅用于显示层过滤）：
 * - 开关关闭 → 显示；
 * - 当前版本未配置等级上限（levelCap 为 null/undefined）→ 不过滤，显示；
 * - 角色等级未知（level 为 null/undefined，老数据未扫描）→ 默认显示，
 *   等扫描回填后自动生效；
 * - 否则仅当角色等级等于当前版本等级上限时显示。
 *
 * @param role 含 level 字段的角色（level 可缺失或为 null）
 * @param levelCap 当前版本的等级上限（null/undefined 表示未配置）
 * @param hideNonMaxLevelEnabled 「隐藏非满级角色」开关是否开启
 */
export const shouldShowRoleByLevel = (
  role: { level?: number | null },
  levelCap: number | null | undefined,
  hideNonMaxLevelEnabled: boolean
): boolean => {
  if (!hideNonMaxLevelEnabled) {
    return true;
  }
  if (levelCap === null || levelCap === undefined) {
    return true;
  }
  if (role.level === null || role.level === undefined) {
    return true;
  }
  return role.level === levelCap;
};

/**
 * 从版本列表中取「当前版本」的等级上限。
 *
 * 当前版本定义为 sortOrder 最大的版本；若该版本未配置 levelCap（null/undefined）
 * 或列表为空，返回 null（表示不做等级过滤）。
 */
export const getCurrentVersionLevelCap = (versions: GameVersion[]): number | null => {
  if (!Array.isArray(versions) || versions.length === 0) {
    return null;
  }
  const latest = versions.reduce((acc, cur) => (cur.sortOrder > acc.sortOrder ? cur : acc));
  return latest.levelCap ?? null;
};
