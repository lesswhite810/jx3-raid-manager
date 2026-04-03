// 职业图标配置
// 基于剑网三游戏配色

export interface SectConfig {
  name: string;
  color: string;        // 主色调（背景色）
  textColor: string;    // 文字颜色
  borderColor: string;   // 边框颜色
  shortName: string;     // 简称（用于图标显示）
}

// 职业图标 ID 映射
// 图标来源: https://img.jx3box.com/image/xf/{id}.png
export const SECT_ICON_IDS: Record<string, number> = {
  // 七秀
  '冰心诀': 10081,
  '云裳心经': 10080,
  // 万花
  '花间游': 10021,
  '离经易道': 10028,
  // 五毒
  '毒经': 10175,
  '补天诀': 10176,
  // 纯阳
  '紫霞功': 10014,
  '太虚剑意': 10015,
  // 少林
  '易筋经': 10003,
  '洗髓经': 10002,
  // 天策
  '傲血战意': 10026,
  '铁牢律': 10062,
  // 藏剑
  '问水诀': 10144,
  '笑尘诀': 10268,
  // 苍云
  '分山劲': 10390,
  '铁骨衣': 10389,
  // 明教
  '焚影圣诀': 10242,
  '明尊琉璃体': 10243,
  // 田青
  '天罗诡道': 10225,
  '惊羽诀': 10224,
  // 衍天
  '隐龙诀': 10585,
  '太玄经': 10615,
  // 孤锋
  '孤锋诀': 10698,
  '山海心决': 10756,
  // 北傲
  '北傲诀': 10464,
  '凌海诀': 10533,
  // 丐帮
  '丐帮': 10107,
  // 通用
  '通用': 0,
  // 其他门派
  '莫问': 10447,
  '相知': 10448,
  '无方': 10627,
  '灵素': 10626,
  '周天功': 10786,
  '幽罗引': 10821,
};

// 职业配置映射
export const SECT_CONFIG: Record<string, SectConfig> = {
  '冰心诀': {
    name: '冰心诀',
    color: 'bg-pink-100 dark:bg-pink-900/30',
    textColor: 'text-pink-700 dark:text-pink-400',
    borderColor: 'border-pink-200 dark:border-pink-800',
    shortName: 'BX',
  },
  '云裳心经': {
    name: '云裳心经',
    color: 'bg-pink-100 dark:bg-pink-900/30',
    textColor: 'text-pink-700 dark:text-pink-400',
    borderColor: 'border-pink-200 dark:border-pink-800',
    shortName: 'YS',
  },
  '花间游': {
    name: '花间游',
    color: 'bg-purple-100 dark:bg-purple-900/30',
    textColor: 'text-purple-700 dark:text-purple-400',
    borderColor: 'border-purple-200 dark:border-purple-800',
    shortName: 'HJ',
  },
  '离经易道': {
    name: '离经易道',
    color: 'bg-purple-100 dark:bg-purple-900/30',
    textColor: 'text-purple-700 dark:text-purple-400',
    borderColor: 'border-purple-200 dark:border-purple-800',
    shortName: 'LJ',
  },
  '毒经': {
    name: '毒经',
    color: 'bg-green-100 dark:bg-green-900/30',
    textColor: 'text-green-700 dark:text-green-400',
    borderColor: 'border-green-200 dark:border-green-800',
    shortName: 'DJ',
  },
  '补天诀': {
    name: '补天诀',
    color: 'bg-green-100 dark:bg-green-900/30',
    textColor: 'text-green-700 dark:text-green-400',
    borderColor: 'border-green-200 dark:border-green-800',
    shortName: 'BT',
  },
  // 长歌
  '莫问': {
    name: '莫问',
    color: 'bg-indigo-100 dark:bg-indigo-900/30',
    textColor: 'text-indigo-700 dark:text-indigo-400',
    borderColor: 'border-indigo-200 dark:border-indigo-800',
    shortName: 'MW',
  },
  '相知': {
    name: '相知',
    color: 'bg-indigo-100 dark:bg-indigo-900/30',
    textColor: 'text-indigo-700 dark:text-indigo-400',
    borderColor: 'border-indigo-200 dark:border-indigo-800',
    shortName: 'XZ',
  },
  // 药宗
  '无方': {
    name: '无方',
    color: 'bg-teal-100 dark:bg-teal-900/30',
    textColor: 'text-teal-700 dark:text-teal-400',
    borderColor: 'border-teal-200 dark:border-teal-800',
    shortName: 'WF',
  },
  '灵素': {
    name: '灵素',
    color: 'bg-teal-100 dark:bg-teal-900/30',
    textColor: 'text-teal-700 dark:text-teal-400',
    borderColor: 'border-teal-200 dark:border-teal-800',
    shortName: 'LS',
  },
  '傲血战意': {
    name: '傲血战意',
    color: 'bg-red-100 dark:bg-red-900/30',
    textColor: 'text-red-700 dark:text-red-400',
    borderColor: 'border-red-200 dark:border-red-800',
    shortName: 'AX',
  },
  '铁牢律': {
    name: '铁牢律',
    color: 'bg-red-100 dark:bg-red-900/30',
    textColor: 'text-red-700 dark:text-red-400',
    borderColor: 'border-red-200 dark:border-red-800',
    shortName: 'TL',
  },
  '易筋经': {
    name: '易筋经',
    color: 'bg-amber-100 dark:bg-amber-900/30',
    textColor: 'text-amber-700 dark:text-amber-400',
    borderColor: 'border-amber-200 dark:border-amber-800',
    shortName: 'YJ',
  },
  '洗髓经': {
    name: '洗髓经',
    color: 'bg-amber-100 dark:bg-amber-900/30',
    textColor: 'text-amber-700 dark:text-amber-400',
    borderColor: 'border-amber-200 dark:border-amber-800',
    shortName: 'XS',
  },
  '焚影圣诀': {
    name: '焚影圣诀',
    color: 'bg-yellow-100 dark:bg-yellow-900/30',
    textColor: 'text-yellow-700 dark:text-yellow-400',
    borderColor: 'border-yellow-200 dark:border-yellow-800',
    shortName: 'FY',
  },
  '明尊琉璃体': {
    name: '明尊琉璃体',
    color: 'bg-yellow-100 dark:bg-yellow-900/30',
    textColor: 'text-yellow-700 dark:text-yellow-400',
    borderColor: 'border-yellow-200 dark:border-yellow-800',
    shortName: 'MZ',
  },
  '分山劲': {
    name: '分山劲',
    color: 'bg-stone-100 dark:bg-stone-900/30',
    textColor: 'text-stone-700 dark:text-stone-400',
    borderColor: 'border-stone-200 dark:border-stone-800',
    shortName: 'FS',
  },
  '铁骨衣': {
    name: '铁骨衣',
    color: 'bg-stone-100 dark:bg-stone-900/30',
    textColor: 'text-stone-700 dark:text-stone-400',
    borderColor: 'border-stone-200 dark:border-stone-800',
    shortName: 'TG',
  },
  '紫霞功': {
    name: '紫霞功',
    color: 'bg-sky-100 dark:bg-sky-900/30',
    textColor: 'text-sky-700 dark:text-sky-400',
    borderColor: 'border-sky-200 dark:border-sky-800',
    shortName: 'ZX',
  },
  '太虚剑意': {
    name: '太虚剑意',
    color: 'bg-sky-100 dark:bg-sky-900/30',
    textColor: 'text-sky-700 dark:text-sky-400',
    borderColor: 'border-sky-200 dark:border-sky-800',
    shortName: 'TX',
  },
  // 唐门
  '天罗诡道': {
    name: '天罗诡道',
    color: 'bg-lime-100 dark:bg-lime-900/30',
    textColor: 'text-lime-700 dark:text-lime-400',
    borderColor: 'border-lime-200 dark:border-lime-800',
    shortName: 'TL',
  },
  '惊羽诀': {
    name: '惊羽诀',
    color: 'bg-emerald-100 dark:bg-emerald-900/30',
    textColor: 'text-emerald-700 dark:text-emerald-400',
    borderColor: 'border-emerald-200 dark:border-emerald-800',
    shortName: 'JY',
  },
  // 藏剑
  '问水诀': {
    name: '问水诀',
    color: 'bg-yellow-100 dark:bg-yellow-900/30',
    textColor: 'text-yellow-700 dark:text-yellow-400',
    borderColor: 'border-yellow-200 dark:border-yellow-800',
    shortName: 'WS',
  },
  // 丐帮（笑尘诀）
  '笑尘诀': {
    name: '笑尘诀',
    color: 'bg-amber-600 dark:bg-amber-900/50',
    textColor: 'text-amber-50',
    borderColor: 'border-amber-500 dark:border-amber-700',
    shortName: 'GB',
  },
  // 霸刀
  '北傲诀': {
    name: '北傲诀',
    color: 'bg-rose-100 dark:bg-rose-900/30',
    textColor: 'text-rose-700 dark:text-rose-400',
    borderColor: 'border-rose-200 dark:border-rose-800',
    shortName: 'BA',
  },
  // 蓬莱
  '凌海诀': {
    name: '凌海诀',
    color: 'bg-rose-100 dark:bg-rose-900/30',
    textColor: 'text-rose-700 dark:text-rose-400',
    borderColor: 'border-rose-200 dark:border-rose-800',
    shortName: 'LH',
  },
  // 凌雪
  '隐龙诀': {
    name: '隐龙诀',
    color: 'bg-cyan-100 dark:bg-cyan-900/30',
    textColor: 'text-cyan-700 dark:text-cyan-400',
    borderColor: 'border-cyan-200 dark:border-cyan-800',
    shortName: 'YL',
  },
  '太玄经': {
    name: '太玄经',
    color: 'bg-cyan-100 dark:bg-cyan-900/30',
    textColor: 'text-cyan-700 dark:text-cyan-400',
    borderColor: 'border-cyan-200 dark:border-cyan-800',
    shortName: 'TX',
  },
  '孤锋诀': {
    name: '孤锋诀',
    color: 'bg-slate-100 dark:bg-slate-900/30',
    textColor: 'text-slate-700 dark:text-slate-400',
    borderColor: 'border-slate-200 dark:border-slate-800',
    shortName: 'GF',
  },
  '山海心决': {
    name: '山海心决',
    color: 'bg-slate-100 dark:bg-slate-900/30',
    textColor: 'text-slate-700 dark:text-slate-400',
    borderColor: 'border-slate-200 dark:border-slate-800',
    shortName: 'SH',
  },
  // 段氏
  '周天功': {
    name: '周天功',
    color: 'bg-violet-100 dark:bg-violet-900/30',
    textColor: 'text-violet-700 dark:text-violet-400',
    borderColor: 'border-violet-200 dark:border-violet-800',
    shortName: 'ZT',
  },
  // 无相楼
  '幽罗引': {
    name: '幽罗引',
    color: 'bg-violet-100 dark:bg-violet-900/30',
    textColor: 'text-violet-700 dark:text-violet-400',
    borderColor: 'border-violet-200 dark:border-violet-800',
    shortName: 'YL',
  },
  '通用': {
    name: '通用',
    color: 'bg-gray-100 dark:bg-gray-900/30',
    textColor: 'text-gray-700 dark:text-gray-400',
    borderColor: 'border-gray-200 dark:border-gray-800',
    shortName: 'TY',
  }
};

/**
 * 获取职业配置
 */
export function getSectConfig(sectName: string): SectConfig {
  return SECT_CONFIG[sectName] || {
    name: sectName,
    color: 'bg-blue-100 dark:bg-blue-900/30',
    textColor: 'text-blue-700 dark:text-blue-400',
    borderColor: 'border-blue-200 dark:border-blue-800',
    shortName: sectName.slice(0, 2)
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
    border: config.borderColor
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
