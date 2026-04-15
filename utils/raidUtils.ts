import { Raid } from '../types';

// ID 格式：{playerCount}人{difficulty}{name}，例如 "25人普通会战弓月城"
export const getRaidKey = (raid: Raid): string => {
  return `${raid.playerCount}人${raid.difficulty}${raid.name}`;
};

export const findRaidByKey = (raids: Raid[], name: string, playerCount: 10 | 25, difficulty: '普通' | '英雄' | '挑战'): Raid | undefined => {
  const key = `${playerCount}人${difficulty}${name}`;
  return raids.find(raid => getRaidKey(raid) === key);
};

export const isDuplicateRaid = (raids: Raid[], name: string, playerCount: 10 | 25, difficulty: '普通' | '英雄' | '挑战'): boolean => {
  const key = `${playerCount}人${difficulty}${name}`;
  return raids.some(raid => getRaidKey(raid) === key);
};
