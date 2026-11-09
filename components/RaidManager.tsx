import React, { useState, useMemo, useEffect, useRef } from 'react';
import { Raid, Config } from '../types';
import { Trash2, Shield, Power, Star } from 'lucide-react';
import { getRaidKey } from '../utils/raidUtils';
import { toast } from '../utils/toastManager';
import { TrialPlaceManager } from './TrialPlaceManager';
import { BaizhanManager } from './BaizhanManager';
import { VersionSelect } from './VersionSelect';
import { TrialDraw, TrialPlaceRecord, Account, BaizhanRecord, RaidRecord } from '../types';
import { db } from '../services/db';
import { useCancellableEffect } from '../hooks/useCancellableEffect';
import { RaidDetail } from './RaidDetail';
import { RaidCdOverview } from './RaidCdOverview';
import { PendingRecordsPanel } from './PendingRecordsPanel';
import { AddRecordModal } from './AddRecordModal';
import { RoleRecordsModal } from './RoleRecordsModal';
import { AddTrialRecordModal } from './AddTrialRecordModal';
import { TrialRoleRecordsModal } from './TrialRoleRecordsModal';
import { AddBaizhanRecordModal } from './AddBaizhanRecordModal';
import { BaizhanRoleRecordsModal } from './BaizhanRoleRecordsModal';
import { useTrialSeasonProgress } from '../hooks/useTrialSeasonProgress';
import { CdCellStatus, CdOverviewRow } from '../utils/raidCdOverview';
import { buildWeeklyRaidGroups, SeasonContext, WeeklyRaidGroup } from '../utils/weeklyRaids';

interface RaidManagerProps {
  raids: Raid[];
  setRaids: React.Dispatch<React.SetStateAction<Raid[]>>;
  records: RaidRecord[];
  setRecords: React.Dispatch<React.SetStateAction<RaidRecord[]>>;
  pendingRecords?: RaidRecord[];
  onEditRecord?: (record: RaidRecord) => void;
  onEditBaizhanRecord?: (record: BaizhanRecord) => void;
  trialRecords: TrialPlaceRecord[];
  baizhanRecords: BaizhanRecord[];
  accounts: Account[];
  config?: Config;
  onRefreshRecords?: () => void;
  onRefreshTrialRecords?: () => void;
  onRefreshBaizhanRecords?: () => void;
  /** 配置变更上抛给 App，避免子组件直连数据库形成双写 */
  onConfigChange?: (updater: (prev: Config) => Config) => void;
  weeklyRaidNames: string[];
  weeklyRaidStatus?: 'loading' | 'success' | 'error';
}

interface MergedRaid {
  name: string;
  version: string;
  raids: Raid[];
  isActive: boolean;
  disabled: boolean;
  difficultyLabels: { [key: string]: string };
}

/** CD 概览里副本单元格的点击目标：mode 决定弹的是「添加记录」还是「记录详情」 */
interface RaidCellAction {
  mode: 'add' | 'view';
  raid: Raid;
  row: CdOverviewRow;
}

/** 试炼之地 / 百战单元格的点击目标 */
interface ActivityCellAction {
  mode: 'add' | 'view';
  row: CdOverviewRow;
}

const DIFFICULTY_LABELS: Record<string, string> = {
  '普通': '普通',
  '英雄': '英雄',
  '挑战': '挑战'
};

const DIFFICULTY_COLORS: Record<string, string> = {
  '普通': 'bg-ds-success-soft text-ds-success-strong border-ds-success-soft dark:bg-ds-success-soft/30 dark:text-ds-success-strong dark:border-ds-success-soft',
  '英雄': 'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:border-blue-800',
  '挑战': 'bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-900/30 dark:text-rose-400 dark:border-rose-800'
};

// 用于在下拉框中标识「概览」视图，避免与真实赛季名冲突。
// 概览 = CD 概览表；其余版本值仍按赛季展示副本卡片。
const OVERVIEW_VIEW = '__favorite__';



export const RaidManager: React.FC<RaidManagerProps> = ({
  raids,
  setRaids,
  records,
  setRecords,
  pendingRecords,
  onEditRecord,
  onEditBaizhanRecord,
  trialRecords,
  baizhanRecords,
  accounts,
  config,
  onRefreshRecords,
  onRefreshTrialRecords,
  onRefreshBaizhanRecords,
  onConfigChange,
  weeklyRaidNames,
  weeklyRaidStatus = 'success'
}) => {
  // 当前页面：概览（含 CD 概览表与赛季副本卡片）/ 试炼之地 / 百战 / 单个副本详情由 selectedRaid 控制。
  // 试炼之地与百战不再占据顶部页签，只通过 CD 概览表的列头进入。
  const [activeTab, setActiveTab] = useState<'raid' | 'trial' | 'baizhan'>('raid');
  const [selectedRaid, setSelectedRaid] = useState<Raid | null>(null);
  // 点击 CD 概览里的「待确认」单元格时递增，用于命令式打开待确认弹窗
  const [pendingPanelOpenSignal, setPendingPanelOpenSignal] = useState(0);

  // CD 概览单元格点击目标：未清/部分清 → 添加记录，已清 → 查看该角色在该副本的记录
  const [raidCellAction, setRaidCellAction] = useState<RaidCellAction | null>(null);
  const [trialCellAction, setTrialCellAction] = useState<ActivityCellAction | null>(null);
  const [baizhanCellAction, setBaizhanCellAction] = useState<ActivityCellAction | null>(null);

  // 检查是否有角色启用了试炼之地 / 百战
  const { hasTrialEnabled, hasBaizhanEnabled } = useMemo(() => {
    let trial = false;
    let baizhan = false;
    for (const acc of accounts) {
      if (acc.disabled) continue;
      for (const role of acc.roles) {
        if (role.disabled || role.isClient) continue;
        if (role.visibility?.trial !== false) trial = true;
        if (role.visibility?.baizhan !== false) baizhan = true;
        if (trial && baizhan) break;
      }
      if (trial && baizhan) break;
    }
    return { hasTrialEnabled: trial, hasBaizhanEnabled: baizhan };
  }, [accounts]);

  // 当前页面因没有启用角色而不可用时，自动切回概览
  useEffect(() => {
    if (activeTab === 'trial' && !hasTrialEnabled) {
      setActiveTab('raid');
    } else if (activeTab === 'baizhan' && !hasBaizhanEnabled) {
      setActiveTab('raid');
    }
  }, [hasTrialEnabled, hasBaizhanEnabled, activeTab]);

  // 试炼进度周期与试炼之地页共用同一份数据，避免两处次数不一致
  const { trialProgressPeriod } = useTrialSeasonProgress();

  const [versionOrderMap, setVersionOrderMap] = useState<Record<string, number>>({});
  const [isOrderLoaded, setIsOrderLoaded] = useState(false);

  // 收藏状态
  const [favoriteRaids, setFavoriteRaids] = useState<Set<string>>(new Set());
  // 标记收藏是否已加载完成，避免在 selectedVersion 初始化时被空状态误导
  const favoritesLoadedRef = useRef(false);
  // 当前赛季上下文：赛季 id 用于同版本内区分新旧赛季，大版本名用于兜底
  // （早期预置副本只写了 version、库中 season_id 为 NULL，只能按大版本判断）。
  const [seasonContext, setSeasonContext] = useState<SeasonContext>({
    currentSeasonId: null,
    currentVersionName: null,
  });

  useCancellableEffect((signal) => {
    const loadSeasonContext = async () => {
      try {
        const season = await db.getCurrentSeason();
        if (signal.cancelled) return;
        const seasonId = Number.isInteger(season?.id) ? (season?.id ?? null) : null;

        let versionName: string | null = null;
        if (season && Number.isInteger(season.versionId)) {
          const versions = await db.getGameVersions();
          if (signal.cancelled) return;
          versionName = versions.find(version => version.id === season.versionId)?.name ?? null;
        }

        setSeasonContext({ currentSeasonId: seasonId, currentVersionName: versionName });
      } catch (err) {
        console.error('Failed to load current season:', err);
      }
    };
    loadSeasonContext();
  }, []);

  // 加载收藏列表
  useCancellableEffect((signal) => {
    const loadFavorites = async () => {
      try {
        const favorites = await db.getFavoriteRaids();
        if (signal.cancelled) return;
        setFavoriteRaids(new Set(favorites));
      } catch (err) {
        console.error('Failed to load favorite raids:', err);
      } finally {
        favoritesLoadedRef.current = true;
      }
    };
    loadFavorites();
  }, []);

  // 切换收藏状态
  const toggleFavorite = async (raidName: string) => {
    try {
      if (favoriteRaids.has(raidName)) {
        await db.removeFavoriteRaid(raidName);
        setFavoriteRaids(prev => {
          const newSet = new Set(prev);
          newSet.delete(raidName);
          return newSet;
        });
        toast.success(`已取消收藏 ${raidName}`);
      } else {
        await db.addFavoriteRaid(raidName);
        setFavoriteRaids(prev => {
          const newSet = new Set(prev);
          newSet.add(raidName);
          return newSet;
        });
        toast.success(`已收藏 ${raidName}`);
      }
    } catch (err) {
      console.error('Failed to toggle favorite:', err);
      toast.error('操作失败，请重试');
    }
  };

  useCancellableEffect((signal) => {
    const fetchVersions = async () => {
      try {
        const cacheKey = 'raid_versions_cache_v2';
        const cache = await db.getCache(cacheKey);
        if (signal.cancelled) return;

        let shouldFetch = true;

        if (cache && cache.updatedAt) {
          const now = new Date();
          const lastUpdate = new Date(cache.updatedAt);
          // 每天早上7点刷新
          const refreshTime = new Date(now);
          refreshTime.setHours(7, 0, 0, 0);

          if (now.getTime() < refreshTime.getTime()) {
            refreshTime.setDate(refreshTime.getDate() - 1);
          }

          if (lastUpdate.getTime() > refreshTime.getTime()) {
            shouldFetch = false;
            const orderMap: Record<string, number> = {};
            (cache.value as string[]).forEach((v: string, i: number) => {
              orderMap[v] = i;
            });
            setVersionOrderMap(orderMap);
          }
        }

        if (shouldFetch) {
          const versions = await db.getRaidVersions();
          if (signal.cancelled) return;
          const orderMap: Record<string, number> = {};
          versions.forEach((v: string, i: number) => {
            orderMap[v] = i;
          });
          setVersionOrderMap(orderMap);
          await db.saveCache(cacheKey, versions);
        }
        if (signal.cancelled) return;
        setIsOrderLoaded(true);
      } catch (err) {
        console.error('Failed to fetch raid versions order', err);
        if (!signal.cancelled) setIsOrderLoaded(true);
      }
    };

    fetchVersions();
  }, []);

  const versions = useMemo(() => {
    const versionSet = new Set<string>();
    raids.forEach(raid => {
      if (raid.version) {
        versionSet.add(raid.version);
      }
    });

    return Array.from(versionSet).sort((a, b) => {
      const orderA = versionOrderMap[a] !== undefined ? versionOrderMap[a] : 999;
      const orderB = versionOrderMap[b] !== undefined ? versionOrderMap[b] : 999;
      return orderA - orderB;
    });
  }, [raids, versionOrderMap]);

  // 默认选择最新版本（由于需要等待后端字典加载完毕，故初始化为空/降级值，随后通过效应重置）
  const [selectedVersion, setSelectedVersion] = useState<string>('all');

  const isStaticRaid = (raid: Raid) => {
    return !!raid.static;
  };

  const weeklyRaidGroups = useMemo<WeeklyRaidGroup[]>(
    () => buildWeeklyRaidGroups(raids, weeklyRaidNames, favoriteRaids, seasonContext),
    [raids, weeklyRaidNames, favoriteRaids, seasonContext],
  );

  // 本周团队秘境区域已经展示过的难度（按副本难度 key）。
  // 收藏区按 key 去重，而不是按副本名：历史赛季副本在本周区只展示 10 人难度，
  // 其 25 人难度仍需留在收藏区，否则用户会失去这些难度的入口。
  const weeklyCoveredRaidKeys = useMemo(
    () => new Set(weeklyRaidGroups.flatMap(group => group.raids.map(getRaidKey))),
    [weeklyRaidGroups],
  );

  // 计算收藏的副本列表（只显示激活的难度）
  // 顺序由后端按版本排序返回；已在本周团队秘境展示过的难度不再重复
  const favoriteMergedRaids = useMemo(() => {
    const favoriteList: MergedRaid[] = [];

    // 按副本名称分组，保持 favoriteRaids 的顺序（后端已按版本排序）
    const nameGroups = new Map<string, Raid[]>();
    raids.forEach(raid => {
      // 只处理收藏的副本
      if (!favoriteRaids.has(raid.name)) return;
      // 该难度已在本周团队秘境区域展示
      if (weeklyCoveredRaidKeys.has(getRaidKey(raid))) return;
      // 只显示激活的难度
      if (!raid.isActive) return;

      const key = raid.name;
      if (!nameGroups.has(key)) {
        nameGroups.set(key, []);
      }
      nameGroups.get(key)!.push(raid);
    });

    // 按照 favoriteRaids 的顺序（Set 保持插入顺序，后端已按版本排序）
    favoriteRaids.forEach(raidName => {
      const raidList = nameGroups.get(raidName);
      if (!raidList || raidList.length === 0) return;

      const difficultyLabels: { [key: string]: string } = {};
      const hasMultiplePlayerCounts = new Set(raidList.map(r => r.playerCount)).size > 1;

      raidList.forEach(raid => {
        const key = getRaidKey(raid);
        const difficultyLabel = DIFFICULTY_LABELS[raid.difficulty];
        if (hasMultiplePlayerCounts) {
          difficultyLabels[key] = `${raid.playerCount}人${difficultyLabel}`;
        } else {
          difficultyLabels[key] = difficultyLabel;
        }
      });

      const sortedRaids = [...raidList].sort((a, b) => {
        const playerCountOrder = { 10: 0, 25: 1 };
        const difficultyOrder: Record<string, number> = { '普通': 0, '英雄': 1, '挑战': 2 };
        if (playerCountOrder[a.playerCount] !== playerCountOrder[b.playerCount]) {
          return playerCountOrder[a.playerCount] - playerCountOrder[b.playerCount];
        }
        return difficultyOrder[a.difficulty] - difficultyOrder[b.difficulty];
      });

      favoriteList.push({
        name: raidName,
        version: sortedRaids[0].version || '其他',
        raids: sortedRaids,
        isActive: true,
        disabled: false,
        difficultyLabels
      });
    });

    return favoriteList;
  }, [raids, favoriteRaids, weeklyCoveredRaidKeys]);

  const hasCollectionEntries = favoriteMergedRaids.length > 0 || weeklyRaidGroups.length > 0 || weeklyRaidStatus !== 'success';

  // 初始化 / 校正 selectedVersion：等收藏与版本字典都加载完后再决定默认视图
  useEffect(() => {
    if (!favoritesLoadedRef.current || !isOrderLoaded || versions.length === 0) {
      return;
    }

    setSelectedVersion((prev) => {
      // 概览视图但已无任何可展示内容：回退到最新版本
      if (prev === OVERVIEW_VIEW && !hasCollectionEntries) {
        return versions[0];
      }
      // 概览视图且有内容：保留
      if (prev === OVERVIEW_VIEW) {
        return prev;
      }
      // 用户已主动选择一个真实赛季：保留
      if (versions.includes(prev)) {
        return prev;
      }
      // 初始值 'all' 或不在现存列表里：有内容则默认进入概览视图，否则推到数组首位（最新版本）
      return hasCollectionEntries ? OVERVIEW_VIEW : versions[0];
    });
  }, [versions, isOrderLoaded, favoriteMergedRaids, hasCollectionEntries]);

  const filteredRaids = useMemo(() => {
    return raids.filter(raid => raid.version === selectedVersion);
  }, [raids, selectedVersion]);

  const groupedRaids = useMemo(() => {
    const groups: Record<string, Raid[]> = {};
    filteredRaids.forEach(raid => {
      const version = raid.version || '其他';
      if (!groups[version]) {
        groups[version] = [];
      }
      groups[version].push(raid);
    });

    Object.keys(groups).forEach(version => {
      groups[version].reverse();
    });

    return groups;
  }, [filteredRaids]);

  const mergedGroupedRaids = useMemo(() => {
    const mergedGroups: Record<string, MergedRaid[]> = {};

    Object.entries(groupedRaids).forEach(([version, versionRaids]) => {
      const mergedRaids: MergedRaid[] = [];

      // 首先按人数类型分组
      const raidsByPlayerCount: Record<number, Raid[]> = {
        10: [],
        25: []
      };

      versionRaids.forEach(raid => {
        raidsByPlayerCount[raid.playerCount].push(raid);
      });

      // 处理每个副本名称
      const nameGroups = new Map<string, Raid[]>();
      versionRaids.forEach(raid => {
        const key = raid.name;
        if (!nameGroups.has(key)) {
          nameGroups.set(key, []);
        }
        nameGroups.get(key)!.push(raid);
      });

      nameGroups.forEach((raids, name) => {
        const difficultyLabels: { [key: string]: string } = {};

        // 只处理难度标签，不修改 isActive 状态
        const hasMultiplePlayerCounts = new Set(raids.map(r => r.playerCount)).size > 1;

        raids.forEach(raid => {
          const key = getRaidKey(raid);
          const difficultyLabel = DIFFICULTY_LABELS[raid.difficulty];

          // 如果存在多种人数版本，在标签中添加人数标识
          if (hasMultiplePlayerCounts) {
            difficultyLabels[key] = `${raid.playerCount}人${difficultyLabel}`;
          } else {
            difficultyLabels[key] = difficultyLabel;
          }
        });

        const sortedRaids = raids.sort((a, b) => {
          const playerCountOrder = { 10: 0, 25: 1 };
          const difficultyOrder: Record<string, number> = { '普通': 0, '英雄': 1, '挑战': 2 };

          if (playerCountOrder[a.playerCount] !== playerCountOrder[b.playerCount]) {
            return playerCountOrder[a.playerCount] - playerCountOrder[b.playerCount];
          }

          return difficultyOrder[a.difficulty] - difficultyOrder[b.difficulty];
        });

        const mergedRaid: MergedRaid = {
          name,
          version: raids[0].version || '其他',
          raids: sortedRaids,
          isActive: sortedRaids.some(r => r.isActive),
          disabled: sortedRaids.every(r => !r.isActive),
          difficultyLabels
        };
        mergedRaids.push(mergedRaid);
      });

      // 这里不再需要对版本下的副本数组按版本再次排序，因为它们本身就是同一个版本的副本。
      // 如果是为了某种内部顺序，我们可以保留原状或按名称排序
      mergedGroups[version] = mergedRaids;
    });

    return mergedGroups;
  }, [groupedRaids]);



  const toggleRaidStatus = (name: string) => {
    const raidsWithName = raids.filter(r => r.name === name);
    if (raidsWithName.length === 0) return;

    const currentActiveCount = raidsWithName.filter(r => r.isActive).length;
    const newStatus = currentActiveCount === 0;

    setRaids(prev => prev.map(r => {
      if (r.name === name) {
        return { ...r, isActive: newStatus };
      }
      return r;
    }));

    const raidName = raidsWithName[0].name;
    toast.success(`${raidName} 已${newStatus ? '启用所有难度' : '禁用所有难度'}`);
  };

  const toggleRaidDifficultyStatus = (key: string) => {
    setRaids(prev => prev.map(r => {
      if (getRaidKey(r) === key) {
        const newStatus = !r.isActive;
        return { ...r, isActive: newStatus };
      }
      return r;
    }));

    const raid = raids.find(r => getRaidKey(r) === key);
    if (raid) {
      const isNowActive = !raid.isActive;
      toast.success(`${raid.name} - ${DIFFICULTY_LABELS[raid.difficulty]} 已${isNowActive ? '启用' : '禁用'}`);
    }
  };

  // ==== CD 概览单元格点击：未清/部分清去补记录，已清去看记录 ====

  const handleRaidCellClick = (raid: Raid, row: CdOverviewRow, status: CdCellStatus) => {
    setRaidCellAction({ mode: status === 'complete' ? 'view' : 'add', raid, row });
  };

  const handleTrialCellClick = (row: CdOverviewRow, status: CdCellStatus) => {
    if (status === 'na') return;
    // 试炼记录要挂在某个大版本的进度周期下，周期没就绪时先不让补录，避免记到错误赛季
    if (status !== 'complete' && trialProgressPeriod === undefined) {
      toast.info('试炼进度数据正在加载，请稍后再试');
      return;
    }
    if (status !== 'complete' && trialProgressPeriod === null) {
      toast.error('无法确定当前试炼大版本，请检查赛季配置后重试');
      return;
    }
    setTrialCellAction({ mode: status === 'complete' ? 'view' : 'add', row });
  };

  const handleBaizhanCellClick = (row: CdOverviewRow, status: CdCellStatus) => {
    setBaizhanCellAction({ mode: status === 'complete' ? 'view' : 'add', row });
  };

  const handleAddRaidRecord = async (recordData: Partial<RaidRecord>) => {
    try {
      await db.addRecord(recordData as RaidRecord);
      onRefreshRecords?.();
      toast.success('记录添加成功');
    } catch (error) {
      console.error('添加记录失败:', error);
      toast.error('添加记录失败，请重试');
    }
  };

  const handleAddTrialRecord = async (
    record: TrialPlaceRecord,
    options?: { preserveFlippedIndex?: boolean }
  ) => {
    try {
      await db.addTrialRecord(record, options?.preserveFlippedIndex);
      onRefreshTrialRecords?.();
    } catch (error) {
      console.error('添加试炼记录失败:', error);
      toast.error('添加试炼记录失败，请重试');
    }
  };

  const handleAddBaizhanRecord = async (record: BaizhanRecord) => {
    try {
      await db.addBaizhanRecord(record);
      onRefreshBaizhanRecords?.();
    } catch (error) {
      console.error('添加百战记录失败:', error);
      toast.error('添加百战记录失败，请重试');
    }
  };

  const handleDeleteTrialRecord = async (recordId: string) => {
    try {
      await db.deleteTrialRecord(recordId);
      onRefreshTrialRecords?.();
      toast.success('删除试炼记录成功');
    } catch (error) {
      console.error('删除试炼记录失败:', error);
      toast.error('删除试炼记录失败');
    }
  };

  const handleDeleteBaizhanRecord = async (recordId: string) => {
    try {
      await db.deleteBaizhanRecord(recordId);
      onRefreshBaizhanRecords?.();
      toast.success('删除百战记录成功');
    } catch (error) {
      console.error('删除百战记录失败:', error);
      toast.error('删除百战记录失败');
    }
  };

  const handleUpdateTrialDrawFlippedIndex = async (
    _recordId: string,
    drawId: string,
    flippedIndex: number | null
  ) => {
    try {
      await db.updateTrialDrawFlippedIndex(_recordId, drawId, flippedIndex);
      onRefreshTrialRecords?.();
    } catch (error) {
      console.error('更新翻牌牌位失败:', error);
      toast.error('更新翻牌牌位失败');
    }
  };

  const handleUpdateTrialDrawEquipment = async (
    _recordId: string,
    drawId: string,
    cardIndex: number,
    equipmentId: string | null
  ) => {
    try {
      await db.updateTrialDrawEquipment(drawId, cardIndex, equipmentId);
      onRefreshTrialRecords?.();
    } catch (error) {
      console.error('更新翻牌装备失败:', error);
      toast.error('更新翻牌装备失败');
    }
  };

  const handleUpdateTrialDrawEquipmentReview = async (
    _recordId: string,
    draw: TrialDraw,
    reviewed: boolean
  ) => {
    try {
      await db.setTrialDrawEquipmentReviewed(draw.id, reviewed, draw.flippedIndex, [
        draw.card1, draw.card2, draw.card3, draw.card4, draw.card5,
      ]);
    } catch (error) {
      console.error('更新翻牌核对状态失败:', error);
      toast.error('更新翻牌核对状态失败');
    } finally {
      // 冲突时也刷新，避免用户重复按旧画面确认。
      onRefreshTrialRecords?.();
    }
  };

  return (
    <div className="flex flex-col h-full gap-4">
      <div className="flex-1 min-h-0 overflow-y-auto pr-1 space-y-6">
      {activeTab === 'baizhan' ? (
        <div className="space-y-4">
          <button
            onClick={() => setActiveTab('raid')}
            className="flex items-center gap-2 text-sm text-muted hover:text-main transition-colors"
          >
            ← 返回概览
          </button>
          <BaizhanManager
            records={baizhanRecords}
            accounts={accounts}
            onEditRecord={onEditBaizhanRecord}
            onRefreshRecords={onRefreshBaizhanRecords}
          />
        </div>
      ) : activeTab === 'trial' ? (
        <div className="space-y-4">
          <button
            onClick={() => setActiveTab('raid')}
            className="flex items-center gap-2 text-sm text-muted hover:text-main transition-colors"
          >
            ← 返回概览
          </button>
          <TrialPlaceManager
            records={trialRecords}
            accounts={accounts}
            config={config}
            onRefreshRecords={onRefreshTrialRecords}
            onConfigChange={onConfigChange}
          />
        </div>
      ) : selectedRaid ? (
        <RaidDetail
          key={`raidDetail-${getRaidKey(selectedRaid)}`}
          raid={selectedRaid}
          accounts={accounts}
          records={records}
          onBack={() => setSelectedRaid(null)}
          setRecords={setRecords}
          onEditRecord={onEditRecord}
          onRefreshRecords={onRefreshRecords}
        />
      ) : (
        <>
          <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4">
            <h2 className="text-xl font-bold text-main">副本管理</h2>
            <div className="flex gap-2 items-center">
              <PendingRecordsPanel
                records={pendingRecords ?? []}
                raids={raids}
                accounts={accounts}
                onRefreshRecords={onRefreshRecords}
                onRefreshTrialRecords={onRefreshTrialRecords}
                openSignal={pendingPanelOpenSignal}
              />
              {versions.length > 0 && (
                <VersionSelect
                  value={selectedVersion}
                  onChange={setSelectedVersion}
                  options={[
                    ...(hasCollectionEntries ? [{ value: OVERVIEW_VIEW, label: '概览' }] : []),
                    ...versions.map(version => ({ value: version, label: version })),
                  ]}
                />
              )}
            </div>
          </div>

          {/* CD 概览：一眼看出各角色在本周团队秘境、收藏副本、试炼之地与百战的进度，无需逐个点进详情。
              表头可直接跳转副本详情、试炼之地与百战页面。仅「概览」视图展示，切到赛季时回到副本卡片。 */}
          {selectedVersion === OVERVIEW_VIEW && (
            <RaidCdOverview
              raids={raids}
              accounts={accounts}
              records={records}
              trialRecords={trialRecords}
              baizhanRecords={baizhanRecords}
              trialProgressPeriod={trialProgressPeriod}
              weeklyRaidNames={weeklyRaidNames}
              favoriteRaids={favoriteRaids}
              seasonContext={seasonContext}
              versionOrder={versionOrderMap}
              onOpenRaid={setSelectedRaid}
              onOpenTrial={() => setActiveTab('trial')}
              onOpenBaizhan={() => setActiveTab('baizhan')}
              onRequestConfirm={() => setPendingPanelOpenSignal(prev => prev + 1)}
              onOpenRaidCell={handleRaidCellClick}
              onOpenTrialCell={handleTrialCellClick}
              onOpenBaizhanCell={handleBaizhanCellClick}
            />
          )}

          {selectedVersion === OVERVIEW_VIEW && weeklyRaidStatus !== 'success' && (
            <div className="rounded-card border border-ds-warning-soft bg-ds-warning-soft px-3 py-2 text-sm text-ds-warning-strong">
              {weeklyRaidStatus === 'loading' ? '正在获取本周团队秘境…' : '本周团队秘境获取失败，CD 概览仅包含已收藏副本。'}
            </div>
          )}

          {selectedVersion !== OVERVIEW_VIEW && (Object.keys(mergedGroupedRaids).length > 0 ? (
            versions.map(version => {
              const versionRaids = mergedGroupedRaids[version];
              if (!versionRaids) return null;

              return (
                <div key={version} className="space-y-4">
                  <div className="flex items-center gap-3">
                    <div className="h-px bg-border flex-1"></div>
                    <h3 className="text-sm font-bold text-muted uppercase tracking-wider bg-base px-3 py-1 rounded-full border border-border">
                      {version}
                    </h3>
                    <div className="h-px bg-border flex-1"></div>
                  </div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 sm:gap-4">
                    {versionRaids.map(mergedRaid => {
                      const isRaidActive = mergedRaid.isActive && mergedRaid.raids.some(r => r.isActive);
                      const isStatic = mergedRaid.raids.some(r => isStaticRaid(r));
                      const isDisabled = mergedRaid.disabled;
                      return (
                        <div
                          key={getRaidKey(mergedRaid.raids[0])}
                          className={`p-4 rounded-xl border transition-all duration-200 relative group
                        ${isDisabled
                              ? 'border-base bg-base opacity-70'
                              : 'bg-surface border-base hover:border-primary/50 hover:shadow-sm'
                            }`}
                        >
                          <div className="flex justify-between items-start mb-3">
                            <div className="flex items-center gap-2">
                              <span className={`p-1.5 rounded-lg ${isDisabled ? 'bg-base text-muted' : 'bg-primary/10 text-primary'}`}>
                                <Shield size={16} />
                              </span>
                              <div>
                                <h3 className={`font-semibold text-sm ${isDisabled ? 'text-muted' : 'text-main'}`}>{mergedRaid.name}</h3>
                              </div>
                            </div>
                            <div className="flex items-center gap-2">
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  toggleFavorite(mergedRaid.name);
                                }}
                                className={`transition-all p-1 rounded-md ${
                                  favoriteRaids.has(mergedRaid.name)
                                    ? 'text-ds-warning-strong hover:text-ds-warning-strong hover:bg-ds-warning-soft'
                                    : 'text-muted hover:text-ds-warning-strong hover:bg-ds-warning-soft/50'
                                }`}
                                title={favoriteRaids.has(mergedRaid.name) ? '取消收藏' : '收藏此副本'}
                              >
                                <Star size={16} fill={favoriteRaids.has(mergedRaid.name) ? 'currentColor' : 'none'} />
                              </button>
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  toggleRaidStatus(mergedRaid.name);
                                }}
                                className={`transition-all p-1 rounded-md ${isDisabled
                                  ? 'text-muted hover:text-main hover:bg-base'
                                  : isRaidActive
                                    ? 'text-ds-success-strong hover:text-ds-success-strong hover:bg-ds-success-soft'
                                    : 'text-muted hover:text-main hover:bg-base'
                                  }`}
                                title={isDisabled ? '启用此副本' : '禁用此副本（所有难度）'}
                              >
                                <Power size={16} />
                              </button>
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  if (isStatic) {
                                    alert('预制副本不能删除，只能禁用');
                                    return;
                                  }
                                  if (confirm('确认删除此副本？')) {
                                    // 先落库再改 UI：自动保存默认不删行，必须显式调用删除命令
                                    mergedRaid.raids.forEach(raid => {
                                      const key = getRaidKey(raid);
                                      db.deleteRaidStructured(key)
                                        .then(() => {
                                          setRaids(prev => prev.filter(r => getRaidKey(r) !== key));
                                        })
                                        .catch(error => {
                                          console.error('删除副本失败:', error);
                                          alert('删除副本失败，请重试');
                                        });
                                    });
                                  }
                                }}
                                className={`transition-colors p-1 rounded-md ${isStatic
                                  ? 'text-muted cursor-not-allowed opacity-50'
                                  : 'text-muted hover:text-red-500 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/40'
                                  }`}
                                title={isStatic ? '预制副本不能删除' : '删除'}
                              >
                                <Trash2 size={16} />
                              </button>
                            </div>
                          </div>

                          <div className="flex items-center gap-2 mb-3 flex-wrap justify-center sm:justify-start">
                            {mergedRaid.raids.map(raid => {
                              const label = mergedRaid.difficultyLabels[getRaidKey(raid)] || DIFFICULTY_LABELS[raid.difficulty];
                              const isSpecialRaid = raid.name === '会战弓月城' || raid.name === '缚罪之渊';
                              return (
                                <div
                                  key={getRaidKey(raid)}
                                  onClick={() => setSelectedRaid(raid)}
                                  onContextMenu={(e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    toggleRaidDifficultyStatus(getRaidKey(raid));
                                  }}
                                  className={`relative px-2 py-1 text-xs font-medium rounded border cursor-pointer transition-all hover:scale-105 min-w-[60px] text-center ${raid.isActive
                                    ? DIFFICULTY_COLORS[raid.difficulty]
                                    : 'bg-base text-muted border-base hover:bg-base/80 opacity-60'
                                    }`}
                                  title={raid.isActive ? `${label} - 点击进入详情，右键切换状态` : `已禁用 - ${label} - 右键启用${!raid.isActive && !isSpecialRaid ? '（默认禁用）' : ''}`}
                                >
                                  {label}
                                </div>
                              );
                            })}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              );
            })
          ) : (
            <div className="text-center py-12 text-muted bg-surface rounded-xl border border-base">
              <Shield className="w-12 h-12 mx-auto mb-2 opacity-20" />
              <p>暂无副本数据</p>
            </div>
          ))}
        </>
      )}
      </div>

      {/* CD 概览单元格点击后弹出的记录弹窗：未清/部分清 → 添加记录，已清 → 该角色在该副本的记录明细 */}
      {raidCellAction && raidCellAction.mode === 'add' && (
        <AddRecordModal
          isOpen
          onClose={() => setRaidCellAction(null)}
          onSubmit={handleAddRaidRecord}
          raid={raidCellAction.raid}
          role={{
            id: raidCellAction.row.roleId,
            name: raidCellAction.row.roleName,
            server: raidCellAction.row.server,
            region: raidCellAction.row.region,
            sect: raidCellAction.row.sect,
            accountId: raidCellAction.row.accountId,
            accountName: raidCellAction.row.accountName,
            bossCooldowns: raidCellAction.row.cells[getRaidKey(raidCellAction.raid)]?.bossCooldowns,
          }}
        />
      )}

      {raidCellAction && raidCellAction.mode === 'view' && (
        <RoleRecordsModal
          isOpen
          onClose={() => setRaidCellAction(null)}
          role={{
            id: raidCellAction.row.roleId,
            name: raidCellAction.row.roleName,
            server: raidCellAction.row.server,
            region: raidCellAction.row.region,
            sect: raidCellAction.row.sect,
            accountId: raidCellAction.row.accountId,
            accountName: raidCellAction.row.accountName,
          }}
          records={records}
          raid={raidCellAction.raid}
          setRecords={setRecords}
          isAdmin={true}
          onEditRecord={onEditRecord}
          onRefreshRecords={onRefreshRecords}
        />
      )}

      {trialCellAction && trialCellAction.mode === 'add' && (
        <AddTrialRecordModal
          isOpen
          onClose={() => setTrialCellAction(null)}
          onSubmit={handleAddTrialRecord}
          accounts={accounts}
          initialRole={{
            id: trialCellAction.row.roleId,
            name: trialCellAction.row.roleName,
            server: trialCellAction.row.server,
            region: trialCellAction.row.region,
            accountId: trialCellAction.row.accountId,
            accountName: trialCellAction.row.accountName,
            sect: trialCellAction.row.sect ?? '',
          }}
          autoFillEnabled={config?.trial?.autoFillEnabled ?? false}
          records={trialRecords}
          trialProgressPeriod={trialProgressPeriod}
        />
      )}

      {trialCellAction && trialCellAction.mode === 'view' && (
        <TrialRoleRecordsModal
          isOpen
          onClose={() => setTrialCellAction(null)}
          role={{
            id: trialCellAction.row.roleId,
            name: trialCellAction.row.roleName,
            server: trialCellAction.row.server,
            sect: trialCellAction.row.sect,
            martial: trialCellAction.row.martial,
            accountName: trialCellAction.row.accountName,
          }}
          records={trialRecords}
          onDeleteRecord={handleDeleteTrialRecord}
          onUpdateDrawFlippedIndex={handleUpdateTrialDrawFlippedIndex}
          onUpdateDrawEquipment={handleUpdateTrialDrawEquipment}
          onUpdateDrawEquipmentReview={handleUpdateTrialDrawEquipmentReview}
        />
      )}

      {baizhanCellAction && baizhanCellAction.mode === 'add' && (
        <AddBaizhanRecordModal
          isOpen
          onClose={() => setBaizhanCellAction(null)}
          onSubmit={handleAddBaizhanRecord}
          accounts={accounts}
          initialRole={{
            id: baizhanCellAction.row.roleId,
            name: baizhanCellAction.row.roleName,
            server: baizhanCellAction.row.server,
            region: baizhanCellAction.row.region,
            accountId: baizhanCellAction.row.accountId,
            accountName: baizhanCellAction.row.accountName,
            sect: baizhanCellAction.row.sect ?? '',
            equipmentScore: baizhanCellAction.row.equipmentScore,
          }}
        />
      )}

      {baizhanCellAction && baizhanCellAction.mode === 'view' && (
        <BaizhanRoleRecordsModal
          isOpen
          onClose={() => setBaizhanCellAction(null)}
          role={{
            id: baizhanCellAction.row.roleId,
            name: baizhanCellAction.row.roleName,
            server: baizhanCellAction.row.server,
            sect: baizhanCellAction.row.sect,
            martial: baizhanCellAction.row.martial,
            accountName: baizhanCellAction.row.accountName,
          }}
          records={baizhanRecords}
          onDeleteRecord={handleDeleteBaizhanRecord}
          onEditRecord={onEditBaizhanRecord}
        />
      )}
    </div>
  );
};
