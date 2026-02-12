import { Raid, BossCooldownInfo, BossRecord } from '../types';
import { getRaidBossConfig } from '../data/raidBosses';
import { getLastMonday7AM, getNextMonday7AM, getTenPersonCycle } from './cooldownManager';

export const calculateBossCooldowns = (
  raid: Raid,
  bossRecords: BossRecord[],
  roleId: string
): BossCooldownInfo[] => {
  const config = getRaidBossConfig(raid.name, raid.difficulty, raid.playerCount);
  
  if (!config || !config.hasBossTracking) {
    return [];
  }

  const now = new Date();
  const isTenPerson = raid.playerCount === 10;

  let windowStart: Date;
  let windowEnd: Date;

  if (isTenPerson) {
    const cycle = getTenPersonCycle(now);
    windowStart = cycle.start;
    windowEnd = cycle.end;
  } else {
    windowStart = getLastMonday7AM(now);
    windowEnd = getNextMonday7AM(now);
  }

  const roleBossRecords = bossRecords.filter(
    record => record.roleId === roleId
  );

  return config.bosses.map(boss => {
    const recordsInWindow = roleBossRecords.filter(record => {
      if (record.bossId !== boss.id) return false;
      const recordDate = new Date(record.date);
      return recordDate >= windowStart && recordDate < windowEnd;
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

  const config = getRaidBossConfig(raid.name, raid.difficulty, raid.playerCount);
  if (!config || !config.hasBossTracking) return true;

  const cooldowns = calculateBossCooldowns(raid, bossRecords, roleId);
  const bossCooldown = cooldowns.find(bc => bc.bossId === bossId);

  return bossCooldown?.canAddRecord ?? true;
};

export const getAvailableBosses = (
  raid: Raid,
  bossRecords: BossRecord[],
  roleId: string
): { id: string; name: string }[] => {
  const config = getRaidBossConfig(raid.name, raid.difficulty, raid.playerCount);
  
  if (!config || !config.hasBossTracking) {
    return [];
  }

  const cooldowns = calculateBossCooldowns(raid, bossRecords, roleId);
  
  return cooldowns
    .filter(bc => bc.canAddRecord)
    .map(bc => ({ id: bc.bossId, name: bc.bossName }));
};
