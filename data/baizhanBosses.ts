// 百战BOSS数据配置
import { BaizhanBoss } from '../types';

// 百战BOSS列表 - 根据搜索结果整理
// 来源: 剑网3官方百战系统、西山居游戏中心、剑三魔盒
export const BAIZHAN_BOSSES: BaizhanBoss[] = [
  // 一阶 (3人)
  {
    id: 'tier1_boss1',
    name: '寅',
    tier: 1,
    recommendedSkillLevel: 10000,
    difficulty: 3
  },
  {
    id: 'tier1_boss2',
    name: '雷神',
    tier: 1,
    recommendedSkillLevel: 10000,
    difficulty: 3
  },
  {
    id: 'tier1_boss3',
    name: '子',
    tier: 1,
    recommendedSkillLevel: 10000,
    difficulty: 3
  },

  // 二阶 (3人)
  {
    id: 'tier2_boss1',
    name: '叶冬辰',
    tier: 2,
    recommendedSkillLevel: 15000,
    difficulty: 3
  },
  {
    id: 'tier2_boss2',
    name: '韦柔丝',
    tier: 2,
    recommendedSkillLevel: 15000,
    difficulty: 3
  },
  {
    id: 'tier2_boss3',
    name: '濯尘',
    tier: 2,
    recommendedSkillLevel: 15000,
    difficulty: 3
  },

  // 三阶 (3人)
  {
    id: 'tier3_boss1',
    name: '藤原樱奈',
    tier: 3,
    recommendedSkillLevel: 20000,
    difficulty: 3
  },
  {
    id: 'tier3_boss2',
    name: '方鹤影',
    tier: 3,
    recommendedSkillLevel: 20000,
    difficulty: 3
  },
  {
    id: 'tier3_boss3',
    name: '苏髓',
    tier: 3,
    recommendedSkillLevel: 20000,
    difficulty: 3
  },

  // 四阶 (6人)
  {
    id: 'tier4_boss1',
    name: '阿奴尔',
    tier: 4,
    recommendedSkillLevel: 25000,
    difficulty: 6
  },
  {
    id: 'tier4_boss2',
    name: '源明雅',
    tier: 4,
    recommendedSkillLevel: 25000,
    difficulty: 6
  },
  {
    id: 'tier4_boss3',
    name: '司徒11',
    tier: 4,
    recommendedSkillLevel: 25000,
    difficulty: 6
  },

  // 五阶 (6人)
  {
    id: 'tier5_boss1',
    name: '华鹤炎',
    tier: 5,
    recommendedSkillLevel: 35000,
    difficulty: 6
  },
  {
    id: 'tier5_boss2',
    name: '沈孤鸿',
    tier: 5,
    recommendedSkillLevel: 35000,
    difficulty: 6
  },
  {
    id: 'tier5_boss3',
    name: '慕容追风',
    tier: 5,
    recommendedSkillLevel: 35000,
    difficulty: 6
  },

  // 六阶 (6人)
  {
    id: 'tier6_boss1',
    name: '公孙大娘',
    tier: 6,
    recommendedSkillLevel: 45000,
    difficulty: 6
  },
  {
    id: 'tier6_boss2',
    name: '李忘生',
    tier: 6,
    recommendedSkillLevel: 45000,
    difficulty: 6
  },
  {
    id: 'tier6_boss3',
    name: '莫雨',
    tier: 6,
    recommendedSkillLevel: 45000,
    difficulty: 6
  },

  // 七阶 (10人)
  {
    id: 'tier7_boss1',
    name: '独孤意',
    tier: 7,
    recommendedSkillLevel: 60000,
    difficulty: 10
  },
  {
    id: 'tier7_boss2',
    name: '岳琳',
    tier: 7,
    recommendedSkillLevel: 60000,
    difficulty: 10
  },
  {
    id: 'tier7_boss3',
    name: '南宫伯',
    tier: 7,
    recommendedSkillLevel: 60000,
    difficulty: 10
  },

  // 八阶 (10人)
  {
    id: 'tier8_boss1',
    name: '穆天',
    tier: 8,
    recommendedSkillLevel: 75000,
    difficulty: 10
  },
  {
    id: 'tier8_boss2',
    name: '柳鸾',
    tier: 8,
    recommendedSkillLevel: 75000,
    difficulty: 10
  },
  {
    id: 'tier8_boss3',
    name: '苏云',
    tier: 8,
    recommendedSkillLevel: 75000,
    difficulty: 10
  },

  // 九阶 (10人)
  {
    id: 'tier9_boss1',
    name: '独孤残',
    tier: 9,
    recommendedSkillLevel: 85000,
    difficulty: 10
  },
  {
    id: 'tier9_boss2',
    name: '岳无',
    tier: 9,
    recommendedSkillLevel: 85000,
    difficulty: 10
  },
  {
    id: 'tier9_boss3',
    name: '慕容断',
    tier: 9,
    recommendedSkillLevel: 85000,
    difficulty: 10
  },

  // 十阶 (10人)
  {
    id: 'tier10_boss1',
    name: '剑圣',
    tier: 10,
    recommendedSkillLevel: 100000,
    difficulty: 10
  },
  {
    id: 'tier10_boss2',
    name: '魔主',
    tier: 10,
    recommendedSkillLevel: 100000,
    difficulty: 10
  },
  {
    id: 'tier10_boss3',
    name: '终极BOSS',
    tier: 10,
    recommendedSkillLevel: 100000,
    difficulty: 10
  },
];

// 按难度获取BOSS列表
export const getBossesByDifficulty = (difficulty: 3 | 6 | 10): BaizhanBoss[] => {
  return BAIZHAN_BOSSES.filter(boss => boss.difficulty === difficulty);
};

// 按阶数获取BOSS列表
export const getBossesByTier = (tier: number): BaizhanBoss[] => {
  return BAIZHAN_BOSSES.filter(boss => boss.tier === tier);
};

// 获取所有阶数
export const getAllTiers = (): number[] => {
  const tiers = new Set(BAIZHAN_BOSSES.map(boss => boss.tier));
  return Array.from(tiers).sort((a, b) => a - b);
};

// 根据ID获取BOSS
export const getBossById = (id: string): BaizhanBoss | undefined => {
  return BAIZHAN_BOSSES.find(boss => boss.id === id);
};

// 百战技能颜色列表
export const BAIZHAN_SKILL_COLORS = [
  { name: '红色', color: '#ef4444', value: 'red' },
  { name: '橙色', color: '#f97316', value: 'orange' },
  { name: '黄色', color: '#eab308', value: 'yellow' },
  { name: '绿色', color: '#22c55e', value: 'green' },
  { name: '青色', color: '#06b6d4', value: 'cyan' },
  { name: '蓝色', color: '#3b82f6', value: 'blue' },
  { name: '紫色', color: '#a855f7', value: 'purple' },
  { name: '粉色', color: '#ec4899', value: 'pink' },
];

// 获取颜色配置
export const getSkillColor = (value: string) => {
  return BAIZHAN_SKILL_COLORS.find(c => c.value === value);
};
