import { Raid } from '../types';

export interface CooldownInfo {
  canAdd: boolean;
  remainingTime: number;
  nextAvailableTime: Date | null;
  cooldownType: 'none' | 'weekly' | 'biweekly'; // biweekly 代表一周两次的重置
  message: string;
  hasRecordInCurrentCycle: boolean;
}

export interface RecordLog {
  id: string;
  timestamp: Date;
  roleId: string;
  roleName: string;
  raidName: string;
  success: boolean;
  errorMessage?: string;
  details?: string;
}

const OPERATION_LOGS: RecordLog[] = [];

export const logOperation = (
  roleId: string,
  roleName: string,
  raidName: string,
  success: boolean,
  errorMessage?: string,
  details?: string
): void => {
  const log: RecordLog = {
    id: `log_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`,
    timestamp: new Date(),
    roleId,
    roleName,
    raidName,
    success,
    errorMessage,
    details
  };
  OPERATION_LOGS.push(log);

  console.log('[Record Operation]', success ? '✅' : '❌', log);
};

export const getOperationLogs = (): RecordLog[] => {
  return [...OPERATION_LOGS];
};

export const clearOperationLogs = (): void => {
  OPERATION_LOGS.length = 0;
};

export const getServerStandardTime = (): Date => {
  return new Date();
};

/**
 * 获取基于当前时间的“上一个”周一 07:00
 * 用于统计计算
 */
export const getLastMonday = (date: Date): Date => {
  const d = new Date(date);
  const day = d.getDay(); // 0 (Sun) - 6 (Sat)
  const hour = d.getHours();

  // 计算偏移量：如果今天是周一且还没到7点，或者今天是周日，都要往前推
  let daysToSubtract = 0;
  if (day === 1 && hour < 7) {
    daysToSubtract = 7;
  } else if (day === 0) {
    daysToSubtract = 6;
  } else if (day > 1) {
    daysToSubtract = day - 1;
  } else { // day === 1 && hour >= 7
    daysToSubtract = 0;
  }

  d.setDate(d.getDate() - daysToSubtract);
  d.setHours(7, 0, 0, 0);
  return d;
};

/**
 * 获取基于当前时间的“下一个”周一 07:00
 * 用于统计计算
 */
export const getNextMonday = (date: Date): Date => {
  const lastMonday = getLastMonday(date);
  const nextMonday = new Date(lastMonday);
  nextMonday.setDate(nextMonday.getDate() + 7);
  return nextMonday;
};

/**
 * 获取 10人本的当前 CD 周期范围
 * 周期1: 周一 07:00 ~ 周五 07:00
 * 周期2: 周五 07:00 ~ 下周一 07:00
 */
export const getTenPersonCycle = (date: Date): { start: Date, end: Date } => {
  const nowTime = date.getTime();
  const lastMonday = getLastMonday(date);
  const thisFriday = new Date(lastMonday);
  thisFriday.setDate(lastMonday.getDate() + 4); // 周一 + 4天 = 周五
  thisFriday.setHours(7, 0, 0, 0);

  const nextMonday = new Date(lastMonday);
  nextMonday.setDate(lastMonday.getDate() + 7); // 下周一

  // 检查当前时间是在 周一~周五 还是 周五~下周一
  if (nowTime >= lastMonday.getTime() && nowTime < thisFriday.getTime()) {
    // 处于上半周 (Mon 7:00 - Fri 7:00)
    return { start: lastMonday, end: thisFriday };
  } else {
    // 处于下半周 (Fri 7:00 - Next Mon 7:00)
    // 注意：如果当前是周一凌晨(比如3点)，getLastMonday会返回上周一，
    // 此时 thisFriday 是上周五。nowTime 肯定 > thisFriday。
    // 所以这里的逻辑是通用的。
    return { start: thisFriday, end: nextMonday };
  }
};

export const calculateCooldown = (
  raid: Raid,
  records: { date: string | number; bossIds?: string[]; bossId?: string }[],
  now: Date = getServerStandardTime()
): CooldownInfo => {
  const isTenPerson = raid.playerCount === 10;

  // 1. 确定当前的 CD 窗口
  let windowStart: Date;
  let windowEnd: Date;

  if (isTenPerson) {
    const cycle = getTenPersonCycle(now);
    windowStart = cycle.start;
    windowEnd = cycle.end;
  } else {
    // 25人本：周一到周一
    windowStart = getLastMonday(now);
    windowEnd = getNextMonday(now);
  }

  // 2. 检查窗口内是否有记录
  // 过滤出所有在 [windowStart, windowEnd) 区间内的记录
  const recordsInWindow = records.filter(r => {
    const rDate = new Date(r.date);
    return rDate >= windowStart && rDate < windowEnd;
  });

  if (recordsInWindow.length > 0) {
    // 如果该副本配置了Boss进度追踪（即存在boss记录），则不限制总记录数
    const trackBosses = raid.bosses && raid.bosses.length > 0;

    if (trackBosses) {
      // 计算当前周期内打掉的不同 boss 数量
      const killedBossIds = new Set<string>();
      recordsInWindow.forEach(r => {
        if (r.bossIds && r.bossIds.length > 0) {
          r.bossIds.forEach(id => killedBossIds.add(id));
        } else if (r.bossId) {
          killedBossIds.add(r.bossId);
        }
      });

      const totalBossesCount = raid.bosses!.length;
      const isCleared = killedBossIds.size >= totalBossesCount;

      return {
        canAdd: true,
        remainingTime: 0,
        nextAvailableTime: windowEnd,
        cooldownType: isTenPerson ? 'biweekly' : 'weekly',
        message: isCleared ? '本周期已全通（可继续分配Boss记录）' : '本周期可继续打剩余Boss',
        hasRecordInCurrentCycle: isCleared
      };
    }

    // 已有记录，CD 中
    return {
      canAdd: false,
      remainingTime: windowEnd.getTime() - now.getTime(),
      nextAvailableTime: windowEnd,
      cooldownType: isTenPerson ? 'biweekly' : 'weekly',
      message: `本周期记录已存在，${windowEnd.toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })} 后刷新`,
      hasRecordInCurrentCycle: true
    };
  } else {
    // 无记录，可添加
    return {
      canAdd: true,
      remainingTime: 0,
      nextAvailableTime: null,
      cooldownType: 'none',
      message: '当前可添加记录',
      hasRecordInCurrentCycle: false
    };
  }
};

export const formatRemainingTime = (ms: number): string => {
  if (ms <= 0) return '可添加';

  const days = Math.floor(ms / (1000 * 60 * 60 * 24));
  const hours = Math.floor((ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
  const minutes = Math.floor((ms % (1000 * 60 * 60)) / (1000 * 60));

  if (days > 0) {
    return `${days}天${hours}小时`;
  }
  if (hours > 0) {
    return `${hours}小时${minutes}分钟`;
  }
  return `${minutes}分钟`;
};

export const getRaidCooldownRules = (raid: Raid): string => {
  const isTenPerson = raid.playerCount === 10;
  if (!isTenPerson) {
    return '25人本：每周一 7:00 刷新';
  }
  return '10人本：每周一 7:00 和 周五 7:00 刷新';
};

export interface RaidRefreshInfo {
  nextRefreshTime: Date | null;
  formattedTime: string;
  isRefreshing: boolean;
  refreshCount: number;
  refreshSchedule: string;
}

export const getRaidRefreshInfo = (raid: Raid, now: Date = new Date()): RaidRefreshInfo => {
  const isTenPerson = raid.playerCount === 10;

  let nextRefresh: Date;
  let scheduleStr: string;

  if (isTenPerson) {
    const cycle = getTenPersonCycle(now);
    nextRefresh = cycle.end;
    scheduleStr = '周一/周五 7:00';
  } else {
    nextRefresh = getNextMonday(now);
    scheduleStr = '每周一 7:00';
  }

  return {
    nextRefreshTime: nextRefresh,
    formattedTime: '', // 这一项似乎没用到，或者在外层格式化
    isRefreshing: false,
    refreshCount: isTenPerson ? 2 : 1,
    refreshSchedule: scheduleStr
  };
};

export const formatCountdown = (ms: number): string => {
  if (ms <= 0) return '已刷新';

  const days = Math.floor(ms / (1000 * 60 * 60 * 24));
  const hours = Math.floor((ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
  const minutes = Math.floor((ms % (1000 * 60 * 60)) / (1000 * 60));
  const seconds = Math.floor((ms % (1000 * 60)) / 1000);

  const p = (n: number) => n.toString().padStart(2, '0');

  if (days > 0) {
    return `${days}天 ${p(hours)}:${p(minutes)}:${p(seconds)}`;
  }
  return `${p(hours)}:${p(minutes)}:${p(seconds)}`;
};
