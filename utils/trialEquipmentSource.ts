import trialEquipmentSources from '../data/trialEquipmentSources.json';
import { Season } from '../types';

export type TrialEquipmentType = 'weapon' | 'armor' | 'trinket';

export interface TrialEquipmentSlot {
  type: TrialEquipmentType;
  subType?: number;
  detailType?: number;
}

export interface TrialEquipmentSource {
  keyword: string;
  slots: TrialEquipmentSlot[];
}

export interface TrialEquipmentLike {
  Name?: string;
  SubType?: number;
  DetailType?: number;
}

const sources = trialEquipmentSources as {
  default: TrialEquipmentSource;
  seasons: Record<string, TrialEquipmentSource>;
};

/** 返回当前赛季已确认的试炼装备来源；未知赛季沿用历史“无修”来源。 */
export const getTrialEquipmentSource = (
  season: Pick<Season, 'name'> | null | undefined
): TrialEquipmentSource => {
  const seasonName = season?.name;
  return seasonName && Object.hasOwn(sources.seasons, seasonName)
    ? sources.seasons[seasonName]
    : sources.default;
};

/**
 * 校验远端返回的装备仍符合来源白名单。
 *
 * 魔盒的名称查询会同时返回制作装备等同名条目，因此不能仅依赖请求参数。
 */
export const isTrialEquipmentFromSource = (
  item: TrialEquipmentLike,
  type: TrialEquipmentType,
  source: TrialEquipmentSource
): boolean => {
  if (!item.Name?.includes(source.keyword)) {
    return false;
  }

  return source.slots.some(slot => isTrialEquipmentInSlot(item, type, slot));
};

export const isTrialEquipmentInSlot = (
  item: TrialEquipmentLike,
  type: TrialEquipmentType,
  slot: TrialEquipmentSlot
): boolean => slot.type === type
  && (slot.subType === undefined || Number(item.SubType) === slot.subType)
  && (slot.detailType === undefined || Number(item.DetailType) === slot.detailType);
