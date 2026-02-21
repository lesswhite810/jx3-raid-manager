import { Raid } from '../types';

// ID 格式：{playerCount}人{difficulty}{name}，例如 "25人普通弓月城"
export const getRaidKey = (raid: Raid): string => {
  return `${raid.playerCount}人${raid.difficulty}${raid.name}`;
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

export const findRaidByKey = (raids: Raid[], name: string, playerCount: 10 | 25, difficulty: '普通' | '英雄' | '挑战'): Raid | undefined => {
  const key = `${playerCount}人${difficulty}${name}`;
  return raids.find(raid => getRaidKey(raid) === key);
};

export const isDuplicateRaid = (raids: Raid[], name: string, playerCount: 10 | 25, difficulty: '普通' | '英雄' | '挑战'): boolean => {
  const key = `${playerCount}人${difficulty}${name}`;
  return raids.some(raid => getRaidKey(raid) === key);
};