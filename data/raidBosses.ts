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

  // ========== 横刀断浪版本 ==========
  // 冷龙峰 - 25人
  {
    raidName: '冷龙峰',
    difficulty: 'HEROIC',
    playerCount: 25,
    version: '横刀断浪',
    hasBossTracking: true,
    bosses: [
      { id: 'lenglongfeng_1', name: '葛木寒', order: 1 },
      { id: 'lenglongfeng_2', name: '雨轻红', order: 2 },
      { id: 'lenglongfeng_3', name: '喜雅', order: 3 },
      { id: 'lenglongfeng_4', name: '无支祈', order: 4 },
      { id: 'lenglongfeng_5', name: '赤厄明', order: 5 },
    ]
  },
  {
    raidName: '冷龙峰',
    difficulty: 'NORMAL',
    playerCount: 25,
    version: '横刀断浪',
    hasBossTracking: true,
    bosses: [
      { id: 'lenglongfeng_pt_1', name: '葛木寒', order: 1 },
      { id: 'lenglongfeng_pt_2', name: '雨轻红', order: 2 },
      { id: 'lenglongfeng_pt_3', name: '喜雅', order: 3 },
      { id: 'lenglongfeng_pt_4', name: '无支祈', order: 4 },
      { id: 'lenglongfeng_pt_5', name: '赤厄明', order: 5 },
    ]
  },
  // 西津渡 - 25人
  {
    raidName: '西津渡',
    difficulty: 'HEROIC',
    playerCount: 25,
    version: '横刀断浪',
    hasBossTracking: true,
    bosses: [
      { id: 'xijindu_1', name: '张景超', order: 1 },
      { id: 'xijindu_2', name: '刘展', order: 2 },
      { id: 'xijindu_3', name: '苏凤楼', order: 3 },
      { id: 'xijindu_4', name: '韩敬青', order: 4 },
      { id: 'xijindu_5', name: '藤原佑野', order: 5 },
      { id: 'xijindu_6', name: '李重茂', order: 6 },
    ]
  },
  {
    raidName: '西津渡',
    difficulty: 'NORMAL',
    playerCount: 25,
    version: '横刀断浪',
    hasBossTracking: true,
    bosses: [
      { id: 'xijindu_pt_1', name: '张景超', order: 1 },
      { id: 'xijindu_pt_2', name: '刘展', order: 2 },
      { id: 'xijindu_pt_3', name: '苏凤楼', order: 3 },
      { id: 'xijindu_pt_4', name: '韩敬青', order: 4 },
      { id: 'xijindu_pt_5', name: '藤原佑野', order: 5 },
      { id: 'xijindu_pt_6', name: '李重茂', order: 6 },
    ]
  },
  // 武狱黑牢 - 25人
  {
    raidName: '武狱黑牢',
    difficulty: 'HEROIC',
    playerCount: 25,
    version: '横刀断浪',
    hasBossTracking: true,
    bosses: [
      { id: 'wuyuheilao_1', name: '时风', order: 1 },
      { id: 'wuyuheilao_2', name: '乐临川', order: 2 },
      { id: 'wuyuheilao_3', name: '牛波', order: 3 },
      { id: 'wuyuheilao_4', name: '和正', order: 4 },
      { id: 'wuyuheilao_5', name: '武云阙', order: 5 },
    ]
  },
  {
    raidName: '武狱黑牢',
    difficulty: 'NORMAL',
    playerCount: 25,
    version: '横刀断浪',
    hasBossTracking: true,
    bosses: [
      { id: 'wuyuheilao_pt_1', name: '时风', order: 1 },
      { id: 'wuyuheilao_pt_2', name: '乐临川', order: 2 },
      { id: 'wuyuheilao_pt_3', name: '牛波', order: 3 },
      { id: 'wuyuheilao_pt_4', name: '和正', order: 4 },
      { id: 'wuyuheilao_pt_5', name: '武云阙', order: 5 },
    ]
  },
  // 九老洞 - 25人
  {
    raidName: '九老洞',
    difficulty: 'HEROIC',
    playerCount: 25,
    version: '横刀断浪',
    hasBossTracking: true,
    bosses: [
      { id: 'jiulaodong_1', name: '魏华', order: 1 },
      { id: 'jiulaodong_2', name: '方有涯', order: 2 },
      { id: 'jiulaodong_3', name: '月行空', order: 3 },
      { id: 'jiulaodong_4', name: '洪瑀', order: 4 },
      { id: 'jiulaodong_5', name: '麒麟', order: 5 },
      { id: 'jiulaodong_6', name: '月泉淮', order: 6 },
    ]
  },
  {
    raidName: '九老洞',
    difficulty: 'NORMAL',
    playerCount: 25,
    version: '横刀断浪',
    hasBossTracking: true,
    bosses: [
      { id: 'jiulaodong_pt_1', name: '魏华', order: 1 },
      { id: 'jiulaodong_pt_2', name: '方有涯', order: 2 },
      { id: 'jiulaodong_pt_3', name: '月行空', order: 3 },
      { id: 'jiulaodong_pt_4', name: '洪瑀', order: 4 },
      { id: 'jiulaodong_pt_5', name: '麒麟', order: 5 },
      { id: 'jiulaodong_pt_6', name: '月泉淮', order: 6 },
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
