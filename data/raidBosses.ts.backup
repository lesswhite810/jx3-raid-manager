import { RaidBossConfig } from '../types';

export const RAID_BOSS_CONFIGS: RaidBossConfig[] = [
  // ========== 丝路风雨版本 ==========
  // 一之窟 - 25人
  {
    raidName: '一之窟',
    difficulty: 'HEROIC',
    playerCount: 25,
    version: '丝路风雨',
    hasBossTracking: true,
    bosses: [
      { id: 'yizhiku_1', name: '骆耀阳', order: 1 },
      { id: 'yizhiku_2', name: '韦柔丝', order: 2 },
      { id: 'yizhiku_3', name: '宋泉', order: 3 },
      { id: 'yizhiku_4', name: '伍靖远', order: 4 },
      { id: 'yizhiku_5', name: '苏什', order: 5 },
      { id: 'yizhiku_6', name: '芭德', order: 6 },
    ]
  },
  {
    raidName: '一之窟',
    difficulty: 'NORMAL',
    playerCount: 25,
    version: '丝路风雨',
    hasBossTracking: true,
    bosses: [
      { id: 'yizhiku_pt_1', name: '骆耀阳', order: 1 },
      { id: 'yizhiku_pt_2', name: '韦柔丝', order: 2 },
      { id: 'yizhiku_pt_3', name: '宋泉', order: 3 },
      { id: 'yizhiku_pt_4', name: '伍靖远', order: 4 },
      { id: 'yizhiku_pt_5', name: '苏什', order: 5 },
      { id: 'yizhiku_pt_6', name: '芭德', order: 6 },
    ]
  },
  // 太极宫 - 25人
  {
    raidName: '太极宫',
    difficulty: 'HEROIC',
    playerCount: 25,
    version: '丝路风雨',
    hasBossTracking: true,
    bosses: [
      { id: 'taijigong_1', name: '刑延恩', order: 1 },
      { id: 'taijigong_2', name: '许灵素', order: 2 },
      { id: 'taijigong_3', name: '侯青', order: 3 },
      { id: 'taijigong_4', name: '李系', order: 4 },
      { id: 'taijigong_5', name: '年勒', order: 5 },
      { id: 'taijigong_6', name: '薛琢玉', order: 6 },
    ]
  },
  {
    raidName: '太极宫',
    difficulty: 'NORMAL',
    playerCount: 25,
    version: '丝路风雨',
    hasBossTracking: true,
    bosses: [
      { id: 'taijigong_pt_1', name: '刑延恩', order: 1 },
      { id: 'taijigong_pt_2', name: '许灵素', order: 2 },
      { id: 'taijigong_pt_3', name: '侯青', order: 3 },
      { id: 'taijigong_pt_4', name: '李系', order: 4 },
      { id: 'taijigong_pt_5', name: '年勒', order: 5 },
      { id: 'taijigong_pt_6', name: '薛琢玉', order: 6 },
    ]
  },
  // 弓月城 - 25人
  {
    raidName: '弓月城',
    difficulty: 'HEROIC',
    playerCount: 25,
    version: '丝路风雨',
    hasBossTracking: true,
    bosses: [
      { id: 'gongyuecheng_1', name: '李无衣', order: 1 },
      { id: 'gongyuecheng_2', name: '慕容雅', order: 2 },
      { id: 'gongyuecheng_3', name: '图南子', order: 3 },
      { id: 'gongyuecheng_4', name: '慕容牙', order: 4 },
      { id: 'gongyuecheng_5', name: '尹雪尘', order: 5 },
    ]
  },
  {
    raidName: '弓月城',
    difficulty: 'NORMAL',
    playerCount: 25,
    version: '丝路风雨',
    hasBossTracking: true,
    bosses: [
      { id: 'gongyuecheng_pt_1', name: '李无衣', order: 1 },
      { id: 'gongyuecheng_pt_2', name: '慕容雅', order: 2 },
      { id: 'gongyuecheng_pt_3', name: '图南子', order: 3 },
      { id: 'gongyuecheng_pt_4', name: '慕容牙', order: 4 },
      { id: 'gongyuecheng_pt_5', name: '尹雪尘', order: 5 },
    ]
  },
];

export const getRaidBossConfig = (
  raidName: string,
  difficulty: 'NORMAL' | 'HEROIC' | 'CHALLENGE',
  playerCount: 10 | 25
): RaidBossConfig | undefined => {
  return RAID_BOSS_CONFIGS.find(
    config => 
      config.raidName === raidName && 
      config.difficulty === difficulty && 
      config.playerCount === playerCount
  );
};

export const hasBossTracking = (
  raidName: string,
  difficulty: 'NORMAL' | 'HEROIC' | 'CHALLENGE',
  playerCount: 10 | 25
): boolean => {
  const config = getRaidBossConfig(raidName, difficulty, playerCount);
  return config?.hasBossTracking ?? false;
};
