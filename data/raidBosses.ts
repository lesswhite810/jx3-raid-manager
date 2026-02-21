import { Boss, Raid } from '../types';

// 默认 BOSS 配置（按副本名称索引，所有难度共享）
const DEFAULT_BOSSES: Record<string, Boss[]> = {
  '弓月城': [
    { id: 'gongyuecheng_1', name: '巴图仁钦', order: 1 },
    { id: 'gongyuecheng_2', name: '竭勒', order: 2 },
    { id: 'gongyuecheng_3', name: '图南子', order: 3 },
    { id: 'gongyuecheng_4', name: '叶葵', order: 4 },
    { id: 'gongyuecheng_5', name: '尹雪尘', order: 5 },
  ],
  '缚罪之渊': [
    { id: 'fuzuizhiyuan_1', name: '阿里曼幻身', order: 1 },
    { id: 'fuzuizhiyuan_2', name: '阿萨辛', order: 2 },
  ],
};

// 获取副本的默认 BOSS 列表
export const getDefaultBosses = (raidName: string): Boss[] | undefined => {
  return DEFAULT_BOSSES[raidName];
};

// 为副本自动注入默认 BOSS（如果副本没有 bosses 字段）
export const injectDefaultBosses = (raid: Raid): Raid => {
  if (raid.bosses && raid.bosses.length > 0) return raid;
  const defaults = DEFAULT_BOSSES[raid.name];
  if (defaults) {
    return { ...raid, bosses: defaults };
  }
  return raid;
};

// 批量注入
export const injectDefaultBossesForRaids = (raids: Raid[]): Raid[] => {
  return raids.map(injectDefaultBosses);
};

// 判断副本是否启用 BOSS CD 追踪
export const hasBossTracking = (raid: Raid): boolean => {
  return (raid.bosses && raid.bosses.length > 0) || !!DEFAULT_BOSSES[raid.name];
};
