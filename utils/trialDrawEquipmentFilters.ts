import { JX3Equip, JX3EquipMagicType } from '../services/jx3BoxApi';

export const TRIAL_DRAW_ATTRIBUTE_FILTERS = [
  { key: '外功', label: '外功' },
  { key: '内功', label: '内功' },
  { key: '会心', label: '会心' },
  { key: '会效', label: '会效' },
  { key: '破防', label: '破防' },
  { key: '加速', label: '加速' },
  { key: '破招', label: '破招' },
  { key: '无双', label: '无双' },
] as const;

const ATTRIBUTE_LABELS: Record<string, string> = {
  atPhysicsAttackPowerBase: '外功',
  atMagicAttackPowerBase: '内功',
  atSolarAttackPowerBase: '内功',
  atLunarAttackPowerBase: '内功',
  atPoisonAttackPowerBase: '内功',
  atNeutralAttackPowerBase: '内功',
  atPhysicsCriticalStrike: '会心',
  atMagicCriticalStrike: '会心',
  atAllTypeCriticalStrike: '会心',
  atPhysicsCriticalDamagePowerBase: '会效',
  atMagicCriticalDamagePowerBase: '会效',
  atAllTypeCriticalDamagePowerBase: '会效',
  atPhysicsOvercome: '破防',
  atPhysicsOvercomeBase: '破防',
  atMagicOvercome: '破防',
  atMagicOvercomeBase: '破防',
  atSolarOvercomeBase: '破防',
  atLunarOvercomeBase: '破防',
  atPoisonOvercomeBase: '破防',
  atNeutralOvercomeBase: '破防',
  atHaste: '加速',
  // 试炼装备目录中的加速绿字使用 Base 变体；保留旧字段兼容旧缓存。
  atHasteBase: '加速',
  atSurplusValueBase: '破招',
  atStrainBase: '无双',
};

export interface TrialDrawEquipmentFilters {
  query: string;
  minLevel: number;
  maxLevel: number;
  slot: string;
  bindType: number | null;
  attributes: ReadonlySet<string>;
}

const getMagicAttributeType = (value: JX3EquipMagicType | undefined): string | null => {
  if (typeof value === 'string') return value;
  if (value && Array.isArray(value.attr)) {
    return value.attr.find((entry): entry is string => typeof entry === 'string') ?? null;
  }
  return null;
};

export const getTrialDrawEquipmentAttributes = (item: JX3Equip): string[] => {
  const attributes = new Set<string>();
  for (let index = 1; index <= 16; index += 1) {
    const magicType = item[`_Magic${index}Type`] as JX3EquipMagicType | undefined;
    const label = ATTRIBUTE_LABELS[getMagicAttributeType(magicType) ?? ''];
    if (label) attributes.add(label);
  }
  // _Attrs 是目录已归类的有效属性。与魔法字段合并，避免一件装备同时
  // 包含外功等魔法字段时丢失目录中的加速、会心等绿字标签。
  if (Array.isArray(item._Attrs)) {
    const fallback: Record<string, string> = {
      Critical: '会心', CriticalDamage: '会效', Overcome: '破防', Haste: '加速',
      Surplus: '破招', Strain: '无双',
    };
    item._Attrs.forEach(attribute => {
      const label = fallback[attribute];
      if (label) attributes.add(label);
    });
  }
  // 手工选择器兼容 JX3Box 的另一种响应形态。只合并明确标记为绿色且
  // 在映射表中有证据的属性，未知字段不推断为任何筛选标签。
  if (Array.isArray(item.attributes)) {
    item.attributes.forEach(attribute => {
      if (attribute.color !== 'green' || !attribute.type) return;
      const label = ATTRIBUTE_LABELS[attribute.type];
      if (label) attributes.add(label);
    });
  }
  return Array.from(attributes);
};

export const getTrialDrawEquipmentSlot = (item: JX3Equip): string => {
  if (item.TypeLabel?.trim()) return item.TypeLabel.trim();
  if (Number(item.DetailType) === 6) return '暗器';
  switch (Number(item.SubType)) {
    case 1: return '武器';
    case 2: return '上衣';
    case 3: return '帽子';
    case 4: return '项链';
    case 5: return '戒指';
    case 6: return '腰带';
    case 7: return '腰坠';
    case 8: return '下装';
    case 9: return '鞋子';
    case 10: return '护腕';
    default: return '其他';
  }
};

export const getTrialDrawEquipmentBindLabel = (bindType: number | undefined): string => {
  switch (Number(bindType)) {
    case 1: return '不绑定';
    case 2: return '装备后绑定';
    case 3: return '拾取后绑定';
    default: return '绑定未知';
  }
};

export const getTrialDrawEquipmentLevelBounds = (items: JX3Equip[]): { min: number; max: number } | null => {
  const levels = items.map(item => Number(item.Level)).filter(level => Number.isFinite(level) && level > 0);
  if (levels.length === 0) return null;
  return { min: Math.min(...levels), max: Math.max(...levels) };
};

export const filterTrialDrawEquipments = (
  items: JX3Equip[],
  filters: TrialDrawEquipmentFilters,
): JX3Equip[] => {
  const term = filters.query.trim().toLocaleLowerCase('zh-CN');
  return items.filter(item => {
    const slot = getTrialDrawEquipmentSlot(item);
    const attributes = getTrialDrawEquipmentAttributes(item);
    const bindLabel = getTrialDrawEquipmentBindLabel(item.BindType);
    const level = Number(item.Level);
    if (!Number.isFinite(level) || level < filters.minLevel || level > filters.maxLevel) return false;
    if (filters.slot !== '全部' && slot !== filters.slot) return false;
    if (filters.bindType !== null && Number(item.BindType) !== filters.bindType) return false;
    if (!Array.from(filters.attributes).every(attribute => attributes.includes(attribute))) return false;
    if (!term) return true;
    const searchableText = [item.Name, item.ID, slot, attributes.join(' '), bindLabel, level]
      .filter(value => value !== undefined && value !== null)
      .join(' ')
      .toLocaleLowerCase('zh-CN');
    return term.split(/\s+/).every(token => searchableText.includes(token));
  });
};
