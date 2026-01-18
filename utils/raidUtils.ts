import { Raid } from '../types';
import { STATIC_RAIDS, StaticRaid } from '../data/staticRaids';

export const getRaidKey = (raid: Raid): string => {
  return `${raid.name}-${raid.playerCount}-${raid.difficulty}`;
};

export const convertStaticRaidToRaid = (staticRaid: StaticRaid): Raid => {
  const description = staticRaid.description || '';
  let difficulty: 'NORMAL' | 'HEROIC' | 'CHALLENGE' = 'NORMAL';
  let playerCount: 10 | 25 = 25;

  if (description.includes('英雄')) {
    difficulty = 'HEROIC';
  } else if (description.includes('挑战')) {
    difficulty = 'CHALLENGE';
  }

  if (description.includes('10人')) {
    playerCount = 10;
  } else if (description.includes('25人')) {
    playerCount = 25;
  }

  return {
    name: staticRaid.name,
    difficulty,
    playerCount,
    version: staticRaid.version,
    notes: staticRaid.description,
    isActive: staticRaid.isActive ?? true
  };
};

export const getRaidCacheKey = (): string => {
  return 'jx3_raids';
};

export const saveRaidCache = (raids: Raid[]): void => {
  try {
    localStorage.setItem(getRaidCacheKey(), JSON.stringify(raids));
  } catch (error) {
    console.error('保存副本缓存失败:', error);
  }
};

export const loadRaidCache = (): Raid[] => {
  try {
    const cached = localStorage.getItem(getRaidCacheKey());
    if (cached) {
      const parsed = JSON.parse(cached);
      return Array.isArray(parsed) ? parsed : [];
    }
  } catch (error) {
    console.error('加载副本缓存失败:', error);
  }
  return [];
};

export const clearRaidCache = (): void => {
  try {
    localStorage.removeItem(getRaidCacheKey());
    console.log('副本缓存已清除');
  } catch (error) {
    console.error('清除副本缓存失败:', error);
  }
};

export const mergeRaids = (cachedRaids: Raid[]): Raid[] => {
  const staticRaids = STATIC_RAIDS.map(convertStaticRaidToRaid);
  
  const raidMap = new Map<string, Raid>();
  
  // 首先处理静态副本，添加标记
  staticRaids.forEach(raid => {
    const key = getRaidKey(raid);
    raid.static = true;
    raidMap.set(key, raid);
  });
  
  // 合并缓存的副本数据，保留用户的自定义状态
  cachedRaids.forEach(raid => {
    const key = getRaidKey(raid);
    const existingRaid = raidMap.get(key);
    
    if (existingRaid) {
      // 如果缓存的副本与静态副本同名（相同 key），保留用户的 isActive 状态
      // 同时保留其他可能的自定义属性
      raidMap.set(key, {
        ...existingRaid,
        isActive: raid.isActive,
        notes: raid.notes || existingRaid.notes,
        static: true // 保持静态标记
      });
    } else {
      // 新的自定义副本
      raidMap.set(key, raid);
    }
  });
  
  return Array.from(raidMap.values());
};

export const getStaticRaids = (): Raid[] => {
  return STATIC_RAIDS.map(convertStaticRaidToRaid);
};

export const findRaidByKey = (raids: Raid[], name: string, playerCount: 10 | 25, difficulty: 'NORMAL' | 'HEROIC' | 'CHALLENGE'): Raid | undefined => {
  const key = `${name}-${playerCount}-${difficulty}`;
  return raids.find(raid => getRaidKey(raid) === key);
};

export const isDuplicateRaid = (raids: Raid[], name: string, playerCount: 10 | 25, difficulty: 'NORMAL' | 'HEROIC' | 'CHALLENGE'): boolean => {
  const key = `${name}-${playerCount}-${difficulty}`;
  return raids.some(raid => getRaidKey(raid) === key);
};