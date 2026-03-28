// 职业图标配置
// 基于剑网三游戏配色

export interface SectConfig {
  name: string;
  color: string;        // 主色调（背景色）
  textColor: string;    // 文字颜色
  borderColor: string;   // 边框颜色
  shortName: string;     // 简称（用于图标显示）
  iconPath?: string;     // 图标路径（可选）
}

// 职业图标 ID 映射
export const SECT_ICON_IDS: Record<string, number> = {
  // 纯阳
  '紫霞功': 11001,
  '太虚剑意': 11001,
  // 七秀
  '冰心诀': 11002,
  '云裳心经': 11002,
  // 万花
  '花间游': 11003,
  '离经易道': 11003,
  // 少林
  '易筋经': 11004,
  '洗髓经': 11004,
  // 天策
  '傲血战意': 11005,
  '铁牢律': 11005,
  // 藏剑
  '问水诀': 11006,
  '笑尘诀': 11006,
  // 五毒
  '毒经': 11008,
  '补天诀': 11008,
  // 明教
  '焚影圣诀': 11009,
  '明尊琉璃体': 11009,
  // 苍云
  '分山劲': 11010,
  '铁骨衣': 11010,
  // 田青
  '天罗诡道': 11011,
  '惊羽诀': 11011,
  // 衍天
  '隐龙诀': 11012,
  '太玄经': 11012,
  // 孤锋
  '孤锋诀': 11013,
  '山海心决': 11013,
  // 北傲
  '北傲诀': 11014,
  '凌海诀': 11014,
  // 通用
  '通用': 11001,
  // 其他
  '莫问': 11001,
  '相知': 11001,
  '无方': 11001,
  '灵素': 11001,
  '周天功': 11001,
  '幽罗引': 11001,
};

// 职业配置映射
export const SECT_CONFIG: Record<string, SectConfig> = {
  '冰心诀': {
    name: '冰心诀',
    color: 'bg-pink-100 dark:bg-pink-900/30',
    textColor: 'text-pink-700 dark:text-pink-400',
    borderColor: 'border-pink-200 dark:border-pink-800',
    shortName: 'BX',
    iconPath: '/sect-icons/icon_11002.png'
  },
  '云裳心经': {
    name: '云裳心经',
    color: 'bg-pink-100 dark:bg-pink-900/30',
    textColor: 'text-pink-700 dark:text-pink-400',
    borderColor: 'border-pink-200 dark:border-pink-800',
    shortName: 'YS',
    iconPath: '/sect-icons/icon_11002.png'
  },
  '花间游': {
    name: '花间游',
    color: 'bg-purple-100 dark:bg-purple-900/30',
    textColor: 'text-purple-700 dark:text-purple-400',
    borderColor: 'border-purple-200 dark:border-purple-800',
    shortName: 'HJ',
    iconPath: '/sect-icons/icon_11003.png'
  },
  '离经易道': {
    name: '离经易道',
    color: 'bg-purple-100 dark:bg-purple-900/30',
    textColor: 'text-purple-700 dark:text-purple-400',
    borderColor: 'border-purple-200 dark:border-purple-800',
    shortName: 'LJ',
    iconPath: '/sect-icons/icon_11003.png'
  },
  '毒经': {
    name: '毒经',
    color: 'bg-green-100 dark:bg-green-900/30',
    textColor: 'text-green-700 dark:text-green-400',
    borderColor: 'border-green-200 dark:border-green-800',
    shortName: 'DJ',
    iconPath: '/sect-icons/icon_11008.png'
  },
  '补天诀': {
    name: '补天诀',
    color: 'bg-green-100 dark:bg-green-900/30',
    textColor: 'text-green-700 dark:text-green-400',
    borderColor: 'border-green-200 dark:border-green-800',
    shortName: 'BT',
    iconPath: '/sect-icons/icon_11008.png'
  },
  '莫问': {
    name: '莫问',
    color: 'bg-indigo-100 dark:bg-indigo-900/30',
    textColor: 'text-indigo-700 dark:text-indigo-400',
    borderColor: 'border-indigo-200 dark:border-indigo-800',
    shortName: 'MW',
    iconPath: '/sect-icons/icon_11001.png'
  },
  '相知': {
    name: '相知',
    color: 'bg-indigo-100 dark:bg-indigo-900/30',
    textColor: 'text-indigo-700 dark:text-indigo-400',
    borderColor: 'border-indigo-200 dark:border-indigo-800',
    shortName: 'XZ',
    iconPath: '/sect-icons/icon_11001.png'
  },
  '无方': {
    name: '无方',
    color: 'bg-teal-100 dark:bg-teal-900/30',
    textColor: 'text-teal-700 dark:text-teal-400',
    borderColor: 'border-teal-200 dark:border-teal-800',
    shortName: 'WF',
    iconPath: '/sect-icons/icon_11001.png'
  },
  '灵素': {
    name: '灵素',
    color: 'bg-teal-100 dark:bg-teal-900/30',
    textColor: 'text-teal-700 dark:text-teal-400',
    borderColor: 'border-teal-200 dark:border-teal-800',
    shortName: 'LS',
    iconPath: '/sect-icons/icon_11001.png'
  },
  '傲血战意': {
    name: '傲血战意',
    color: 'bg-red-100 dark:bg-red-900/30',
    textColor: 'text-red-700 dark:text-red-400',
    borderColor: 'border-red-200 dark:border-red-800',
    shortName: 'AX',
    iconPath: '/sect-icons/icon_11005.png'
  },
  '铁牢律': {
    name: '铁牢律',
    color: 'bg-red-100 dark:bg-red-900/30',
    textColor: 'text-red-700 dark:text-red-400',
    borderColor: 'border-red-200 dark:border-red-800',
    shortName: 'TL',
    iconPath: '/sect-icons/icon_11005.png'
  },
  '易筋经': {
    name: '易筋经',
    color: 'bg-amber-100 dark:bg-amber-900/30',
    textColor: 'text-amber-700 dark:text-amber-400',
    borderColor: 'border-amber-200 dark:border-amber-800',
    shortName: 'YJ',
    iconPath: '/sect-icons/icon_11004.png'
  },
  '洗髓经': {
    name: '洗髓经',
    color: 'bg-amber-100 dark:bg-amber-900/30',
    textColor: 'text-amber-700 dark:text-amber-400',
    borderColor: 'border-amber-200 dark:border-amber-800',
    shortName: 'XS',
    iconPath: '/sect-icons/icon_11004.png'
  },
  '焚影圣诀': {
    name: '焚影圣诀',
    color: 'bg-yellow-100 dark:bg-yellow-900/30',
    textColor: 'text-yellow-700 dark:text-yellow-400',
    borderColor: 'border-yellow-200 dark:border-yellow-800',
    shortName: 'FY',
    iconPath: '/sect-icons/icon_11009.png'
  },
  '明尊琉璃体': {
    name: '明尊琉璃体',
    color: 'bg-yellow-100 dark:bg-yellow-900/30',
    textColor: 'text-yellow-700 dark:text-yellow-400',
    borderColor: 'border-yellow-200 dark:border-yellow-800',
    shortName: 'MZ',
    iconPath: '/sect-icons/icon_11009.png'
  },
  '分山劲': {
    name: '分山劲',
    color: 'bg-stone-100 dark:bg-stone-900/30',
    textColor: 'text-stone-700 dark:text-stone-400',
    borderColor: 'border-stone-200 dark:border-stone-800',
    shortName: 'FS',
    iconPath: '/sect-icons/icon_11010.png'
  },
  '铁骨衣': {
    name: '铁骨衣',
    color: 'bg-stone-100 dark:bg-stone-900/30',
    textColor: 'text-stone-700 dark:text-stone-400',
    borderColor: 'border-stone-200 dark:border-stone-800',
    shortName: 'TG',
    iconPath: '/sect-icons/icon_11010.png'
  },
  '紫霞功': {
    name: '紫霞功',
    color: 'bg-sky-100 dark:bg-sky-900/30',
    textColor: 'text-sky-700 dark:text-sky-400',
    borderColor: 'border-sky-200 dark:border-sky-800',
    shortName: 'ZX',
    iconPath: '/sect-icons/icon_11001.png'
  },
  '太虚剑意': {
    name: '太虚剑意',
    color: 'bg-sky-100 dark:bg-sky-900/30',
    textColor: 'text-sky-700 dark:text-sky-400',
    borderColor: 'border-sky-200 dark:border-sky-800',
    shortName: 'TX',
    iconPath: '/sect-icons/icon_11001.png'
  },
  '天罗诡道': {
    name: '天罗诡道',
    color: 'bg-lime-100 dark:bg-lime-900/30',
    textColor: 'text-lime-700 dark:text-lime-400',
    borderColor: 'border-lime-200 dark:border-lime-800',
    shortName: 'TL',
    iconPath: '/sect-icons/icon_11011.png'
  },
  '惊羽诀': {
    name: '惊羽诀',
    color: 'bg-emerald-100 dark:bg-emerald-900/30',
    textColor: 'text-emerald-700 dark:text-emerald-400',
    borderColor: 'border-emerald-200 dark:border-emerald-800',
    shortName: 'JY',
    iconPath: '/sect-icons/icon_11011.png'
  },
  '问水诀': {
    name: '问水诀',
    color: 'bg-yellow-100 dark:bg-yellow-900/30',
    textColor: 'text-yellow-700 dark:text-yellow-400',
    borderColor: 'border-yellow-200 dark:border-yellow-800',
    shortName: 'WS',
    iconPath: '/sect-icons/icon_11006.png'
  },
  '笑尘诀': {
    name: '笑尘诀',
    color: 'bg-yellow-100 dark:bg-yellow-900/30',
    textColor: 'text-yellow-700 dark:text-yellow-400',
    borderColor: 'border-yellow-200 dark:border-yellow-800',
    shortName: 'XC',
    iconPath: '/sect-icons/icon_11006.png'
  },
  '北傲诀': {
    name: '北傲诀',
    color: 'bg-rose-100 dark:bg-rose-900/30',
    textColor: 'text-rose-700 dark:text-rose-400',
    borderColor: 'border-rose-200 dark:border-rose-800',
    shortName: 'BA',
    iconPath: '/sect-icons/icon_11014.png'
  },
  '凌海诀': {
    name: '凌海诀',
    color: 'bg-rose-100 dark:bg-rose-900/30',
    textColor: 'text-rose-700 dark:text-rose-400',
    borderColor: 'border-rose-200 dark:border-rose-800',
    shortName: 'LH',
    iconPath: '/sect-icons/icon_11014.png'
  },
  '隐龙诀': {
    name: '隐龙诀',
    color: 'bg-cyan-100 dark:bg-cyan-900/30',
    textColor: 'text-cyan-700 dark:text-cyan-400',
    borderColor: 'border-cyan-200 dark:border-cyan-800',
    shortName: 'YL',
    iconPath: '/sect-icons/icon_11012.png'
  },
  '太玄经': {
    name: '太玄经',
    color: 'bg-cyan-100 dark:bg-cyan-900/30',
    textColor: 'text-cyan-700 dark:text-cyan-400',
    borderColor: 'border-cyan-200 dark:border-cyan-800',
    shortName: 'TX',
    iconPath: '/sect-icons/icon_11012.png'
  },
  '孤锋诀': {
    name: '孤锋诀',
    color: 'bg-slate-100 dark:bg-slate-900/30',
    textColor: 'text-slate-700 dark:text-slate-400',
    borderColor: 'border-slate-200 dark:border-slate-800',
    shortName: 'GF',
    iconPath: '/sect-icons/icon_11013.png'
  },
  '山海心决': {
    name: '山海心决',
    color: 'bg-slate-100 dark:bg-slate-900/30',
    textColor: 'text-slate-700 dark:text-slate-400',
    borderColor: 'border-slate-200 dark:border-slate-800',
    shortName: 'SH',
    iconPath: '/sect-icons/icon_11013.png'
  },
  '周天功': {
    name: '周天功',
    color: 'bg-violet-100 dark:bg-violet-900/30',
    textColor: 'text-violet-700 dark:text-violet-400',
    borderColor: 'border-violet-200 dark:border-violet-800',
    shortName: 'ZT',
    iconPath: '/sect-icons/icon_11001.png'
  },
  '幽罗引': {
    name: '幽罗引',
    color: 'bg-violet-100 dark:bg-violet-900/30',
    textColor: 'text-violet-700 dark:text-violet-400',
    borderColor: 'border-violet-200 dark:border-violet-800',
    shortName: 'YL',
    iconPath: '/sect-icons/icon_11001.png'
  },
  '通用': {
    name: '通用',
    color: 'bg-gray-100 dark:bg-gray-900/30',
    textColor: 'text-gray-700 dark:text-gray-400',
    borderColor: 'border-gray-200 dark:border-gray-800',
    shortName: 'TY',
    iconPath: '/sect-icons/icon_11001.png'
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
 * 获取职业图标路径
 */
export function getSectIconPath(sectName: string): string | undefined {
  const config = getSectConfig(sectName);
  return config.iconPath;
}
