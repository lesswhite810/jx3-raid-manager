import { Raid } from '../types';

export interface CooldownInfo {
  canAdd: boolean;
  remainingTime: number;
  nextAvailableTime: Date | null;
  cooldownType: 'none' | 'weekly' | 'window' | 'daily-bonus';
  message: string;
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

const getWeekStart = (date: Date): Date => {
  const d = new Date(date);
  const dayOfWeek = d.getDay();
  const mondayOffset = dayOfWeek === 0 ? -6 : 1 - dayOfWeek;
  d.setDate(d.getDate() + mondayOffset);
  d.setHours(7, 0, 0, 0);
  return d;
};

const getFridayStart = (date: Date): Date => {
  const d = new Date(date);
  const dayOfWeek = d.getDay();
  let fridayOffset = 5 - dayOfWeek;
  if (fridayOffset < 0) fridayOffset += 7;
  d.setDate(d.getDate() + fridayOffset);
  d.setHours(7, 0, 0, 0);
  return d;
};

const getNextMonday = (date: Date): Date => {
  const d = new Date(date);
  const dayOfWeek = d.getDay();
  const daysUntilMonday = dayOfWeek === 0 ? 1 : 8 - dayOfWeek;
  d.setDate(d.getDate() + daysUntilMonday);
  d.setHours(7, 0, 0, 0);
  return d;
};

const getNextFriday = (date: Date): Date => {
  const d = new Date(date);
  const dayOfWeek = d.getDay();
  let daysUntilFriday = 5 - dayOfWeek;
  if (daysUntilFriday <= 0) daysUntilFriday += 7;
  d.setDate(d.getDate() + daysUntilFriday);
  d.setHours(7, 0, 0, 0);
  return d;
};

export const calculateCooldown = (
  raid: Raid,
  records: { date: string }[],
  now: Date = getServerStandardTime()
): CooldownInfo => {
  const isTenPerson = raid.playerCount === 10;
  const weekStart = getWeekStart(now);
  const weekEnd = new Date(weekStart);
  weekEnd.setDate(weekStart.getDate() + 7);
  
  const fridayStart = getFridayStart(now);
  const nextFriday = getNextFriday(now);
  const nextMonday = getNextMonday(now);
  
  const weekRecords = records.filter(r => new Date(r.date) >= weekStart && new Date(r.date) < weekEnd);
  
  if (!isTenPerson) {
    const weekRecordCount = weekRecords.length;
    
    if (weekRecordCount < 1) {
      return {
        canAdd: true,
        remainingTime: 0,
        nextAvailableTime: null,
        cooldownType: 'none',
        message: '本周可添加 1 次记录'
      };
    }
    
    return {
      canAdd: false,
      remainingTime: nextMonday.getTime() - now.getTime(),
      nextAvailableTime: nextMonday,
      cooldownType: 'weekly',
      message: `本周记录已添加，下周 ${nextMonday.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' })} 7:00 后可添加`
    };
  }
  
  const fridayStartTime = fridayStart.getTime();
  const nextFridayTime = nextFriday.getTime();
  const nowTime = now.getTime();
  
  const weekdayRecords = weekRecords.filter(r => new Date(r.date).getTime() < fridayStartTime);
  
  const dayOfWeek = now.getDay();
  
  if (dayOfWeek >= 1 && dayOfWeek <= 4) {
    const thisWeekdayStart = new Date(now);
    thisWeekdayStart.setHours(7, 0, 0, 0);
    const nextWeekdayStart = new Date(thisWeekdayStart);
    nextWeekdayStart.setDate(thisWeekdayStart.getDate() + 1);
    
    const weekdayWindowRecords = weekRecords.filter(r => {
      const recordTime = new Date(r.date).getTime();
      return recordTime >= thisWeekdayStart.getTime() && recordTime < nextWeekdayStart.getTime();
    });
    
    if (weekdayRecords.length === 0 && weekdayWindowRecords.length === 0) {
      return {
        canAdd: true,
        remainingTime: 0,
        nextAvailableTime: null,
        cooldownType: 'none',
        message: '当前时间窗口可添加 1 次记录'
      };
    }
    
    return {
      canAdd: false,
      remainingTime: nextWeekdayStart.getTime() - nowTime,
      nextAvailableTime: nextWeekdayStart,
      cooldownType: 'window',
      message: `今日记录已添加，明天 7:00 后可添加`
    };
  }
  
  if (dayOfWeek === 5) {
    if (nowTime < fridayStartTime) {
      const thisFridayRecords = weekRecords.filter(r => {
        const recordTime = new Date(r.date).getTime();
        const prevFridayStart = new Date(fridayStart);
        prevFridayStart.setDate(fridayStart.getDate() - 7);
        return recordTime >= prevFridayStart.getTime() && recordTime < fridayStartTime;
      });
      
      if (thisFridayRecords.length < 1) {
        return {
          canAdd: true,
          remainingTime: 0,
          nextAvailableTime: null,
          cooldownType: 'none',
          message: '周五刷新后可添加 1 次记录'
        };
      }
      
      return {
        canAdd: false,
        remainingTime: fridayStart.getTime() - nowTime,
        nextAvailableTime: fridayStart,
        cooldownType: 'window',
        message: `周五 ${fridayStart.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })} 刷新后可添加`
      };
    }
    
    const todayRecords = weekRecords.filter(r => {
      const recordTime = new Date(r.date).getTime();
      return recordTime >= fridayStartTime && recordTime < nextFridayTime;
    });
    
    if (todayRecords.length < 1) {
      return {
        canAdd: true,
        remainingTime: 0,
        nextAvailableTime: null,
        cooldownType: 'daily-bonus',
        message: '今日可额外添加 1 次记录'
      };
    }
    
    return {
      canAdd: false,
      remainingTime: nextFriday.getTime() - nowTime,
      nextAvailableTime: nextFriday,
      cooldownType: 'weekly',
      message: `今日已添加，下个周期 ${nextFriday.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' })} 7:00 后可添加`
    };
  }
  
  if (dayOfWeek === 6 || dayOfWeek === 0) {
    const nextDay7AM = new Date(now);
    nextDay7AM.setHours(7, 0, 0, 0);
    if (nowTime >= nextDay7AM.getTime()) {
      nextDay7AM.setDate(nextDay7AM.getDate() + 1);
    }
    
    return {
      canAdd: false,
      remainingTime: nextDay7AM.getTime() - nowTime,
      nextAvailableTime: nextDay7AM,
      cooldownType: 'window',
      message: `周末不开放，明天 7:00 后可添加`
    };
  }
  
  return {
    canAdd: true,
    remainingTime: 0,
    nextAvailableTime: null,
    cooldownType: 'none',
    message: '可添加记录'
  };
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
    return '25人本：每周一7点刷新，每周可添加1次记录';
  }
  return '10人本：周一至周五每天7点刷新，每个时间窗口可添加1次，周五刷新后可额外添加1次';
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
  const dayOfWeek = now.getDay();
  const nowTime = now.getTime();
  
  const weekStart = getWeekStart(now);
  const fridayStart = getFridayStart(now);
  const nextMonday = getNextMonday(now);
  
  if (!isTenPerson) {
    return {
      nextRefreshTime: nextMonday,
      formattedTime: '',
      isRefreshing: false,
      refreshCount: 1,
      refreshSchedule: '每周一 7:00'
    };
  }
  
  if (dayOfWeek === 0 || (dayOfWeek === 1 && nowTime < weekStart.getTime() + 7 * 60 * 60 * 1000)) {
    return {
      nextRefreshTime: fridayStart,
      formattedTime: '',
      isRefreshing: false,
      refreshCount: 2,
      refreshSchedule: '周一/周五 7:00'
    };
  }
  
  if (dayOfWeek >= 1 && dayOfWeek < 5) {
    return {
      nextRefreshTime: fridayStart,
      formattedTime: '',
      isRefreshing: false,
      refreshCount: 2,
      refreshSchedule: '周一/周五 7:00'
    };
  }
  
  return {
    nextRefreshTime: nextMonday,
    formattedTime: '',
    isRefreshing: false,
    refreshCount: 2,
    refreshSchedule: '周一/周五 7:00'
  };
};

export const formatCountdown = (ms: number): string => {
  if (ms <= 0) return '已刷新';
  
  const days = Math.floor(ms / (1000 * 60 * 60 * 24));
  const hours = Math.floor((ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
  const minutes = Math.floor((ms % (1000 * 60 * 60)) / (1000 * 60));
  const seconds = Math.floor((ms % (1000 * 60)) / 1000);
  
  if (days > 0) {
    return `${days}天 ${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
};
