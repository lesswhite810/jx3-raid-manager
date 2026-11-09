import { useEffect, useState } from 'react';
import { Season } from '../types';
import { db } from '../services/db';
import { getTrialProgressPeriod, TrialProgressPeriod } from '../utils/trialProgress';

export interface TrialSeasonProgress {
  /** undefined 表示仍在加载；null 表示无法确定周期，此时不能按本周统计试炼进度。 */
  trialProgressPeriod: TrialProgressPeriod | null | undefined;
  /** 当前赛季，供需要赛季起点的统计使用。 */
  currentSeason: Season | null;
}

/**
 * 读取当前赛季与大版本信息，推导试炼进度的统计周期。
 *
 * 试炼进度以大版本为周期（不是自然周），所以凡是需要"本周试炼次数"的地方
 * 都必须拿到同一份周期，否则会出现页面之间数字不一致。
 */
export const useTrialSeasonProgress = (): TrialSeasonProgress => {
  const [trialProgressPeriod, setTrialProgressPeriod] = useState<TrialProgressPeriod | null | undefined>(undefined);
  const [currentSeason, setCurrentSeason] = useState<Season | null>(null);

  useEffect(() => {
    let cancelled = false;
    setTrialProgressPeriod(undefined);

    Promise.all([db.getCurrentSeason(), db.getSeasons(), db.getGameVersions()])
      .then(([season, seasons, versions]) => {
        if (cancelled) return;
        setCurrentSeason(season);
        setTrialProgressPeriod(getTrialProgressPeriod(season, seasons, versions));
      })
      .catch(() => {
        if (cancelled) return;
        setCurrentSeason(null);
        setTrialProgressPeriod(null);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return { trialProgressPeriod, currentSeason };
};
