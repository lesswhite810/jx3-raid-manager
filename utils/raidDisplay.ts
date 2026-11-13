import { Raid } from '../types';

/** 难度显示文案 */
export const DIFFICULTY_LABELS: Record<string, string> = {
  '普通': '普通',
  '英雄': '英雄',
  '挑战': '挑战'
};

/** 难度标签配色（历史模式的收藏卡片沿用） */
export const DIFFICULTY_COLORS: Record<string, string> = {
  '普通': 'bg-ds-success-soft text-ds-success-strong border-ds-success-soft dark:bg-ds-success-soft/30 dark:text-ds-success-strong dark:border-ds-success-soft',
  '英雄': 'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:border-blue-800',
  '挑战': 'bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-900/30 dark:text-rose-400 dark:border-rose-800'
};

/** 同一副本名下的多难度归并结果 */
export interface MergedRaid {
  name: string;
  version: string;
  raids: Raid[];
  isActive: boolean;
  disabled: boolean;
  difficultyLabels: { [key: string]: string };
}

/** 预置副本（不能删除，只能禁用） */
export const isStaticRaid = (raid: Raid): boolean => !!raid.static;
