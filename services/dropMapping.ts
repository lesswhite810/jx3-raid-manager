/**
 * 特殊掉落映射服务
 * 用于识别聊天记录中的特殊物品并映射到对应的标志字段
 */

import { SpecialItem, DropFlags } from '../types';

/**
 * 特殊掉落类型映射配置
 */
export const SPECIAL_DROP_MAPPINGS: Record<string, {
  field: keyof DropFlags;
  label: string;
  keywords: string[];
}> = {
  xuanjing: {
    field: 'hasXuanjing',
    label: '玄晶',
    keywords: ['玄晶', '化玉', '绮丽', '月华', '天乙', '星源'],
  },
  maju: {
    field: 'hasMaJu',
    label: '马具',
    keywords: ['马鞍', '马鞭', '缰绳', '护甲', '马具'],
  },
  pet: {
    field: 'hasPet',
    label: '宠物',
    keywords: ['宠物', '跟宠'],
  },
  pendant: {
    field: 'hasPendant',
    label: '挂件',
    keywords: ['挂件', '腰挂', '背挂', '面挂'],
  },
  mount: {
    field: 'hasMount',
    label: '坐骑',
    keywords: ['坐骑', '马匹'],
  },
  appearance: {
    field: 'hasAppearance',
    label: '外观',
    keywords: ['外观', '成衣', '校服', '盒子'],
  },
  title: {
    field: 'hasTitle',
    label: '称号',
    keywords: ['称号'],
  },
  secretBook: {
    field: 'hasSecretBook',
    label: '秘籍',
    keywords: ['秘籍', '残页', '断篇'],
  },
};

/**
 * 默认的掉落标志（全为 false）
 */
export const DEFAULT_DROP_FLAGS: DropFlags = {
  hasXuanjing: false,
  hasMaJu: false,
  hasPet: false,
  hasPendant: false,
  hasMount: false,
  hasAppearance: false,
  hasTitle: false,
  hasSecretBook: false,
};

/**
 * 根据物品名称识别特殊掉落类型
 * @param itemName 物品名称
 * @returns 掉落类型 key，未识别返回 null
 */
export function identifySpecialDrop(itemName: string): string | null {
  for (const [type, config] of Object.entries(SPECIAL_DROP_MAPPINGS)) {
    if (config.keywords.some(keyword => itemName.includes(keyword))) {
      return type;
    }
  }
  return null;
}

/**
 * 从特殊物品列表生成掉落标志
 * @param items 特殊物品列表
 * @returns 掉落标志对象
 */
export function generateDropFlags(items: SpecialItem[]): DropFlags {
  const flags = { ...DEFAULT_DROP_FLAGS };

  for (const item of items) {
    const dropType = identifySpecialDrop(item.name);
    if (dropType && SPECIAL_DROP_MAPPINGS[dropType]) {
      flags[SPECIAL_DROP_MAPPINGS[dropType].field] = true;
    }
  }

  return flags;
}

/**
 * 获取物品的掉落类型标签列表
 * @param items 特殊物品列表
 * @returns 标签列表 [{ type, label }]
 */
export function getDropTypeTags(items: SpecialItem[]): Array<{ type: string; label: string }> {
  const types = new Map<string, { type: string; label: string }>();

  for (const item of items) {
    const dropType = identifySpecialDrop(item.name);
    if (dropType && SPECIAL_DROP_MAPPINGS[dropType] && !types.has(dropType)) {
      types.set(dropType, {
        type: dropType,
        label: SPECIAL_DROP_MAPPINGS[dropType].label,
      });
    }
  }

  return Array.from(types.values());
}

/**
 * 获取掉落标签的图标名称（用于 Lucide 图标）
 */
export function getDropIcon(type: string): string {
  const iconMap: Record<string, string> = {
    xuanjing: 'Sparkles',
    maju: 'Anchor',
    pet: 'Ghost',
    pendant: 'Package',
    mount: 'Flag',
    appearance: 'Shirt',
    title: 'Crown',
    secretBook: 'BookOpen',
  };
  return iconMap[type] || 'Star';
}

/**
 * 获取掉落标签的颜色类名
 */
export function getDropColorClass(type: string): string {
  const colorMap: Record<string, string> = {
    xuanjing: 'text-amber-500',
    maju: 'text-blue-500',
    pet: 'text-purple-500',
    pendant: 'text-orange-500',
    mount: 'text-green-500',
    appearance: 'text-pink-500',
    title: 'text-yellow-600',
    secretBook: 'text-cyan-600',
  };
  return colorMap[type] || 'text-gray-500';
}
