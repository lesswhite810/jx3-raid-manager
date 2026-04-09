// 职业图标配置
// 数据源: src/data/kungfuData.ts (由 vite-plugin-kungfu-data 从 data/kungfu_data.json 自动生成)

import {
  SECT_ICON_IDS as DATA_SECT_ICON_IDS,
  SECT_CONFIG as DATA_SECT_CONFIG,
  MARTIAL_TO_SECT as DATA_MARTIAL_TO_SECT,
  FORCE_NAME_TO_ID,
  KUNGFU_MAP,
  type SectConfig as DataSectConfig,
} from '../src/data/kungfuData';

// 重新导出类型和原始数据，供其他模块使用
export type { Force, Kungfu, SectConfig } from '../src/data/kungfuData';
export { FORCE_NAME_TO_ID, KUNGFU_MAP };

// 职业图标 ID 映射
// 图标来源: https://img.jx3box.com/image/xf/{id}.png
export const SECT_ICON_IDS: Record<string, number> = DATA_SECT_ICON_IDS;

// 职业配置映射
export const SECT_CONFIG: Record<string, DataSectConfig> = DATA_SECT_CONFIG;

// 心法到门派的映射
const MARTIAL_TO_SECT: Record<string, string> = DATA_MARTIAL_TO_SECT;

/**
 * 获取职业配置
 */
export function getSectConfig(sectName: string): DataSectConfig & { name: string } {
  const config = SECT_CONFIG[sectName];
  if (config) {
    return {
      name: sectName,
      ...config,
    };
  }
  // 回退配置
  return {
    name: sectName,
    color: 'bg-blue-100 dark:bg-blue-900/30',
    textColor: 'text-blue-700 dark:text-blue-400',
    borderColor: 'border-blue-200 dark:border-blue-800',
    shortName: sectName.slice(0, 2),
  };
}

/**
 * 获取职业简称
 */
export function getSectShortName(sectName: string): string {
  const config = getSectConfig(sectName);
  return config.shortName;
}

/**
 * 获取职业颜色类
 */
export function getSectColorClasses(sectName: string): {
  bg: string;
  text: string;
  border: string;
} {
  const config = getSectConfig(sectName);
  return {
    bg: config.color,
    text: config.textColor,
    border: config.borderColor,
  };
}

/**
 * 获取职业图标路径（优先本地路径）
 */
export function getSectIconPath(sectName: string): string | undefined {
  const iconId = SECT_ICON_IDS[sectName];
  if (iconId === undefined || iconId === 0) {
    return undefined;
  }
  // 优先使用本地路径
  return `/sect-icons/icon_${iconId}.png`;
}

/**
 * 获取职业图标 CDN URL
 */
export function getSectIconUrl(sectName: string): string | undefined {
  const iconId = SECT_ICON_IDS[sectName];
  if (iconId === undefined || iconId === 0) {
    return undefined;
  }
  return `https://img.jx3box.com/image/xf/${iconId}.png`;
}

/**
 * 根据心法名称获取对应的门派名称
 */
export function getSectByMartial(martial: string): string {
  return MARTIAL_TO_SECT[martial] || '';
}
