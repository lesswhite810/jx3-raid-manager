import { Raid, BossCooldownInfo, BossRecord } from '../types';
import { getTenPersonCycle, getLastMonday, getNextMonday } from './cooldownManager';

export const calculateBossCooldowns = (
  raid: Raid,
  bossRecords: BossRecord[],
  roleId: string,
  now: Date = new Date(),
  useFullWeek: boolean = false
): BossCooldownInfo[] => {
  const bosses = raid.bosses;

  if (!bosses || bosses.length === 0) {
    return [];
  }

  const isTenPerson = raid.playerCount === 10;
  let windowStart: Date;
  let windowEnd: Date;

  if (isTenPerson && !useFullWeek) {
    // CD 判断：10人本使用半周周期（周一~周五 / 周五~下周一）
    const cycle = getTenPersonCycle(now);
    windowStart = cycle.start;
    windowEnd = cycle.end;
  } else {
    // 显示用或25人本：使用整周窗口（周一~下周一）
    windowStart = getLastMonday(now);
    windowEnd = getNextMonday(now);
  }

  const roleBossRecords = bossRecords.filter(record => {
    if (record.roleId !== roleId) return false;
    if (record.status === 'rejected') return false; // 排除已拒绝记录
    const rDate = new Date(record.date);
    return rDate >= windowStart && rDate < windowEnd;
  });

  return bosses.map(boss => {
    const recordsInWindow = roleBossRecords.filter(record => {
      // 支持单选和多选BOSS
      if (record.bossId === boss.id) return true;
      // 检查 bossIds 数组中是否包含该 boss
      if (record.bossIds && record.bossIds.includes(boss.id)) return true;
      return false;
    });

    const hasRecord = recordsInWindow.length > 0;
    const lastRecord = recordsInWindow.sort(
      (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime()
    )[0];

    return {
      bossId: boss.id,
      bossName: boss.name,
      hasRecord,
      lastRecordDate: lastRecord?.date,
      canAddRecord: !hasRecord
    };
  });
};

export const getRaidOverallCooldownStatus = (
  bossCooldowns: BossCooldownInfo[]
): { allInCooldown: boolean; completedCount: number; totalCount: number } => {
  const totalCount = bossCooldowns.length;
  const completedCount = bossCooldowns.filter(bc => bc.hasRecord).length;
  const allInCooldown = totalCount > 0 && completedCount === totalCount;

  return {
    allInCooldown,
    completedCount,
    totalCount
  };
};

export const canAddRecordForBoss = (
  raid: Raid,
  bossId: string | undefined,
  bossRecords: BossRecord[],
  roleId: string
): boolean => {
  if (!bossId) return true;

  const bosses = raid.bosses;
  if (!bosses || bosses.length === 0) return true;

  const cooldowns = calculateBossCooldowns(raid, bossRecords, roleId);
  const bossCooldown = cooldowns.find(bc => bc.bossId === bossId);

  return bossCooldown?.canAddRecord ?? true;
};

export const getAvailableBosses = (
  raid: Raid,
  bossRecords: BossRecord[],
  roleId: string
): { id: string; name: string }[] => {
  const bosses = raid.bosses;

  if (!bosses || bosses.length === 0) {
    return [];
  }

  const cooldowns = calculateBossCooldowns(raid, bossRecords, roleId);

  return cooldowns
    .filter(bc => bc.canAddRecord)
    .map(bc => ({ id: bc.bossId, name: bc.bossName }));
};
